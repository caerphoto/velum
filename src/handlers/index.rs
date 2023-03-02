use axum::{
    extract::{State, Path},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_macros::debug_handler;
use std::time;
use tower_cookies::Cookies;

use super::{
    log_elapsed,
    server_error,
    theme
};

use crate::article::{
    storage::{fetch_paginated_articles, PaginatedArticles},
    view::{
        IndexRenderView,
        RssIndexView,
        RssArticleView,
    },
};
use crate::CommonData;
use crate::SharedData;

fn render_article_list(
    article_list: PaginatedArticles,
    tag: Option<&str>,
    page: usize,
    page_size: usize,
    theme: String,
    data: &CommonData,
) -> (StatusCode, Html<String>) {
    let render_data = IndexRenderView::new(
        &article_list,
        tag,
        page,
        page_size,
        theme,
        data
    );

    let status = if article_list.total_articles > 0 {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };

    match data.hbs.render( "index", &render_data) {
        Ok(rendered_page) => (status, Html(rendered_page)),
        Err(e) => server_error(&format!( "Failed to render article in index. Error: {e:?}")),
    }
}

#[debug_handler]
pub async fn home_handler(
    State(data): State<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    index_handler(Path(String::from("0")), State(data), cookies).await
}

pub async fn index_handler(
    Path(page_or_slug): Path<String>,
    State(data): State<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Handle legacy article route, i.e. /articles/:slug
    // NOTE: eventually this should be removed, once the requests for the old route taper off
    let parse_result = page_or_slug.parse::<usize>();
    if parse_result.is_err() {
        return Err(Redirect::permanent(&(String::from("/article/") + &page_or_slug)))
    }
    let page = parse_result.unwrap();

    let now = time::Instant::now();
    let data = data.read();

    let page_size = data.config.page_size;
    let article_list = fetch_paginated_articles(page, page_size, None, &data.articles);

    let response = render_article_list(
        article_list,
        None,
        page,
        page_size,
        theme(cookies),
        &data,
    );
    log_elapsed("ARTICLE INDEX", None, Some(page), now);

    Ok(response)
}

fn build_rss_articles(data: &CommonData) -> Vec<RssArticleView> {
    let end_index = std::cmp::min(10, data.articles.len());
    data.articles[..end_index]
        .iter()
        .map(|a| RssArticleView::from_parsed_article(a, &data.config.blog_url))
        .collect()
}

pub async fn rss_handler(
    State(data): State<SharedData>,
) -> impl IntoResponse {
    let now = time::Instant::now();
    let data = data.read();
    let articles = build_rss_articles(&data);
    let render_data = RssIndexView {
        blog_title: &data.config.blog_title,
        blog_url: &data.config.blog_url,
        blog_description: &data.config.blog_description,
        articles,
    };

    let res = Response::builder()
        .header("Content-Type", "application/rss+xml;charset=utf-8");

    match data.hbs.render("rss", &render_data) {
        Ok(rendered_doc) => {
            log_elapsed("RSS FEED", None, None, now);
            res.status(StatusCode::OK)
                .body(rendered_doc)
                .unwrap()
        },
        Err(e) => {
            log::error!("Error rendering RSS feed: {:?}", e);
            res.status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Error rendering RSS feed".to_string())
                .unwrap()
        },
    }
}

pub async fn tag_home_handler(
    Path(tag): Path<String>,
    State(data): State<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    tag_handler(Path((tag, 1)), State(data), cookies).await
}

pub async fn tag_handler(
    Path((tag, page)): Path<(String, usize)>,
    State(data): State<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    let now = time::Instant::now();
    let tag_copy = tag.clone();
    let data = data.read();
    let page_size = data.config.page_size;

    let article_result = fetch_paginated_articles(page, page_size, Some(&tag), &data.articles);

    let response = render_article_list(
        article_result,
        Some(&tag),
        page,
        page_size,
        theme(cookies),
        &data,
    );
    log_elapsed("TAG INDEX", Some(&tag_copy), Some(page), now);
    response
}
