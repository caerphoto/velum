mod article;

use std::sync::Arc;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::{fs, cmp, time};
use log::{info};
use serde_json::json;
use warp::Filter;
use handlebars::{Handlebars, handlebars_helper};
use chrono::prelude::*;
use article::view::{gather_article_links, ArticleView, ArticleViewLink};
use article::storage::rebuild_redis_data;
use ordinal::Ordinal;

#[macro_use] extern crate lazy_static;

pub const BASE_PATH: &str = "./content";
const PAGE_SIZE: usize = 10;
const BLOG_TITLE: &str = "Velum Test Blog";


// TODO: friendlier date format, e.g. "3 months ago on 23rd May 2022"
handlebars_helper!(date_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    format!("{} {} {}",
        dt.format("%A"), // Day
        Ordinal(dt.day()), // Date
        dt.format("%B %Y") // Month, year, time
    )
});

fn error_response(msg: String) -> Result<warp::reply::WithStatus<warp::reply::Html<String>>, Infallible> {
    let reply = warp::reply::html(msg);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::INTERNAL_SERVER_ERROR))
}

async fn render_index_page(page: usize, hbs: Arc<Handlebars<'_>>) -> Result<impl warp::Reply, Infallible> {
    let now = time::Instant::now();

    match gather_article_links() {
        Ok(articles) => {
            let pages: Vec<&[ArticleViewLink]> = articles.chunks(PAGE_SIZE).collect();
            let max_page = pages.len();
            let chunk_index = cmp::min(max_page, page.saturating_sub(1));

            match hbs.render(
                "main",
                &json!({
                    "title": BLOG_TITLE,
                    "prev_page": if page > 1 { page - 1 } else { 0 },
                    "current_page": page,
                    "next_page": if page < max_page { page + 1 } else { 0 },
                    "max_page": max_page,
                    "articles": &pages[chunk_index]
                })
            ) {
                Ok(rendered_page) => {
                    info!("Rendered index page {} in {}ms", page, now.elapsed().as_millis());
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
        Err(e) => error_response(format!("Failed to fetch articles list. Error: {:?}", e))
    }

}

async fn render_article(slug: String, hbs: Arc<Handlebars<'_>>) -> Result<impl warp::Reply, Infallible> {
    let now = time::Instant::now();

    if let Some(article) = ArticleView::from_slug(&slug) {
        let reply = warp::reply::html(
            hbs.render(
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

async fn file_not_found(_: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let error_page = fs::read_to_string("content/errors/404.html").unwrap();
    let reply = warp::reply::html(error_page);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::NOT_FOUND))
}

fn tmpl_path(tmpl_name: &str) -> PathBuf {
    let filename = [tmpl_name, "html.hbs"].join(".");
    let path = Path::new(BASE_PATH).join("templates");
    path.join(filename)
}

fn create_handlebars() -> Handlebars<'static> {
    let mut hb = Handlebars::new();
    let index_tmpl_path = tmpl_path("index");
    let article_tmpl_path = tmpl_path("article");
    let header_tmpl_path = tmpl_path("_header");
    let footer_tmpl_path = tmpl_path("_footer");

    hb.set_dev_mode(true);

    hb.register_template_file("article", &article_tmpl_path)
        .expect("Failed to register article template file");
    hb.register_template_file("main", &index_tmpl_path)
        .expect("Failed to register index template file");
    hb.register_template_file("header", &header_tmpl_path)
        .expect("Failed to register header template file");
    hb.register_template_file("footer", &footer_tmpl_path)
        .expect("Failed to register footer template file");
    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));

    hb
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let hbs = Arc::new(create_handlebars());
    let hbs_filter = warp::any().map(move || hbs.clone());

    info!("Rebuilding Redis data from files... ");
    if let Err(e) = rebuild_redis_data() {
        panic!("Failed to rebuild Redis data: {:?}", e);
    }
    info!("...done.");

    let article_index = warp::path::end().map(|| 1usize)
        .and(hbs_filter.clone())
        .and_then(render_index_page);

    let article_index_page = warp::path!("page" / usize)
        .and(hbs_filter.clone())
        .and_then(render_index_page);

    let article = warp::path!("articles" / String)
        .and(hbs_filter.clone())
        .and_then(render_article);

    let images = warp::path("images").and(warp::fs::dir("content/images"));
    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index
        .or(article_index_page)
        .or(article)
        .or(images)
        .or(assets)
        .recover(file_not_found);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
