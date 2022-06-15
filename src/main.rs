use std::sync::Arc;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::io::{self, ErrorKind};
use std::{fs, time, cmp};
use std::collections::BTreeMap;
// use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde_json::json;
use warp::Filter;
use handlebars::Handlebars;
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

// Struct for creating article data from a file on disk.
struct FsArticle {
    content: String,
    created_at: time::SystemTime
}

impl FsArticle {
    fn from_file(path: &PathBuf) -> Result<Self, io::Error> {
        let content = fs::read_to_string(path)?;
        Ok(Self {
            content,
            created_at: time::SystemTime::now(),
        })
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
        lazy_static! { static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z\-]").unwrap(); }
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

    fn age(&self) -> time::Duration {
        let now = time::SystemTime::now();
        match now.duration_since(self.created_at) {
            Ok(d) => d,
            Err(_) => time::Duration::new(0, 0)
        }
    }

    fn formatted_date(&self) -> String {
        // TODO: actual implementation
        String::from("48th of Boptember, 1536")
    }
}

#[derive(serde::Serialize)]
struct ArticleHash {
    title: String,
    content: String,
    route: String,
    created_at: String
}

impl ArticleHash {
    fn from_article(a: &FsArticle) -> Self {
        Self {
            title: a.title().unwrap(),
            content: a.content.clone(),
            route: a.route().unwrap(),
            created_at: a.formatted_date()
        }
    }

    fn from_redis(a: &BTreeMap<String, String>) -> Self {
        Self {
            title: a.get("title").unwrap().clone(),
            content: a.get("content").unwrap().clone(),
            route: a.get("route").unwrap().clone(),
            created_at: a.get("created_at").unwrap().clone(),
        }
    }

    fn as_kv_list(&self) -> Box<[(&str, &str)]> {
        Box::new([
            ("title", &self.title),
            ("content", &self.content),
            ("route", &self.route),
            ("created_at", &self.created_at),
        ])
    }
}

#[derive(Debug)]
struct ArticleNotFound;
impl warp::reject::Reject for ArticleNotFound {}

fn tmpl_path(tmpl_name: &str) -> PathBuf {
    let filename = [tmpl_name, "html.hbs"].join(".");
    let path = Path::new(BASE_PATH).join("templates");
    path.join(filename)
}

fn content_path(article_name: &str) -> PathBuf {
    let filename = [article_name, "md"].join(".");
    let path = Path::new(BASE_PATH).join("articles");
    path.join(filename)
}

fn gather_fs_articles() -> Result<Vec<FsArticle>, io::Error> {
    let dir = PathBuf::from(BASE_PATH).join("articles");
    if !dir.is_dir() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Article path is not a directory"));
    }

    let mut articles: Vec<FsArticle> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            if let Ok(article) = FsArticle::from_file(&path) {
                articles.push(article);
            }
        }
    }
    Ok(articles)
}

fn gather_redis_articles() -> Result<Vec<ArticleHash>, redis::RedisError> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;
    let mut articles: Vec<ArticleHash> = Vec::new();
    let keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        if let Ok(result) = con.hgetall(key) {
            articles.push(ArticleHash::from_redis(&result))
        }
    }

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

    if let Ok(articles) = gather_fs_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                let hash = ArticleHash::from_article(&article);
                con.hset_multiple(&key, &hash.as_kv_list())?;
                // con.hset(&key, "title", article.title().unwrap())?;
                // con.hset(&key, "content", &article.content)?;
                // con.hset(&key, "route", article.route().unwrap())?;
                // con.hset(&key, "created_at", article.formatted_date())?;
            }
        }
    }
    Ok(())
}

fn render_index_page(page: usize, hbs: &Handlebars<'_>) -> String {
    // TODO: read articles from Redis, not filesystem
    if let Ok(articles) = gather_redis_articles() {
        let pages: Vec<&[ArticleHash]> = articles.chunks(PAGE_SIZE).collect();
        let constrained_page = cmp::min(pages.len() - 1, page);

        match hbs.render(
            "main",
            &json!({
                "title": BLOG_TITLE,
                "articles": &pages[constrained_page]
            })
        ) {
            Ok(rendered_page) => rendered_page,
            Err(_) => String::from("Error rendering page :("),
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
        let article = ArticleHash::from_redis(&result);
        let parser = cmark::Parser::new(&article.content);
        let mut parsed_article = String::new();
        cmark::html::push_html(&mut parsed_article, parser);

        let reply = warp::reply::html(
            hbs.render(
                "article",
                &json!({"content": &parsed_article})
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

    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index.
        or(article_index_offset).
        or(article).
        or(assets).
        recover(file_not_found);

    println!("Listening on 3090...");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
