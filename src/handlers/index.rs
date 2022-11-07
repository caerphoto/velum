use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
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
    view::IndexRenderView,
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
