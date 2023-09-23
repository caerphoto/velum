use std::fs;
use std::path::{PathBuf, Path};
use std::io::{self, ErrorKind};
use serde::Serialize;
use uuid::Uuid;
use crate::CommonData;
use crate::config::Config;
use crate::errors::{ParseResult, ParseError};
use crate::article::builder::{Builder, ParsedArticle, ArticlePrevNext};
use crate::io::paths_with_ext_in_dir;


#[derive(Serialize)]
pub struct PaginatedArticles<'a> {
    pub articles: Vec<&'a ParsedArticle>,
    pub total_articles: usize,
}

pub fn fetch_paginated_articles<'a>(
    page: usize,
    per_page: usize,
    tag: Option<&str>,
    articles: &'a [ParsedArticle],
) -> PaginatedArticles<'a> {
    let article_subset: Vec<&ParsedArticle>  = if let Some(t) = tag {
        let t = t.to_string();
        articles
            .iter()
            .filter(|cv| cv.tags.contains(&t))
            .collect()
    } else {
        articles.iter().collect()
    };

    // Pages are normally provided as 1-indexed from the URL, but page 0 is also valid: it means
    // the 'home' index page, where we show the 'blog info' box.
    let page = page.saturating_sub(1);

    let total_articles = article_subset.len();
    if let Some(chunk) = article_subset.chunks(per_page).nth(page) {
        PaginatedArticles { articles: chunk.into(), total_articles }
    } else {
        if article_subset.is_empty() {
            log::error!("No articles found. Tag: {tag:?}");
        } else {
            log::error!("Problem getting page {page} from subset of length {total_articles} with chunk size of {per_page}. Tag {tag:?}");
        }
        PaginatedArticles { articles: Vec::new(), total_articles }
    }

}

pub fn fetch_by_slug<'a >(slug: &str, articles: &'a [ParsedArticle]) -> Option<&'a ParsedArticle> {
    articles.iter().find(|a| a.slug == slug)
}

fn fetch_by_slug_mut<'a >(slug: &str, articles: &'a mut [ParsedArticle]) -> Option<&'a mut ParsedArticle> {
    articles.iter_mut().find(|a| a.slug == slug)
}

fn set_prev_next(articles: &mut Vec<ParsedArticle>) {
    for i in 0..articles.len() { // can't use enumerate because we need to borrow mut within loop
        let prev = if i > 0 {
            articles.get(i - 1).map(ArticlePrevNext::from)
        } else {
            None
        };
        let next = articles.get(i + 1).map(ArticlePrevNext::from);
        let a = &mut articles[i];
        a.prev = prev;
        a.next = next;
    }
}

fn update_article_source(path: &PathBuf, content: &str) -> Result<(), std::io::Error> {
    let metadata = fs::metadata(path)?;
    let filedate = match metadata.created() {
        Ok(c) => c,
        Err(_) => metadata.modified()?
    };
    let mtime = filetime::FileTime::from_system_time(filedate);
    fs::write(path, content)?;

    // Modified time needs to be restored to original value to preserve
    // article order.
    filetime::set_file_mtime(path, mtime)
}

pub fn create_article(content: &str, data: &mut CommonData) -> Result<ParsedArticle, std::io::Error> {
    let temp_filename = PathBuf::from(data.config.content_dir.clone())
        .join("articles")
        .join(Uuid::new_v4().to_string() + ".md");
    fs::write(&temp_filename, content)?;

    let builder = Builder::from_file(&temp_filename, data.config.max_preview_length)?;
    if let Ok(slug) = builder.slug() {
        let new_filename = temp_filename
            .with_file_name(
                slug + builder.timestamp.to_string().as_str() + ".md"
            );
        log::info!("temp: {:?}, new: {:?}", temp_filename, new_filename);
        if new_filename.is_file() {
            return Err(io::Error::new(ErrorKind::Other, "File already exists"))
        }
        fs::rename(&temp_filename, &new_filename)?;
        builder.try_into()
            .map_err(|e| {
                let msg = format!("Failed to create parsed article frrom builder: {e:?}");
                io::Error::new(ErrorKind::Other, msg)
            })
    } else {
        Err(io::Error::new(ErrorKind::Other, "Couldn't create slug from content"))

    }
}

pub fn update_article(slug: &str, new_content: &str, data: &mut CommonData) -> Result<(), std::io::Error> {

    let res = fetch_by_slug_mut(slug, &mut data.articles);
    if let Some(article) = res {
        let builder = Builder {
            content: new_content.to_string(),
            timestamp: article.timestamp,
            source_filename: article.source_filename.clone(),
            max_preview_length: data.config.max_preview_length,
        };

        if let Ok(new_article) = ParsedArticle::try_from(&builder) {
            article.base_content = new_article.base_content;
            article.parsed_content = new_article.parsed_content;
            article.preview = new_article.preview;
            article.tags = new_article.tags;
        }
        update_article_source(
            &article.source_filename,
            &article.base_content,
        )
    } else {
        Err(io::Error::new(ErrorKind::Other, "failed to fetch mutable article reference"))
    }
}

pub fn delete_article<P: AsRef<Path>>(source_filename: P) -> Result<(), std::io::Error> {
    fs::remove_file(source_filename.as_ref())
}

pub fn gather_fs_articles(config: &Config) -> ParseResult<Vec<ParsedArticle>> {
    let dir = PathBuf::from(&config.content_dir).join("articles");
    if !dir.is_dir() {
        let dir = dir.to_string_lossy();
        return Err(ParseError {
            cause: format!("article path `{}` is not a directory", &dir)
        });
    }

    let mut articles: Vec<ParsedArticle> = Vec::new();

    paths_with_ext_in_dir("md", &dir, |path| {
        log::debug!("Building article from {}", path.to_string_lossy());
        match Builder::from_file(path, config.max_preview_length) {
            Ok(builder) => {
                if let Ok(article) = ParsedArticle::try_from(&builder) {
                    articles.push(article);
                } else {
                    log::error!(
                        "Failed to convert builder:\n{}\nto article",
                        &builder
                    );
                }
            },
            Err(e) => {
                // Build can fail if, for example, the file contains invalid UTF-8
                // byte sequences, but we don't really need to panic or return an
                // error, just log the problem and carry on with the next file.
                log::error!(
                    "Failed to build article from {}: {:?}",
                    path.to_string_lossy(),
                    e
                );
            }
        }
    });

    articles.sort_by_key(|k| k.timestamp);
    articles.reverse();
    set_prev_next(&mut articles);
    Ok(articles)
}
