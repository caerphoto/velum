mod admin;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::{fs, time::{self, SystemTime, UNIX_EPOCH}};
use std::net::SocketAddr;
use std::convert::Infallible;
use warp::http::StatusCode;
use serde_json::json;
use crate::CommonData;
use crate::comments::Comment;
use crate::article::storage::{
    LinkList,
    fetch_by_slug,
    fetch_index_links,
};

pub use admin::{
    admin_route,
    login_page_route,
    do_login_route,
    do_logout_route,
    rebuild_index_route,
    create_article_route,
    update_article_route,
    delete_article_route,
};

pub type WarpResult = Result<
    warp::reply::Response,
    warp::reject::Rejection
>;


type InfResult<T> = Result<T, Infallible>;

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

// ---------------------------------------------------------------------------
// TODO: refactor the return types of these functions to be like the admin
//       routes, which are much cleaner.
// ---------------------------------------------------------------------------

pub fn error_response(msg: String) -> Result<warp::reply::WithStatus<warp::reply::Html<String>>, Infallible> {
    let reply = warp::reply::html(msg);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::INTERNAL_SERVER_ERROR))
}

fn build_return_path(page: usize, tag: Option<&str>) -> String {
    if tag.is_some() {
        if page <= 1 {
            format!("/tag/{}", tag.unwrap())
        } else {
            format!("/tag/{}/{}", tag.unwrap(), page)
        }
    } else if page <= 1 {
        "/".to_string()
    } else {
        format!("/index/{}", page)
    }
}

fn render_article_list(
    article_list: LinkList,
    page: usize,
    page_size: usize,
    data: &CommonData,
    tag: Option<&str>,
) -> InfResult<warp::reply::WithStatus<warp::reply::Html<String>>> {
    let title = &data.config.blog_title;
    let max_page = div_ceil(article_list.total_articles, page_size);
    let return_to = build_return_path(page, tag);

    match data.hbs.render(
        "main",
        &json!({
            "title": title,
            "prev_page": if page > 1 { page - 1 } else { 0 },
            "current_page": page,
            "next_page": if page < max_page { page + 1 } else { 0 },
            "max_page": max_page,
            "search_tag": tag.unwrap_or(""),
            "article_count": article_list.total_articles,
            "articles": &article_list.index_views,
            "return_to": return_to,
            "body_class": if tag.is_some() { "tag-index" } else { "index" },
        })
    ) {
        Ok(rendered_page) => {
            Ok(warp::reply::with_status(
                warp::reply::html(rendered_page),
                warp::http::StatusCode::OK)
            )
        },
        Err(e) => {
            error_response(format!("Failed to render article in index. Error: {:?}", e))
        }
    }
}

pub async fn index_page_route(page: usize, data: Arc<Mutex<CommonData>>) -> InfResult<impl warp::Reply> {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;
    let article_list = fetch_index_links(page, page_size, None, &data.articles);
    let response = render_article_list(article_list, page, page_size, &data, None);
    if response.is_ok() {
        log::info!("Rendered article index page {} in {}ms", page, now.elapsed().as_millis());
    }

    response
}

pub async fn tag_search_route(tag: String, page: usize, data: Arc<Mutex<CommonData>>) -> InfResult<impl warp::Reply> {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;
    let article_result = fetch_index_links(page, page_size, Some(&tag), &data.articles);
    let response = render_article_list(article_result, page, page_size, &data, Some(&tag));
    if response.is_ok() {
        log::info!("Rendered tag '{}' index page {} in {}ms", &tag, page, now.elapsed().as_millis());
    }
    response
}

pub async fn article_text_route(slug: String, data: Arc<Mutex<CommonData>>) -> Result<impl warp::Reply, warp::Rejection> {
    let data = data.lock().unwrap();
    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        Ok(warp::http::Response::builder()
            .status(200)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(article.base_content.clone())
            .unwrap()
        )
    } else {
        Err(warp::reject::not_found())
    }
}

pub async fn article_route(slug: String, query: HashMap<String, String>, data: Arc<Mutex<CommonData>>) -> Result<impl warp::Reply, warp::Rejection> {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let title = &data.config.blog_title;

    let default_path = "/".to_string();
    let return_path = query.get("return_to").unwrap_or(&default_path);

    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        let comments = data.comments.get(&slug);
        let reply = warp::reply::html(
            data.hbs.render(
                "article",
                &json!({
                    "title": (article.title.clone() + " &middot ") + title,
                    "article": article,
                    "comments": comments,
                    "prev": article.prev,
                    "next": article.next,
                    "return_path": return_path,
                    "body_class": "article",
                })
            ).expect("Failed to render article with Handlebars")
        );

        log::info!("Rendered article `{}` in {}ms", &slug, now.elapsed().as_millis());
        Ok(warp::reply::with_status(reply, StatusCode::OK))
    } else {
        // let reply = warp::reply::html(String::from("Unable to read article"));
        // Ok(warp::reply::with_status(reply, StatusCode::INTERNAL_SERVER_ERROR))
        Err(warp::reject::not_found())
    }
}

pub async fn comment_route(
    slug: String,
    mut form_data: HashMap<String, String>,
    addr: Option<SocketAddr>,
    data: Arc<Mutex<CommonData>>
) -> warp::reply::WithStatus<warp::reply::Html<String>> {
    let (text, author, author_url) = (
        form_data.remove("text"),
        form_data.remove("author"),
        form_data.remove("author_url")
    );

    if let (Some(text), Some(author), Some(author_url)) = (text, author, author_url) {
        let mut data = data.lock().unwrap();
        let comment = Comment {
            text, author, author_url,
            timestamp: create_timestamp(),
        };
        if let Ok(saved) = data.comments.add(&slug, comment, addr) {
            let reply = warp::reply::html(
                data.hbs.render("comment", &saved).expect("Render comment")
            );
            log::info!("Saved comment on article '{}'", &slug);
            warp::reply::with_status(reply, StatusCode::OK)
        } else {
            let reply = warp::reply::html("failed to save comment".to_string());
            warp::reply::with_status(reply, StatusCode::INTERNAL_SERVER_ERROR)
        }

    } else {
        let reply = warp::reply::html("invalid comment".to_string());
        warp::reply::with_status(reply, StatusCode::BAD_REQUEST)
    }


}

pub async fn file_not_found_route(_: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let error_page = fs::read_to_string("content/errors/404.html").unwrap();
    let reply = warp::reply::html(error_page);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::NOT_FOUND))
}

fn create_timestamp() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        // i64 is enough milliseconds for 292 million years,so coercing it like
        // this is probably fine.
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64)
    }
}
