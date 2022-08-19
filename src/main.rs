mod article;
mod hb;

use std::sync::Arc;
use std::collections::HashMap;
use std::convert::Infallible;
use std::{fs, time};
use serde_json::json;
use warp::Filter;
use handlebars::Handlebars;
use config::Config;
use article::view::ContentView;
use article::builder::ParseError;
use article::storage::{
    LinkList,
    gather_fs_articles,
    fetch_by_slug,
    fetch_index_links,
};
use hb::create_handlebars;

#[macro_use] extern crate lazy_static;

const CONFIG_FILE: &str = "Settings"; // .toml is implied
const DEFAULT_PAGE_SIZE: usize = 5;
const DEFAULT_TITLE: &str = "Velum Blog";

fn load_config() -> Config {
    Config::builder()
        .add_source(config::File::with_name(CONFIG_FILE))
        .build()
        .expect("Failed to build config")
}

type InfResult<T> = Result<T, Infallible>;

#[derive(Clone)]
pub struct CommonData {
    hbs: Handlebars<'static>,
    articles: Vec<ContentView>,
    config: Config,
}

impl CommonData {
    fn new() -> Self {
        let config = load_config();
        let articles = gather_fs_articles(&config).expect("gather FS articles");
        Self {
            hbs: create_handlebars(&config),
            articles,
            config,
        }
    }

    fn rebuild(&mut self) -> Result<(), ParseError> {
        gather_fs_articles(&self.config)
            .and_then(|articles| {
                self.articles = articles;
                Ok(())
            })
    }

    fn page_size(&self) -> usize {
        self.config
            .get_int("page_size")
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize
    }
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

fn error_response(msg: String) -> Result<warp::reply::WithStatus<warp::reply::Html<String>>, Infallible> {
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
    } else {
        if page <= 1 {
            "/".to_string()
        } else {
            format!("/index/{}", page)
        }
    }
}

fn render_article_list(
    article_list: LinkList,
    page: usize,
    page_size: usize,
    data: &CommonData,
    tag: Option<&str>,
) -> InfResult<warp::reply::WithStatus<warp::reply::Html<String>>> {
    let title = data.config
        .get_string("blog_title")
        .unwrap_or(DEFAULT_TITLE.to_owned());

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

async fn index_page_route(page: usize, data: Arc<CommonData>) -> InfResult<impl warp::Reply> {
    let now = time::Instant::now();
    let page_size = data.page_size();
    let article_list = fetch_index_links(page, page_size, None, &data.articles);
    let response = render_article_list(article_list, page, page_size, &data, None);
    if response.is_ok() {
        log::info!("Rendered article index page {} in {}ms", page, now.elapsed().as_millis());
    }

    response
}

async fn tag_search_route(tag: String, page: usize, data: Arc<CommonData>) -> InfResult<impl warp::Reply> {
    let now = time::Instant::now();
    let page_size = data.page_size();
    let article_result = fetch_index_links(page, page_size, Some(&tag), &data.articles);
    let response = render_article_list(article_result, page, page_size, &data, Some(&tag));
    if response.is_ok() {
        log::info!("Rendered tag '{}' index page {} in {}ms", &tag, page, now.elapsed().as_millis());
    }
    response
}

async fn article_route(slug: String, query: HashMap<String, String>, data: Arc<CommonData>) -> InfResult<impl warp::Reply> {
    let now = time::Instant::now();
    let title = data.config
        .get_string("blog_title")
        .unwrap_or(DEFAULT_TITLE.to_owned());

    let default_path = "/".to_string();
    let return_path = query.get("return_to").unwrap_or(&default_path);

    if let Some(article) = fetch_by_slug(&slug, &data.articles) {
        let reply = warp::reply::html(
            data.hbs.render(
                "article",
                &json!({
                    "title": (article.title.clone() + " &middot ") + &title,
                    "article": article,
                    "prev": article.prev,
                    "next": article.next,
                    "return_path": return_path,
                })
            ).expect("Failed to render article with Handlebars")
        );

        log::info!("Rendered article `{}` in {}ms", &slug, now.elapsed().as_millis());
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

    let now = time::Instant::now();
    log::info!("Building article data from files... ");
    let codata = Arc::new(CommonData::new());
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    // This needs to be assined after rebuild, so we can transfer ownership into the lambda
    let codata_filter = warp::any().map(move || codata.clone());

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
