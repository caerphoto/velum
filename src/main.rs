use std::env;
use std::convert::Infallible;
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};
use std::io::{self, prelude::*, Error, ErrorKind, BufReader};
use std::fs;
use serde::Serialize;
use serde_json::json;
use warp::Filter;
use handlebars::Handlebars;
use pulldown_cmark::{Parser, html};

const PAGE_SIZE: usize = 10;

lazy_static! {
    static ref BASE_PATH: String = env::var( "VELUM_BASE").expect("No VELUM_BASE env var set");
}

#[derive(Serialize)]
struct ArticleHeader {
    path: PathBuf,
    route: String,
    title: String,
    created_at: std::time::SystemTime
}

#[derive(Debug)]
struct ArticleNotFound;
impl warp::reject::Reject for ArticleNotFound {}

fn tmpl_path(tmpl_name: &str) -> PathBuf {
    let filename = [tmpl_name, ".html.hbs"].join("");
    let path = Path::new(BASE_PATH.as_str()).join("templates");
    path.join(filename)
}

fn content_path(article_name: &str) -> PathBuf {
    let filename = [article_name, ".md"].join("");
    let path = Path::new(BASE_PATH.as_str()).join("articles");
    path.join(filename)
}

fn read_article_title(path: &PathBuf) -> String {
    // Assumes first line of text in the article file is formatted exactly as '# Article Title'
    let file = fs::File::open(path).expect("Unable to open article file");
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line)
        .expect("Unable to read first line of article file");
    line.replacen("# ", "", 1)
}

fn gather_article_headers() -> Result<Vec<ArticleHeader>, io::Error> {
    let dir = PathBuf::from(BASE_PATH.as_str()).join("articles");
    if !dir.is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput, "Provided base_path is not a directory"));
    }

    let mut article_headers: Vec<ArticleHeader> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            let file_stem = path
                .file_stem().expect("Unable to get file_stem from path")
                .to_str().expect("Unable to convert OsStr to str");
            let article = ArticleHeader {
                path: path.clone(),
                route: String::from(["articles", file_stem].join("/")),
                title: read_article_title(&path),
                created_at: entry.metadata()?.created()?
            };
            article_headers.push(article);
        }
    }
    Ok(article_headers)
}

fn render_index(page: usize) -> String {
    let tmpl_path = tmpl_path("index");
    let mut hb = Handlebars::new();
    hb.register_template_file("main", &tmpl_path)
        .expect("Failed to register index template file");

    let mut article_headers: Vec<ArticleHeader> = Vec::new();
    match gather_article_headers() {
        Err(_) => (),
        Ok(gathered_headers) => article_headers = gathered_headers
    }

    let paginated_headers: Vec<&[ArticleHeader]> = article_headers.chunks(PAGE_SIZE).collect();

    hb.render("main", &json!({"title": "Velum Blog Test", "articles": &paginated_headers[page]}))
        .expect("Failed to render index")
}

async fn render_article(article_name: String) -> Result<impl warp::Reply, warp::Rejection> {
    let tmpl_path = tmpl_path("article");
    let mut hb = Handlebars::new();
    hb.register_template_file("article", &tmpl_path).expect("Failed to register article template file");

    let article_path = content_path(&article_name);
    if let Ok(article_content) = fs::read_to_string(&article_path) {

        let parser = Parser::new(&article_content);
        let mut parsed_article = String::new();
        html::push_html(&mut parsed_article, parser);

        Ok(warp::reply::html(
            hb.render("article", &json!({"content": &parsed_article})).expect("Failed to render article")
        ))
    } else {
        Err(warp::reject::not_found())
    }
}

async fn file_not_found(_: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let error_page = fs::read_to_string("content/errors/404.html").unwrap();
    let reply = warp::reply::html(error_page);
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::NOT_FOUND))
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let article_index = warp::path::end().map(||
        warp::reply::html(render_index(0))
    );
    let article = warp::path!("articles" / String).and_then(render_article);

    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let routes = article_index.or(article).or(assets).recover(file_not_found);

    println!("Listening on 3090...");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
