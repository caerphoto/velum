use std::time;

use axum::{
    http::{
        StatusCode,
        Uri,
    },
    response::{Html, IntoResponse},
    extract::{Path, Extension},
};
use headers::HeaderMap;
use tower_cookies::Cookies;
use serde_json::json;
use regex::Regex;

use crate::SharedData;
use crate::article::storage::fetch_by_slug;
use super::{
    server_error,
    not_found,
    theme,
};

fn return_path(blog_host: &str, uri: Option<String>) -> String {
    lazy_static! {
        static ref INDEX_PATH: Regex = Regex::new(
            // matches:
            //   /index/<page>
            //   /tag/<tag>
            //   /tag/<tag>/<page>
            r"^(/index/\d+)|(/tag/[a-z0-9\-]+(/\d+)?)"
        ).unwrap();
    }
    let default_path = "/".to_string();
    if uri.is_none() { return default_path; }
    if let Ok(referer) = uri.unwrap().parse::<Uri>() {
        if let Some(host) = referer.host() {
            if host != blog_host { return default_path }
        }
        if referer.path() == "/" || INDEX_PATH.is_match(referer.path()) {
            return referer.path().to_string();
        }
    }

    default_path
}

pub async fn article_text_handler(
    Path(slug): Path<String>,
    Extension(data): Extension<SharedData>,
) -> impl IntoResponse {
    let data = data.lock().unwrap();
    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        (StatusCode::OK, article.base_content.clone())
    } else {
        (StatusCode::NOT_FOUND, "Article not found".to_string())
    }
}

pub async fn article_handler(
    Path(slug): Path<String>,
    Extension(data): Extension<SharedData>,
    headers: HeaderMap,
    cookies: Cookies,
) -> impl IntoResponse {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;

    let referer = headers
        .get("Referer")
        .map(|r| String::from(r.to_str().unwrap_or("")));

    let return_path = return_path(&data.config.blog_host, referer);

    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        let comments = data.comments.get(&slug);
        match  data.hbs.render(
            "article",
            &json!({
                "title": &article.title,
                "blog_title": blog_title,
                "article": article,
                "comments": comments,
                "prev": article.prev,
                "next": article.next,
                "return_path": return_path,
                "body_class": "article",
                "content_dir": &data.config.content_dir,
                "themes": &data.config.theme_list,
                "theme": theme(cookies),
            })
        ) {
            Ok(rendered_page) => {
                let reply = (StatusCode:: OK, Html(rendered_page));

                log::info!(
                    "Rendered article `{}` in {}µs",
                    &slug,
                    now.elapsed().as_micros()
                );
                reply
            },
            Err(e) => {
                server_error(&format!( "Failed to render article. Error: {:?}", e))
            }
        }
    } else {
        not_found(None)
    }
}

