use std::time;

use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::{Html, IntoResponse},
};
use headers::HeaderMap;
use regex::Regex;
use tower_cookies::Cookies;

use super::{log_elapsed, not_found, server_error, theme};
use crate::article::{storage::fetch_by_slug, view::ArticleRenderView};
use crate::SharedData;

fn return_path(blog_host: &str, uri: Option<String>) -> String {
    lazy_static! {
        static ref INDEX_PATH: Regex = Regex::new(
            // matches:
            //   /articles/<page>
            //   /tag/<tag>
            //   /tag/<tag>/<page>
            r"^(/articles/\d+)|(/tag/[a-z0-9\-]+(/\d+)?)"
        ).unwrap();
    }
    let default_path = "/".to_string();
    if uri.is_none() {
        return default_path;
    }
    if let Ok(referer) = uri.unwrap().parse::<Uri>() {
        if let Some(host) = referer.host() {
            if host != blog_host {
                return default_path;
            }
        }
        if referer.path() == "/" || INDEX_PATH.is_match(referer.path()) {
            return referer.path().to_string();
        }
    }

    default_path
}

pub async fn article_text_handler(
    Path(slug): Path<String>,
    State(data): State<SharedData>,
) -> impl IntoResponse {
    let data = data.read();
    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        (StatusCode::OK, article.base_content.clone())
    } else {
        (StatusCode::NOT_FOUND, "Article not found".to_string())
    }
}

pub async fn article_handler(
    Path(slug): Path<String>,
    State(data): State<SharedData>,
    headers: HeaderMap,
    cookies: Cookies,
) -> impl IntoResponse {
    let now = time::Instant::now();
    let data = data.read();

    let referer = headers
        .get("Referer")
        .map(|r| String::from(r.to_str().unwrap_or("")));

    let return_path = return_path(&data.config.blog_host, referer);

    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        let theme = theme(cookies);
        let render_data =
            ArticleRenderView::new(article, &data.articles, &return_path, &theme, &data);
        match data.hbs.render("article", &render_data) {
            Ok(rendered_page) => {
                let reply = (StatusCode::OK, Html(rendered_page));
                log_elapsed("ARTICLE", Some(&slug), None, now);
                reply
            }
            Err(e) => server_error(&format!("Failed to render article. Error: {e:?}")),
        }
    } else {
        not_found(None)
    }
}
