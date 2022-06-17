use std::sync::Arc;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::io::{self, ErrorKind};
use std::{fs, time, cmp};
use std::collections::BTreeMap;
use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde_json::json;
use warp::Filter;
use handlebars::{Handlebars, handlebars_helper};
use chrono::prelude::*;
use pulldown_cmark as cmark;
use redis;
use redis::Commands;
use regex::Regex;

#[macro_use]
extern crate lazy_static;

const PAGE_SIZE: usize = 10;
const BASE_PATH: &str = "./content";
const REDIS_HOST: &str = "redis://127.0.0.1/";
const BASE_KEY: &str = "velum:articles:";
const BLOG_TITLE: &str = "Velum Test Blog";
const DEFAULT_TITLE: &str = "<no title>";
const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;

// Struct for creating and managing article data
struct Article {
    content: String,
    timestamp_millis: i64,
}

impl Article {
    fn from_file(path: &PathBuf) -> Result<Self, io::Error> {
        let metadata = fs::metadata(path)?;
        let content = fs::read_to_string(path)?;
        let created = metadata.created()?;
        if let Ok(s) = created.duration_since(UNIX_EPOCH) {
            Ok(Self {
                content,
                timestamp_millis: s.as_millis() as i64
            })
        } else {
            Err(io::Error::new(ErrorKind::Other, "failed to read file"))
        }
    }

    fn from_redis(a: &BTreeMap<String, String>) -> Self {
        let timestamp = a.get("timestamp").unwrap();
        Self {
            content: a.get("content").unwrap().to_string(),
            timestamp_millis: timestamp.parse::<i64>().unwrap_or(0),
        }
    }

    fn parsed_content(&self) -> String {
        let mut parsed_article = String::new();
        let parser = cmark::Parser::new(&self.content);
        cmark::html::push_html(&mut parsed_article, parser);
        parsed_article
    }

    fn title(&self) -> Option<String> {
        lazy_static! { static ref H1: Regex = Regex::new(r"^#\s*").unwrap(); }
        // Assumes first line of content text is formatted exactly as '# Article Title'
        if let Some(l) = self.content.lines().nth(0) {
            Some(String::from(
                H1.replace(l, "")
            ))
        } else {
            None
        }
    }

    fn slug(&self) -> Result<String, &'static str> {
        lazy_static! { static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z0-9\-]").unwrap(); }
        lazy_static! { static ref SEQUENTIAL_HYPEHNS: Regex = Regex::new(r"-+").unwrap(); }
        if let Some(t) = self.title() {
            let lowercase_title = t.to_lowercase();
            let simplified_key = INVALID_CHARS.replace_all(&lowercase_title, "-");
            Ok(String::from(
                SEQUENTIAL_HYPEHNS.replace_all(&simplified_key, "-")
            ))
        } else {
            Err("Unable to create key because artitcle has no title.")
        }
    }

    fn route(&self) -> Result<String, &'static str> {
        if let Ok(slug) = self.slug() {
            Ok(String::from("/articles/") + &slug)
        } else {
            Err("Unable to create route due to error in slug")
        }
    }

    // For passing to Redis via hset_multiple
    fn to_kv_list(&self) -> Box<[(String, String)]> {
        Box::new([
            ("content".to_string(), self.content.clone()),
            ("timestamp".to_string(), self.timestamp_millis.to_string()),
        ])
    }
}

impl Serialize for Article {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut state = serializer.serialize_struct("Article", 4)?;
        let title = &self.title().unwrap_or(String::from(DEFAULT_TITLE));
        state.serialize_field("title", title)?;
        state.serialize_field("content", &self.parsed_content())?;
        state.serialize_field("route", &self.route().unwrap_or("/".to_string()))?;
        state.serialize_field("timestamp", &self.timestamp_millis)?;
        state.end()
    }
}

// TODO: friendlier date format, e.g. "3 months ago on 23rd May 2022"
handlebars_helper!(date_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    dt.to_rfc2822()
});

handlebars_helper!(page_display: |current: usize, max: usize| {
    format!("Page {} of {}", current + 1, max + 1)
});

#[derive(Debug)]
struct ArticleNotFound;
impl warp::reject::Reject for ArticleNotFound {}

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

fn gather_redis_articles() -> Result<Vec<Article>, redis::RedisError> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;
    let mut articles: Vec<Article> = Vec::new();
    let keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        if let Ok(result) = con.hgetall(key) {
            articles.push(Article::from_redis(&result))
        }
    }

    articles.sort_by_key(|a| -a.timestamp_millis);

    Ok(articles)
}

fn destroy_article_keys(con: &mut redis::Connection) -> redis::RedisResult<()> {
    let keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }

    Ok(())
}

fn rebuild_redis_data() -> redis::RedisResult<()> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;

    destroy_article_keys(&mut con)?;

    // TODO: handle potential failure
    if let Ok(articles) = gather_fs_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                con.hset_multiple(&key, &article.to_kv_list())?;
            }
        }
    }
    Ok(())
}

fn render_index_page(page: usize, hbs: &Handlebars<'_>) -> String {
    if let Ok(articles) = gather_redis_articles() {
        let pages: Vec<&[Article]> = articles.chunks(PAGE_SIZE).collect();
        let max_page = pages.len().checked_sub(1).unwrap_or(0);
        let constrained_page = cmp::min(max_page, page);

        match hbs.render(
            "main",
            &json!({
                "title": BLOG_TITLE,
                "prev_page": page.checked_sub(1).unwrap_or(0),
                "current_page": page,
                "max_page": max_page,
                "next_page": if page < max_page { page + 1 } else { 0 },
                "articles": &pages[constrained_page]
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

    if let Ok(result) = con.hgetall(String::from(BASE_KEY) + &slug) {
        let article = Article::from_redis(&result);
        let reply = warp::reply::html(
            hbs.render(
                "article",
                &json!(article)
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
    hb.register_template_file(
        "article",
        &article_tmpl_path
    ).expect("Failed to register article template file");
    hb.register_template_file(
        "main",
        &index_tmpl_path
    ).expect("Failed to register index template file");
    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));
    hb.register_helper("page_display", Box::new(page_display));

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
        index_at_offset(0, &hbs2)
    });
    let hbs3 = hbs.clone();
    let article_index_offset = warp::path!("page" / usize).map(move |page| {
        index_at_offset(page, &hbs3)
    });

    let article = warp::path!("articles" / String).map(move |article| {
        render_article(article, &hbs)
    });

    let images = warp::path("images").and(warp::fs::dir("content/images"));
    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index.
        or(article_index_offset).
        or(article).
        or(images).
        or(assets).
        recover(file_not_found);

    println!("Listening on 3090...");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
