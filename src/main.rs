mod article;

use std::sync::{Arc, Mutex};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::{fs, time};
use log::{info};
use serde_json::json;
use warp::Filter;
use handlebars::{Handlebars, handlebars_helper};
use chrono::prelude::*;
use article::view::{ArticleView, ArticleViewLink};
use article::storage::{REDIS_HOST, rebuild_redis_data, fetch_article_links, fetch_by_tag};
use ordinal::Ordinal;

#[macro_use] extern crate lazy_static;

pub const BASE_PATH: &str = "./content";
const PAGE_SIZE: usize = 10;
const BLOG_TITLE: &str = "Velum Test Blog";

#[derive(Clone)]
struct CommonData {
    hbs: Handlebars<'static>,
    con: Arc<Mutex<redis::Connection>>
}

impl CommonData {
    fn new() -> Self {
        Self {
            hbs: create_handlebars(),
            con: Arc::new(Mutex::new(get_redis_connection())),
        }
    }
}

fn tmpl_path(tmpl_name: &str) -> PathBuf {
    let filename = [tmpl_name, "html.hbs"].join(".");
    let path = Path::new(BASE_PATH).join("templates");
    path.join(filename)
}

// TODO: friendlier date format, e.g. "3 months ago on 23rd May 2022"
handlebars_helper!(date_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    format!("{} {} {}",
        dt.format("%A"), // Day
        Ordinal(dt.day()), // Date
        dt.format("%B %Y") // Month, year, time
    )
});

handlebars_helper!(is_current_tag: |this_tag: String, search_tag: String| {
    this_tag == search_tag
});

fn create_handlebars() -> Handlebars<'static> {
    let mut hb = Handlebars::new();
    let index_tmpl_path = tmpl_path("index");
    let article_tmpl_path = tmpl_path("article");
    let tag_list_tmpl_path = tmpl_path("_tag_list");
    let header_tmpl_path = tmpl_path("_header");
    let footer_tmpl_path = tmpl_path("_footer");

    hb.set_dev_mode(true);

    hb.register_template_file("main", &index_tmpl_path)
        .expect("Failed to register index template file");
    hb.register_template_file("article", &article_tmpl_path)
        .expect("Failed to register article template file");
    hb.register_template_file("tag_list", &tag_list_tmpl_path)
        .expect("Failed to register tag_list template file");
    hb.register_template_file("header", &header_tmpl_path)
        .expect("Failed to register header template file");
    hb.register_template_file("footer", &footer_tmpl_path)
        .expect("Failed to register footer template file");

    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));
    hb.register_helper("is_current_tag", Box::new(is_current_tag));

    hb
}

fn get_redis_connection() -> redis::Connection {
    let client = redis::Client::open(REDIS_HOST).unwrap();
    let con = client.get_connection().unwrap();
    con
}

fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 && rhs > 0 {
        d + 1
    } else {
        d
    }
}

fn error_response(msg: String) -> Result<warp::reply::WithStatus<warp::reply::Html<String>>, Infallible> {
    let reply = warp::reply::html(msg);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::INTERNAL_SERVER_ERROR))
}

fn render_article_list(
    article_result: redis::RedisResult<(Vec<ArticleViewLink>, usize)>,
    page: usize,
    hbs: &Handlebars,
    tag: Option<&str>,
) -> Result<warp::reply::WithStatus<warp::reply::Html<String>>, Infallible> {

    match article_result {
        Ok((articles, all_count)) => {
            let max_page = div_ceil(all_count, PAGE_SIZE);
            match hbs.render(
                "main",
                &json!({
                    "title": BLOG_TITLE,
                    "prev_page": if page > 1 { page - 1 } else { 0 },
                    "current_page": page,
                    "next_page": if page < max_page { page + 1 } else { 0 },
                    "max_page": max_page,
                    "search_tag": tag.unwrap_or(""),
                    "article_count": all_count,
                    "articles": &articles
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
        },
        Err(e) => error_response(format!("Unable to fetch artile links. Error: {:?}", e))
    }
}

async fn index_page_route(page: usize, data: Arc<CommonData>) -> Result<impl warp::Reply, Infallible> {
    let now = time::Instant::now();
    let mut con = data.con.lock().unwrap();
    let article_result = fetch_article_links(page, PAGE_SIZE, &mut con);
    let response = render_article_list(article_result, page, &data.hbs, None);
    if response.is_ok() {
        info!("Rendered article index page {} in {}ms", page, now.elapsed().as_millis());
    }
    response
}

async fn tag_search_route(tag: String, page: usize, data: Arc<CommonData>) -> Result<impl warp::Reply, Infallible> {
    let now = time::Instant::now();
    let mut con = data.con.lock().unwrap();
    let article_result = fetch_by_tag(&tag, page, PAGE_SIZE, &mut con);
    let response = render_article_list(article_result, page, &data.hbs, Some(&tag));
    if response.is_ok() {
        info!("Rendered tag index page {} in {}ms", page, now.elapsed().as_millis());
    }
    response
}

async fn article_route(slug: String, data: Arc<CommonData>) -> Result<impl warp::Reply, Infallible> {
    let now = time::Instant::now();
    let mut con = data.con.lock().unwrap();

    if let Some(article) = ArticleView::from_slug(&slug, &mut con) {
        let reply = warp::reply::html(
            data.hbs.render(
                "article",
                &json!({
                    "title": (article.title.clone() + " &middot ") + BLOG_TITLE,
                    "article": article,
                    "prev": article.prev,
                    "next": article.next
                })
            ).expect("Failed to render article with Handlebars")
        );

        info!("Rendered article `{}` in {}ms", &slug, now.elapsed().as_millis());
        Ok(warp::reply::with_status(reply, warp::http::StatusCode::OK))
    } else {
        let reply = warp::reply::html(String::from("Unable to read article"));
        Ok(warp::reply::with_status(reply, warp::http::StatusCode::INTERNAL_SERVER_ERROR))
    }
}

async fn file_not_found_route(_: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let error_page = fs::read_to_string("content/errors/404.html").unwrap();
    let reply = warp::reply::html(error_page);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::NOT_FOUND))
}


#[tokio::main]
async fn main() {
    env_logger::init();

    let codata = Arc::new(CommonData::new());
    let codata_filter = warp::any().map(move || codata.clone());

    info!("Rebuilding Redis data from files... ");
    if let Err(e) = rebuild_redis_data() {
        panic!("Failed to rebuild Redis data: {:?}", e);
    }
    info!("...done.");

    let article_index = warp::path::end().map(|| 1usize)
        .and(codata_filter.clone())
        .and_then(index_page_route);

    let article_index_at_page = warp::path!("index" / usize)
        .and(codata_filter.clone())
        .and_then(index_page_route);

    let articles_with_tag = warp::path!("tag" / String)
        .map(|tag: String| (tag, 1) )
        .untuple_one()
        .and(codata_filter.clone())
        .and_then(tag_search_route);

    let articles_with_tag_at_page = warp::path!("tag" / String / usize)
        .and(codata_filter.clone())
        .and_then(tag_search_route);

    let article = warp::path!("articles" / String)
        .and(codata_filter.clone())
        .and_then(article_route);

    let images = warp::path("images").and(warp::fs::dir("content/images"));
    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index
        .or(article_index_at_page)
        .or(article)
        .or(articles_with_tag)
        .or(articles_with_tag_at_page)
        .or(images)
        .or(assets)
        .recover(file_not_found_route);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
