use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::time;
use tower_cookies::Cookies;

use super::{
    log_elapsed,
    server_error,
    theme
};

use crate::article::{
    storage::{fetch_index_links, LinkList},
    view::{
        ContentView,
        IndexRenderView,
        RssIndexView,
        RssArticleView,
    },
};
use crate::CommonData;
use crate::SharedData;

fn render_article_list(
    article_list: LinkList,
    tag: Option<&str>,
    page: usize,
    page_size: usize,
    theme: String,
    data: &CommonData,
) -> (StatusCode, Html<String>) {
    let render_data = IndexRenderView::new(
        article_list,
        tag,
        page,
        page_size,
        theme,
        data
    );

    match data.hbs.render( "index", &render_data) {
        Ok(rendered_page) => (StatusCode::OK, Html(rendered_page)),
        Err(e) => server_error(&format!(
            "Failed to render article in index. Error: {:?}",
            e
        )),
    }
}

pub async fn home_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    index_handler(Path(String::from("0")), Extension(data), cookies).await
}

pub async fn index_handler(
    Path(page_or_slug): Path<String>,
    Extension(data): Extension<SharedData>,
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
    let data = data.lock().unwrap();

    let page_size = data.config.page_size;
    let article_list = fetch_index_links(page, page_size, None, &data.articles);

    let response = render_article_list(
        article_list,
        None,
        page,
        page_size,
        theme(cookies),
        &data,
    );
    log_elapsed("article index", None, Some(page), now);

    Ok(response)
}

fn build_rss_articles<'a>(data: &'a CommonData) -> Vec<RssArticleView<'a>> {
    let end_index = std::cmp::min(10, data.articles.len());
    data.articles[..end_index].iter().map(|a| a.to_rss_view(&data.config.blog_url)).collect()
}

pub async fn rss_handler(
    Extension(data): Extension<SharedData>,
) -> impl IntoResponse {
    let data = data.lock().unwrap();
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
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    tag_handler(Path((tag, 1)), Extension(data), cookies).await
}

pub async fn tag_handler(
    Path((tag, page)): Path<(String, usize)>,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    let now = time::Instant::now();
    let tag_copy = tag.clone();
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;

    let article_result = fetch_index_links(page, page_size, Some(&tag), &data.articles);

    let response = render_article_list(
        article_result,
        Some(&tag),
        page,
        page_size,
        theme(cookies),
        &data,
    );
    log_elapsed("tag index", Some(&tag_copy), Some(page), now);
    response
}
