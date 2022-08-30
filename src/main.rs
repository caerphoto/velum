mod commondata;
mod article;
mod hb;
mod comments;
mod errors;
mod routes;

use std::sync::{Arc, Mutex};
use std::time;
use std::collections::HashMap;
use warp::Filter;
use commondata::CommonData;
use routes::{
    index_page_route,
    tag_search_route,
    article_route,
    comment_route,
    file_not_found_route,
};

#[macro_use] extern crate lazy_static;

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

    let comment = warp::path!("comment" / String)
        .and(warp::body::content_length_limit(4000))
        .and(warp::filters::body::form())
        .and(warp::filters::addr::remote())
        .and(codata_filter.clone())
        .and(warp::post())
        .then(comment_route);

    let images = warp::path("images").and(warp::fs::dir("content/images"));
    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));


    let routes = article_index
        .or(article_index_at_page)
        .or(article)
        .or(articles_with_tag)
        .or(articles_with_tag_at_page)
        .or(comment)
        .or(images)
        .or(assets)
        .recover(file_not_found_route);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
