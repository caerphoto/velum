use std::env;
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

fn tmpl_path(base_path: &str, tmpl_name: &str) -> PathBuf {
    let filename = [tmpl_name, ".html.hbs"].join("");
    let path = Path::new(base_path).join("templates");
    path.join(filename)
}

fn content_path(base_path: &str, article_name: &str) -> PathBuf {
    let filename = [article_name, ".md"].join("");
    let path = Path::new(base_path).join("articles");
    path.join(filename)
}

fn read_article_title(path: &PathBuf) -> String {
    // Assumes first line of text in the article file is formatted exactly as '# Article Title'
    let file = fs::File::open(path).expect("Unable to open article file");
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).expect("Unable to read first line of article file");
    line.replacen("# ", "", 1)
}

fn gather_article_headers(base_path: &str) -> Result<Vec<ArticleHeader>, io::Error> {
    let dir = PathBuf::from(base_path).join("articles");
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

fn render_index(base_path: &str, page: usize) -> String {
    let tmpl_path = tmpl_path(base_path, "index");
    let mut hb = Handlebars::new();
    hb.register_template_file("main", &tmpl_path).expect("Failed to register index template file");

    let mut article_headers: Vec<ArticleHeader> = Vec::new();
    match gather_article_headers(base_path) {
        Err(_) => (),
        Ok(gathered_headers) => article_headers = gathered_headers
    }

    let paginated_headers: Vec<&[ArticleHeader]> = article_headers.chunks(PAGE_SIZE).collect();

    hb.render("main", &json!({"articles": &paginated_headers[page]})).expect("Failed to render index")
}

fn render_article(base_path: &str, article_name: &str) -> String {
    let tmpl_path = tmpl_path(base_path, "article");
    let mut hb = Handlebars::new();
    hb.register_template_file("article", &tmpl_path).expect("Failed to register article template file");

    let article_path = content_path(base_path, article_name);
    let article_content = fs::read_to_string(&article_path).expect("Unable to read article content");
    let parser = Parser::new(&article_content);
    let mut parsed_article = String::new();
    html::push_html(&mut parsed_article, parser);

    hb.render("article", &json!({"content": &parsed_article})).expect("Failed to render article")
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    //let base_path = Arc::new(env::var( "VELUM_BASE").expect("No VELUM_BASE env var set"));
    //let base_path_articles = base_path.clone();

    let article_index = warp::path::end().map(||
        warp::reply::html(render_index(&BASE_PATH, 0))
    );
    let article = warp::path!("articles" / String).map(|name: String|
        warp::reply::html(render_article(&BASE_PATH, name.as_str()))
    );

    let routes = article_index.or(article);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3090))
        .await;
}
