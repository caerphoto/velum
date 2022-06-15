use std::sync::Arc;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::io::{self, ErrorKind};
use std::{fs, time, cmp};
use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde_json::json;
use warp::Filter;
use handlebars::Handlebars;
use pulldown_cmark::{Parser, html};
use redis;
use redis::Commands;
use regex::Regex;

#[macro_use]
extern crate lazy_static;

const PAGE_SIZE: usize = 10;
const BASE_PATH: &str = "./content";
const BASE_KEY: &str = "velum:articles:";
const BLOG_TITLE: &str = "Velum Test Blog";

#[derive(Debug)]
struct Article {
    content: String,
    created_at: time::SystemTime
}

impl Article {
    fn new(content: &str) -> Self {
        Article {
            content: String::from(content),
            created_at: time::SystemTime::now(),
        }
    }

    fn from_file(path: &PathBuf) -> Result<Self, io::Error> {
        let content = fs::read_to_string(path)?;
        Ok(Article {
            content,
            created_at: time::SystemTime::now(),
        })
    }

    fn title(&self) -> Option<String> {
        lazy_static! { static ref H1: Regex = Regex::new(r"^#\w*").unwrap(); }
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
        lazy_static! { static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z\- ]").unwrap(); }
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

impl Serialize for Article {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut state = serializer.serialize_struct("Article", 3)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("title", &self.title().unwrap())?;
        state.serialize_field("created_at", &self.formatted_date())?;
        state.end()
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

fn gather_articles() -> Result<Vec<Article>, io::Error> {
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

fn destroy_article_keys(con: &mut redis::Connection) -> redis::RedisResult<()> {
    let keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }

    Ok(())
}

fn rebuild_article_keys() -> redis::RedisResult<()> {
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_connection()?;

    destroy_article_keys(&mut con)?;

    if let Ok(articles) = gather_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                con.hset(&key, "title", article.title().unwrap())?;
                con.hset(&key, "content", article.content)?;
            }
        }
    }
    Ok(())
}

fn render_index_page(page: usize, hbs: &Handlebars<'_>) -> String {
    // TODO: read articles from Redis, not filesystem
    if let Ok(articles) = gather_articles() {
        let pages: Vec<&[Article]> = articles.chunks(PAGE_SIZE).collect();
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

fn render_article(article_name: String, hbs: &Handlebars<'_>) -> impl warp::Reply {
    let article_path = content_path(&article_name);
    if let Ok(article_content) = fs::read_to_string(&article_path) {

        let parser = Parser::new(&article_content);
        let mut parsed_article = String::new();
        html::push_html(&mut parsed_article, parser);


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

    if let Err(e) = rebuild_article_keys() {
        panic!("Failed to rebuild article headers: {:?}", e);
    }

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
