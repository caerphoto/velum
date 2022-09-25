mod admin;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::{fs, time::{self, SystemTime, UNIX_EPOCH}};
use std::net::SocketAddr;
use std::convert::Infallible;
use regex::Regex;
use std::path::PathBuf;
use warp::{Reply, http::Uri};
use mime_guess;
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

pub type SharedData = Arc<Mutex<CommonData>>;

pub type WarpResult = Result<
    warp::reply::Response,
    warp::reject::Rejection
>;

const INTERNAL_SERVER_ERROR: u16 = 500;
pub const BAD_REQUEST: u16 = 400;

pub fn server_error(msg: &str) -> WarpResult {
    log::error!("{}", msg);
    Ok(warp::http::Response::builder()
        .status(INTERNAL_SERVER_ERROR)
        .body("Internal server error :(".into())
        .unwrap()
    )
}

pub fn redirect_to(path: &'static str) -> WarpResult {
    Ok(warp::redirect::found(Uri::from_static(path)).into_response())
}

pub fn empty_response(status: u16) -> WarpResult {
    Ok(
        warp::reply::with_status(
            warp::reply(),
            warp::http::StatusCode::from_u16(status).unwrap()
        ).into_response()
    )
}

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

fn create_timestamp() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        // i64 is enough milliseconds for 292 million years, so coercing it like
        // this is probably fine.
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64)
    }
}

fn render_article_list(
    article_list: LinkList,
    page: usize,
    page_size: usize,
    data: &CommonData,
    tag: Option<&str>,
) -> WarpResult {
    let blog_title = &data.config.blog_title;
    let max_page = div_ceil(article_list.total_articles, page_size);

    let title = if let Some(tag) = tag {
        String::from("Tag: ") + tag
    } else {
        String::from("Article Index")
    };

    match data.hbs.render(
        "index",
        &json!({
            "blog_title": blog_title,
            "title": title,
            "prev_page": if page > 1 { page - 1 } else { 0 },
            "current_page": page,
            "next_page": if page < max_page { page + 1 } else { 0 },
            "max_page": max_page,
            "search_tag": tag.unwrap_or(""),
            "article_count": article_list.total_articles,
            "articles": &article_list.index_views,
            "body_class": if tag.is_some() { "tag-index" } else { "index" },
            "content_dir": &data.config.content_dir,
        })
    ) {
        Ok(rendered_page) => {
            Ok(warp::reply::html(rendered_page).into_response())
        },
        Err(e) => {
            server_error(&format!("Failed to render article in index. Error: {:?}", e))
        }
    }
}

pub async fn index_page_route(page: usize, data: SharedData) -> WarpResult {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;
    let article_list = fetch_index_links(page, page_size, None, &data.articles);
    let response = render_article_list(
        article_list,
        page,
        page_size,
        &data,
        None
    );
    log::info!(
        "Rendered article index page {} in {}µs",
        page,
        now.elapsed().as_micros()
    );

    response
}

pub async fn tag_search_route(tag: String, page: usize, data: SharedData) -> WarpResult {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let page_size = data.config.page_size;
    let article_result = fetch_index_links(page, page_size, Some(&tag), &data.articles);
    let response = render_article_list(
        article_result,
        page,
        page_size,
        &data,
        Some(&tag)
    );
    log::info!(
        "Rendered tag '{}' index page {} in {}µs",
        &tag,
        page,
        now.elapsed().as_micros()
    );
    response
}

pub async fn article_text_route(slug: String, data: SharedData) -> WarpResult {
    let data = data.lock().unwrap();
    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        Ok(warp::http::Response::builder()
            .status(200)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(article.base_content.clone().into())
            .unwrap()
        )
    } else {
        Err(warp::reject::not_found())
    }
}

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

pub async fn article_route(slug: String, referer: Option<String>, data: SharedData) -> WarpResult {
    let now = time::Instant::now();
    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;

    let return_path = return_path(&data.config.blog_host, referer);

    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        let comments = data.comments.get(&slug);
        let reply = Ok(warp::reply::html(
            data.hbs.render(
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
                })
            ).expect("Failed to render article with Handlebars")
        ).into_response());

        log::info!(
            "Rendered article `{}` in {}µs",
            &slug,
            now.elapsed().as_micros()
        );
        reply
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
    data: SharedData
) -> WarpResult {
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
            let reply = Ok(warp::reply::html(
                data.hbs.render("comment", &saved).expect("Render comment")
            ).into_response());
            log::info!("Saved comment on article '{}'", &slug);
            reply
        } else {
            server_error("failed to save comment")
        }

    } else {
        empty_response(BAD_REQUEST)
    }
}

pub async fn timestamped_asset_route(timestamped_name: String, data: SharedData) -> WarpResult {
    lazy_static! {
        static ref DATE_PART: Regex = Regex::new(r"-\d{14}\.").unwrap();
    }

    if !DATE_PART.is_match(&timestamped_name) { return Err(warp::reject::not_found()) }

    let data = data.lock().unwrap();
    let new_name = DATE_PART.replace(&timestamped_name, ".").to_string();
    let real_path = PathBuf::from(&data.config.content_dir)
        .join("assets")
        .join(&new_name);
    log::info!("Serving timestamped assset {} from file {}", &timestamped_name, &real_path.to_string_lossy());
    if let Ok(content) = fs::read_to_string(real_path) {
        let ct = mime_guess::from_path(&new_name).first_or_text_plain();
        Ok(warp::http::Response::builder()
            .status(200)
            .header("Content-Type", ct.as_ref())
            .body(content.into())
            .unwrap()
        )
    } else {
        Err(warp::reject::not_found())
    }
}

pub async fn file_not_found_route(_: warp::Rejection) -> Result<warp::reply::Response, Infallible> {
    let error_page = fs::read_to_string("content/errors/404.html").unwrap();
    let reply = warp::reply::html(error_page);
    Ok(
        warp::reply::with_status(reply, warp::http::StatusCode::NOT_FOUND)
        .into_response()
    )
}

