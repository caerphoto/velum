mod article;
mod article_view;

use std::sync::Arc;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::io::{self, ErrorKind};
use std::{fs, time, cmp};
use serde_json::json;
use warp::Filter;
use handlebars::{Handlebars, handlebars_helper};
use chrono::prelude::*;
use redis::Commands;
use crate::article::Article;
use crate::article_view::{ArticleView, article_keys, BASE_KEY, BASE_TS_KEY};

#[macro_use]
extern crate lazy_static;

const PAGE_SIZE: usize = 10;
const BASE_PATH: &str = "./content";
const REDIS_HOST: &str = "redis://127.0.0.1/";
const BLOG_TITLE: &str = "Velum Test Blog";


// TODO: friendlier date format, e.g. "3 months ago on 23rd May 2022"
handlebars_helper!(date_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    dt.format("%A %e %B %Y at %H:%M").to_string()
});

fn gather_fs_articles() -> Result<Vec<Article>, io::Error> {
    let dir = PathBuf::from(BASE_PATH).join("articles");
    if !dir.is_dir() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Article path is not a directory"));
    }

    let mut articles: Vec<Article> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            if let Ok(article) = Article::from_file(&path) {
                articles.push(article);
            }
        }
    }
    Ok(articles)
}

fn gather_redis_article_views() -> Result<Vec<ArticleView>, redis::RedisError> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;
    let mut articles: Vec<ArticleView> = Vec::new();
    let keys = article_keys(&mut con)?;
    for key in keys {
        if let Ok(result) = con.hgetall(key) {
            articles.push(ArticleView::from_redis(&result, true))
        }
    }

    articles.sort_by_key(|a| -a.timestamp);

    Ok(articles)
}

fn destroy_all_keys(con: &mut redis::Connection) -> redis::RedisResult<()> {
    let mut keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }
    keys = con.keys(String::from(BASE_TS_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }

    Ok(())
}

fn rebuild_redis_data() -> redis::RedisResult<()> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;

    destroy_all_keys(&mut con)?;

    // TODO: handle potential failure
    if let Ok(articles) = gather_fs_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                con.hset_multiple(&key, &article.to_kv_list())?;
                let ts_key = String::from(BASE_TS_KEY) + slug.as_str();
                con.set(ts_key, article.timestamp)?;
            }
        }
    }
    Ok(())
}

fn render_index_page(page: usize, hbs: &Handlebars<'_>) -> String {
    if let Ok(articles) = gather_redis_article_views() {
        let pages: Vec<&[ArticleView]> = articles.chunks(PAGE_SIZE).collect();
        let max_page = pages.len();
        let chunk_index = cmp::min(max_page, page.checked_sub(1).unwrap_or(0));

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
            Ok(rendered_page) => rendered_page,
            Err(e) => format!("Error rendering page {:?}", e),
        }
    } else {
        String::from("")
    }

}

fn index_at_offset(offset: usize, hbs: &Handlebars<'_>) -> impl warp::Reply {
    warp::reply::html(render_index_page(offset, hbs))
}

fn render_article(slug: String, hbs: &Handlebars<'_>) -> impl warp::Reply {
    let client = redis::Client::open(REDIS_HOST).unwrap();
    let mut con = client.get_connection().unwrap();
    let key = String::from(BASE_KEY) + &slug;

    if let Some(article) = ArticleView::from_redis_key(&key, &mut con, true) {
        let (prev, next) = article.surrounding(&mut con);

        let reply = warp::reply::html(
            hbs.render(
                "article",
                &json!({
                    "title": (article.title.clone() + " &middot ") + BLOG_TITLE,
                    "article": article,
                    "prev": prev,
                    "next": next
                })
            ).expect("Failed to render article")
        );

        warp::reply::with_status(reply, warp::http::StatusCode::OK)
    } else {
        let reply = warp::reply::html(String::from("Unable to read article"));
        warp::reply::with_status(reply, warp::http::StatusCode::INTERNAL_SERVER_ERROR)
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
    pretty_env_logger::init();

    let hbs = Arc::new(create_handlebars());

    print!("Rebuilding Redis data from files... ");
    if let Err(e) = rebuild_redis_data() {
        panic!("Failed to rebuild Redis data: {:?}", e);
    }
    println!("done.");

    let hbs2 = hbs.clone();
    let article_index = warp::path::end().map(move || {
        index_at_offset(1, &hbs2)
    });
    let hbs3 = hbs.clone();
    let article_index_offset = warp::path!("page" / usize).map(move |page| {
        index_at_offset(page, &hbs3)
    });

    let article = warp::path!("articles" / String).map(move |article_slug: String| {
        let now = time::Instant::now();
        let res = render_article(article_slug.clone(), &hbs);
        println!("Rendered article \"{}\" in {} ms", article_slug, now.elapsed().as_millis());
        res
    });

    let images = warp::path("images").and(warp::fs::dir("content/images"));
    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index
        .or(article_index_offset)
        .or(article)
        .or(images)
        .or(assets)
        .recover(file_not_found);

    println!("Listening on 3090...");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
