use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde_json::json;
use std::time;
use tower_cookies::Cookies;

use super::{
    log_elapsed,
    server_error,
    theme
};

use crate::article::storage::{fetch_index_links, LinkList};
use crate::CommonData;
use crate::SharedData;

// Integer division rounding up, for calculating page count
fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 && rhs > 0 {
        d + 1
    } else {
        d
    }
}

fn render_article_list(
    article_list: LinkList,
    mut page: usize,
    page_size: usize,
    data: &CommonData,
    tag: Option<&str>,
    theme: Option<String>,
) -> (StatusCode, Html<String>) {
    let blog_title = &data.config.blog_title;
    let last_page = div_ceil(article_list.total_articles, page_size);

    let title = if let Some(tag) = tag {
        String::from("Tag: ") + tag
    } else {
        String::from("Article Index")
    };

    // Page '0' is the home page: shows the same article list as the first index
    // page, but has the additional home page info box.
    let home_page_info = if page == 0 {
        Some(&data.config.info_html)
    } else {
        None
    };
    page = std::cmp::max(page, 1);

    match data.hbs.render(
        "index",
        &json!({
            "blog_title": blog_title,
            "title": title,
            "prev_page": if page > 1 { page - 1 } else { 0 },
            "current_page": page,
            "next_page": if page < last_page { page + 1 } else { 0 },
            "last_page": last_page,
            "search_tag": tag.unwrap_or(""),
            "article_count": article_list.total_articles,
            "articles": &article_list.index_views,
            "body_class": if tag.is_some() { "tag-index" } else { "index" },
            "content_dir": &data.config.content_dir,
            "themes": &data.config.theme_list,
            "theme": theme,
            "home_page_info": home_page_info,
        }),
    ) {
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

    let response = render_article_list(article_list, page, page_size, &data, None, theme(cookies));
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
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;

    let article_result = fetch_index_links(page, page_size, Some(&tag), &data.articles);

    let response = render_article_list(
        article_result,
        page,
        page_size,
        &data,
        Some(&tag),
        theme(cookies),
    );
    log_elapsed("tag index", Some(&tag), Some(page), now);
    response
}
