use std::process;
use std::env;
use std::path::PathBuf;
use std::fs;
use std::error::Error;
use serde_json::json;
use handlebars::Handlebars;
use pulldown_cmark::{Parser, html};

fn path_if_valid(base: &str, dir: &str) -> PathBuf {
    let path: PathBuf = [&base, dir].iter().collect();
    match fs::metadata(&path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                metadata
            } else {
                println!("Error: {} is not a directory", path.display());
                process::exit(1);
            }
        },
        Err(_) => {
            println!("Error: {} doesn't exist or is not accessible", path.display());
            process::exit(1);
        }
    };
    path
}

fn render_main(base_path: &str) -> Result<(), Box<dyn Error>> {
    let tmpl_path: PathBuf = [&base_path, "templates", "main.html.hbs"].iter().collect();
    let article_path: PathBuf = [&base_path, "articles", "test.md"].iter().collect();

    let mut hb = Handlebars::new();
    // let article = fs::read_to_string(&article_path)?.parse()?;
    let article = fs::read_to_string(&article_path)?;
    let parser = Parser::new(&article);
    let mut article_html = String::new();
    html::push_html(&mut article_html, parser);

    hb.register_template_file("main", &tmpl_path)?;
    println!("{}", hb.render("main", &json!({"content": &article_html}))?);
    Ok(())
}

fn main() {
    let base_key = "VELUM_BASE";
    let base_path = match env::var(base_key) {
        Ok(val) => val,
        Err(_) => {
            println!("Error: {} env var not set", base_key);
            process::exit(1);
        }
    };

    println!("Base path: {}", &base_path);
    match render_main(&base_path) {
        Ok(_) => println!("Success!"),
        Err(err) => println!("Failed :( {:?}", err)
    }
}
