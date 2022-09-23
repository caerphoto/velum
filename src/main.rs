mod commondata;
mod article;
mod hb;
mod comments;
mod errors;
mod routes;
mod config;
mod io;

use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time;
use std::env;
use std::net::IpAddr;
use std::collections::HashMap;
use core::convert::TryFrom;
use warp::{Filter, Reply, http::Uri};
use crate::config::Config;
use commondata::CommonData;
use routes::{
    index_page_route,
    tag_search_route,
    article_route,
    article_text_route,
    create_article_route,
    update_article_route,
    delete_article_route,
    comment_route,
    file_not_found_route,
    admin_route,
    login_page_route,
    do_login_route,
    do_logout_route,
    rebuild_index_route,
};

#[macro_use] extern crate lazy_static;

const HASH_COST: u32 = 8;
const MAX_ARTICLE_LENGTH: u64 = 100_000;

fn check_args(config: &mut Config) {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || &args[1] != "register" { return; }

    if let Ok(pw) = rpassword::prompt_password("Enter an admin password: ") {
        if pw.is_empty() {
            println!("Password cannot be blank.");
            std::process::exit(1);
        }
        if let Ok(pw_conf) = rpassword::prompt_password("Confirm admin password: ") {
            if pw != pw_conf {
                println!("Passwords do not match.");
                std::process::exit(1);
            }
            config.secrets.admin_password_hash = Some(
                bcrypt::hash(pw, HASH_COST).expect("Failed to hash password")
            );

            if let Err(e) = config.save() {
                panic!("Config save failed: {:?}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let now = time::Instant::now();
    log::info!("Building article and comment data from files... ");
    let codata = CommonData::new();
    let mut config = codata.config.clone();
    let shared_codata = Arc::new(Mutex::new(codata));
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    check_args(&mut config);

    let codata_filter = warp::any()
        .map(move || shared_codata.clone());

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
        .and(warp::get())
        .and(warp::header::optional::<String>("Referer"))
        .and(codata_filter.clone())
        .and_then(article_route);
    let create_article = warp::path!("articles")
        .and(warp::post())
        .and(warp::filters::body::bytes())
        .and(warp::body::content_length_limit(MAX_ARTICLE_LENGTH))
        .and(warp::cookie::optional::<String>("session_id"))
        .and(codata_filter.clone())
        .and_then(create_article_route);
    let update_article = warp::path!("articles" / String)
        .and(warp::put())
        .and(warp::filters::body::bytes())
        .and(warp::body::content_length_limit(MAX_ARTICLE_LENGTH))
        .and(warp::cookie::optional::<String>("session_id"))
        .and(codata_filter.clone())
        .and_then(update_article_route);
    let delete_article = warp::path!("articles" / String)
        .and(warp::delete())
        .and(warp::cookie::optional::<String>("session_id"))
        .and(codata_filter.clone())
        .and_then(delete_article_route);

    let article_text = warp::path!("articles" / String / "text")
        .and(codata_filter.clone())
        .and_then(article_text_route);

    // Only necessary for handling imported articles from Ghost blog.
    let legacy_article = warp::path!(String)
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::get())
        .map(|slug, _| {
            let path = Uri::try_from(format!("/articles/{}", slug));
            warp::redirect::redirect(
                path.unwrap_or_else(|_| Uri::from_static("/"))
            ).into_response()
        });

    let comment = warp::path!("comment" / String)
        .and(warp::post())
        .and(warp::filters::body::form())
        .and(warp::body::content_length_limit(4000))
        .and(warp::filters::addr::remote())
        .and(codata_filter.clone())
        .and_then(comment_route);

    let admin = warp::path!("admin")
        .and(warp::cookie::optional::<String>("session_id"))
        .and(codata_filter.clone())
        .and_then(admin_route);
    let login_page = warp::path!("login")
        .and(warp::get())
        .and(codata_filter.clone())
        .and_then(login_page_route);
    let do_login = warp::path!("login")
        .and(warp::post())
        .and(warp::body::form())
        .and(warp::body::content_length_limit(2048))
        .and(codata_filter.clone())
        .and_then(do_login_route);
    let do_logout = warp::path!("logout")
        .and(codata_filter.clone())
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and_then(do_logout_route);
    let rebuild_index = warp::path!("rebuild")
        .and(warp::cookie::optional::<String>("session_id"))
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and(codata_filter.clone())
        .and_then(rebuild_index_route);

    let path = PathBuf::from(&config.content_dir);
    let images = warp::path("content")
        .and(warp::path("images"))
        .and(warp::fs::dir(path.join("images")));
    let assets = warp::path("assets").and(warp::fs::dir(path.join("assets")));

    let robots_txt = warp::path!("robots.txt").map(|| "");

    let favicon16 = warp::path!("favicon16.png")
        .and(warp::fs::file(path.join("favicon16.png")));
    let favicon32 = warp::path!("favicon32.png")
        .and(warp::fs::file(path.join("favicon32.png")));
    let favicon_apple = warp::path!("favicon_apple.png")
        .and(warp::fs::file(path.join("favicon_apple.png")));

    let error_logger = warp::filters::log::custom(|info| {
        let s = info.status();
        let msg = format!(
            "{} `{}` {}",
            info.method(),
            info.path(),
            info.status()
        );
        if s.is_client_error() {
            log::info!("{}", msg);
        } else if s.is_server_error() {
            log::error!("{}", msg);
        } else if let Some(r) = info.referer() {
            if !r.contains("blog.andyf.me") {
                log::info!("Referer: {}", r);
            }
        }
    });

    let routes = article_index
        .or(article_index_at_page)
        .or(article)
        .or(create_article)
        .or(update_article)
        .or(delete_article)
        .or(article_text)
        .or(articles_with_tag)
        .or(articles_with_tag_at_page)
        .or(comment)
        .or(admin)
        .or(login_page)
        .or(do_login)
        .or(do_logout)
        .or(rebuild_index)
        .or(images)
        .or(assets)
        .or(robots_txt)
        .or(favicon16).or(favicon32).or(favicon_apple)
        .or(legacy_article)
        .recover(file_not_found_route)
        .with(error_logger);

    let listen_ip = config.listen_ip.parse::<IpAddr>()
        .unwrap_or_else(|_| panic!("Failed to parse listen IP from {}", &config.listen_ip));
    let listen_port = config.listen_port;

    warp::serve(routes)
        .run((listen_ip, listen_port))
        .await;

}
