use std::collections::BTreeMap;
use serde::Serialize;
use pulldown_cmark as cmark;
use crate::article::storage;
pub use crate::article::storage::gather_article_links;

#[derive(Serialize, Clone, Debug)]
pub struct ArticleViewLink {
    pub title: String,
    pub route: String,
    pub timestamp: i64,
}

impl ArticleViewLink {
    pub fn from_redis_result(a: (String, String, i64)) -> Self {
        Self {
            title: a.0,
            route: a.1,
            timestamp: a.2
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ArticleView {
    pub title: String,
    pub content: String,
    pub route: String,
    pub timestamp: i64,
    pub prev: Option<ArticleViewLink>,
    pub next: Option<ArticleViewLink>,
}

impl ArticleView {
    fn parse_content(content: &str) -> String {
        let mut parsed_article = String::new();
        let no_title: String = content
            .lines()
            .skip(1)
            .collect::<Vec<&str>>()
            .join("\n");
        let parser = cmark::Parser::new(&no_title);
        cmark::html::push_html(&mut parsed_article, parser);
        parsed_article
    }


    pub fn from_redis_result(
        result: &BTreeMap<String, String>,
        prev: Option<ArticleViewLink>,
        next: Option<ArticleViewLink>,
    ) -> Self {
        let timestamp = result.get("timestamp").unwrap();
        let content = result.get("content").unwrap();
        Self {
            title: result.get("title").unwrap().to_string(),
            content: Self::parse_content(content),
            route: result.get("route").unwrap().to_string(),
            timestamp: timestamp.parse::<i64>().unwrap_or(0),
            prev: prev,
            next: next,
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
