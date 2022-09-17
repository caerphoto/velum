use std::fs;
use std::cmp::min;
use std::path::PathBuf;
use std::io::{self, ErrorKind};
use uuid::Uuid;
use crate::CommonData;
use crate::config::Config;
use crate::errors::{ParseResult, ParseError};
use crate::article::view::{ContentView, IndexView};
use crate::article::builder::Builder;

pub struct LinkList {
    pub index_views: Vec<IndexView>,
    pub total_articles: usize,
}

fn indices_from_page(page: usize, per_page: usize) -> (usize, usize) {
    let start_index = page.saturating_sub(1) * per_page;
    let end_index = start_index + per_page - 1;
    (start_index, end_index)
}

pub fn fetch_index_links(
    page: usize,
    per_page: usize,
    tag: Option<&str>,
    articles: &Vec<ContentView>,
) -> LinkList {
    let (mut start, mut end) = indices_from_page(page, per_page);

    if let Some(t) = tag {
        let index_views: Vec<IndexView> = articles
            .iter()
            .filter(|cv| cv.tags.contains(&t.to_string()))
            .map(ContentView::to_index_view)
            .collect();

        end = min(end, index_views.len());
        start = min(start, end);
        LinkList {
            index_views: index_views[start..end].into(),
            total_articles: index_views.len(),
        }
    } else {
        end = min(end, articles.len());
        start = min(start, end);
        LinkList {
            index_views: articles[start..end]
                .iter()
                .map(ContentView::to_index_view)
                .collect(),
            total_articles: articles.len(),
        }
    }
}

pub fn fetch_by_slug<'a >(slug: &str, articles: &'a Vec<ContentView>) -> Option<&'a ContentView> {
    for a in articles {
        if a.slug == slug { return Some(a) }
    }

    None
}

fn fetch_by_slug_mut<'a >(slug: &str, articles: &'a mut Vec<ContentView>) -> Option<&'a mut ContentView> {
    for a in articles {
        if a.slug == slug { return Some(a) }
    }

    None
}


fn set_prev_next(articles: &mut Vec<ContentView>) {
    for i in 0..articles.len() {
        let prev = if i > 0 {
            articles.get(i - 1).map(|v| v.to_prev_next_view())
        } else {
            None
        };
        let next = articles.get(i + 1).map(|v| v.to_prev_next_view());
        let mut a = &mut articles[i];
        a.prev = prev;
        a.next = next;
    }
}

fn builder_to_content_view(builder: Builder, config: &Config) -> ParseResult<ContentView> {
        let title = builder.title()?;
        Ok(ContentView {
            slug: builder.slug()?, // borrow here before
            title,                       // move here
            parsed_content: builder.parsed_content(),
            base_content: builder.content.clone(),
            preview: builder.content_preview(config.max_preview_length),
            source_filename: builder.source_filename.clone(),
            timestamp: builder.timestamp,
            tags: builder.tags(),
            prev: None,
            next: None,
        })
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

pub fn create_article(content: &str, data: &mut CommonData) -> Result<IndexView, std::io::Error> {
    let temp_filename = PathBuf::from(data.config.content_dir.clone())
        .join("articles")
        .join(Uuid::new_v4().to_string() + ".md");
    fs::write(&temp_filename, content)?;

    let builder = Builder::from_file(&temp_filename)?;
    if let Ok(slug) = builder.slug() {
        let new_filename = temp_filename
            .clone()
            .with_file_name(
                slug.clone() + builder.timestamp.to_string().as_str() + ".md"
            );
        log::info!("temp: {:?}, new: {:?}", temp_filename, new_filename);
        if new_filename.is_file() {
            return Err(io::Error::new(ErrorKind::Other, "File already exists"))
        }
        fs::rename(&temp_filename, &new_filename)?;
        Ok(IndexView {
            title: builder.title().unwrap_or_else(|_| "error".to_string()),
            slug,
            preview: "".to_string(),
            timestamp: builder.timestamp,
            tags: Vec::new()
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
        };

        if let Ok(new_article) = builder_to_content_view(builder, &data.config) {
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

pub fn delete_article(article: &ContentView) -> Result<(), std::io::Error> {
    fs::remove_file(&article.source_filename)
}

pub fn gather_fs_articles(config: &Config) -> ParseResult<Vec<ContentView>> {
    let path = PathBuf::from(&config.content_dir).join("articles");
    if !path.is_dir() {
        let path = path.to_string_lossy();
        return Err(ParseError { cause: format!("article path `{}` is not a directory", &path) });
    }

    let mut articles: Vec<ContentView> = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() { continue }
        let ext = path.extension().map(|e| e.to_ascii_lowercase());
        if ext.is_none() || ext.unwrap() != "md" { continue }

        log::debug!("Building article from {}", path.to_string_lossy());
        match Builder::from_file(&path) {
            Ok(builder) => {
                let view = builder_to_content_view(builder, config)?;
                articles.push(view);
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
    }
    articles.sort_by_key(|k| k.timestamp);
    articles.reverse();
    set_prev_next(&mut articles);
    Ok(articles)
}
