mod commondata;
mod article;
mod hb;
mod comments;
mod errors;
mod routes;

use std::sync::{Arc, Mutex};
use std::time;
use std::net::IpAddr;
use std::collections::HashMap;
use warp::Filter;
use commondata::{CommonData, load_config};
use routes::{
    index_page_route,
    tag_search_route,
    article_route,
    comment_route,
    file_not_found_route,
};

#[macro_use] extern crate lazy_static;

const DEFAULT_LISTEN_IP: &str = "127.0.0.1";
const DEFAULT_LISTEN_PORT: u16 = 3090;

#[tokio::main]
async fn main() {
    env_logger::init();

    let now = time::Instant::now();
    log::info!("Building article and comment data from files... ");
    let codata = Arc::new(Mutex::new(CommonData::new()));
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    // This needs to be assined after rebuild, so we can transfer ownership into the lambda
    let codata_filter = warp::any()
        .map(move || codata.clone());

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
        .and(warp::query::<HashMap<String, String>>())
        .and(codata_filter.clone())
        .and_then(article_route);

    let legacy_article = warp::path!(String)
        .and(warp::query::<HashMap<String, String>>())
        .and(codata_filter.clone())
        .and_then(article_route);

    let comment = warp::path!("comment" / String)
        .and(warp::body::content_length_limit(4000))
        .and(warp::filters::body::form())
        .and(warp::filters::addr::remote())
        .and(codata_filter.clone())
        .and(warp::post())
        .then(comment_route);

    // TODO: change hard-coded content dir() to use the one from config
    // can't use path! macro because it ends the path
    let images = warp::path("content")
        .and(warp::path("images"))
        .and(warp::fs::dir("content/images"));

    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let errorlogger = warp::filters::log::custom(|info| {
        let s = info.status();
        let msg = format!(
            "{} `{}` {}",
            info.method(),
            info.path(),
            info.status()
        );
        if s.is_client_error() {
            log::info!("{}", msg);
        } else if s.is_server_error() {
            log::error!("{}", msg);
        }
    });

    let routes = article_index
        .or(article_index_at_page)
        .or(article)
        .or(articles_with_tag)
        .or(articles_with_tag_at_page)
        .or(comment)
        .or(images)
        .or(assets)
        .or(legacy_article)
        .recover(file_not_found_route)
        .with(errorlogger);

    let config = load_config();
    let listen_ip = config
        .get_string("listen_ip")
        .unwrap_or_else(|_| DEFAULT_LISTEN_IP.to_string());
    let listen_ip = listen_ip.parse::<IpAddr>()
        .unwrap_or_else(|_| panic!("Failed to parse listen IP from {}", listen_ip));
    let listen_port = config
        .get_int("listen_port")
        .unwrap_or(DEFAULT_LISTEN_PORT as i64) as u16;

    warp::serve(routes)
        .run((listen_ip, listen_port))
        .await;
}
