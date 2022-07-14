use std::collections::BTreeMap;
use serde::Serialize;
use crate::article::storage;
pub use crate::article::storage::fetch_article_links;

#[derive(Serialize, Clone, Debug)]
pub struct ArticleViewLink {
    pub title: String,
    pub slug: String,
    pub timestamp: i64,
    pub tags: Vec<String>,
}

impl ArticleViewLink {
    pub fn from_redis_result(a: (String, String, i64), tags: Vec<String>) -> Self {
        Self {
            title: a.0,
            slug: a.1,
            timestamp: a.2,
            tags
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ArticleView {
    pub title: String,
    pub content: String,
    pub slug: String,
    pub timestamp: i64,
    pub tags: Vec<String>,
    pub prev: Option<ArticleViewLink>,
    pub next: Option<ArticleViewLink>,
}

impl ArticleView {
    pub fn from_redis_result(
        result: &BTreeMap<String, String>,
        tags: Vec<String>,
        prev: Option<ArticleViewLink>,
        next: Option<ArticleViewLink>,
    ) -> Self {
        let timestamp = result.get("timestamp").unwrap();
        Self {
            title: result.get("title").unwrap().to_string(),
            content: result.get("content").unwrap().to_string(),
            slug: result.get("slug").unwrap().to_string(),
            timestamp: timestamp.parse::<i64>().unwrap_or(0),
            tags,
            prev,
            next,
        }
    }

    pub fn from_slug(slug: &str) -> Option<ArticleView> {
        match storage::fetch_from_slug(slug) {
            Ok(result) => {
                Some(result)
            },
            Err(_) => None
        }
    }
}
