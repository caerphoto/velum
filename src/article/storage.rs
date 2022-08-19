use crate::article::view::{ContentView, IndexView};
use crate::article::builder::{Builder, ParseResult, ParseError};
use std::fs;
use std::cmp::min;
use std::path::PathBuf;

pub const DEFAULT_CONTENT_DIR: &str = "./content";

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

    if tag.is_some() {
        let tag = &tag.unwrap().to_string();
        let index_views: Vec<IndexView> = articles
            .iter()
            .filter(|cv| cv.tags.contains(tag))
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
        if a.slug == slug { return Some(&a) }
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

fn builder_to_content_view(builder: Builder) -> ParseResult<ContentView> {
        let title = builder.title()?;
        Ok(ContentView {
            slug: builder.slug(&title)?, // borrow here before
            title,                       // move here
            content: builder.parsed_content(),
            timestamp: builder.timestamp,
            tags: builder.tags(),
            prev: None,
            next: None,
        })
}

pub fn gather_fs_articles(config: &config::Config) -> ParseResult<Vec<ContentView>> {
    let content_dir = config
        .get_string("content_dir")
        .unwrap_or(DEFAULT_CONTENT_DIR.to_owned());
    let path = PathBuf::from(content_dir).join("articles");
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
        if let Ok(builder) = Builder::from_file(&path) {
            let view = builder_to_content_view(builder)?;
            articles.push(view);
        } else {
            // Build can fail if, for example, the file contains invalid UTF-8
            // byte sequences, but we don't really need to panic or return an
            // error, just log the problem and carry on with the next file.
            log::error!("Failed to build article from {}", path.to_string_lossy());
        }
    }
    articles.sort_by_key(|k| k.timestamp);
    articles.reverse();
    set_prev_next(&mut articles);
    Ok(articles)
}
