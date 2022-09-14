mod commondata;
mod article;
mod hb;
mod comments;
mod errors;
mod routes;
mod config;

use std::sync::{Arc, Mutex};
use std::time;
use std::env;
use std::net::IpAddr;
use std::collections::HashMap;
use core::convert::TryFrom;
use warp::{Filter, Reply, http::Uri};
use commondata::CommonData;
use routes::{
    index_page_route,
    tag_search_route,
    article_route,
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

fn check_args(data: Arc<Mutex<CommonData>>) {
    let mut data = data.lock().unwrap();
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
            data.config.admin_password_hash = Some(
                bcrypt::hash(pw, HASH_COST).expect("Failed to hash password")
            );

            match data.config.save() {
                Ok(_) => {},
                Err(e) => { panic!("Config save failed: {:?}", e) }
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
    let shared_codata = Arc::new(Mutex::new(codata));
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    check_args(shared_codata.clone());

    let filter_data = shared_codata.clone();
    let codata_filter = warp::any()
        .map(move || filter_data.clone());

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

    // Only necessary for handling imported articles from Ghost blog.
    let legacy_article = warp::path!(String)
        .and(warp::query::<HashMap<String, String>>())
        .map(|slug, _| {
            let path = Uri::try_from(format!("/articles/{}", slug));
            warp::redirect::redirect(
                path.unwrap_or_else(|_| Uri::from_static("/"))
            ).into_response()
        });

    let comment = warp::path!("comment" / String)
        .and(warp::body::content_length_limit(4000))
        .and(warp::filters::body::form())
        .and(warp::filters::addr::remote())
        .and(codata_filter.clone())
        .and(warp::post())
        .then(comment_route);

    let admin = warp::path!("admin")
        .and(codata_filter.clone())
        .and(warp::cookie::optional::<String>("session_id"))
        .and_then(admin_route);
    let login_page = warp::path!("login")
        .and(warp::get())
        .and(codata_filter.clone())
        .and_then(login_page_route);
    let do_login = warp::path!("login")
        .and(codata_filter.clone())
        .and(warp::post())
        .and(warp::body::content_length_limit(2048))
        .and(warp::body::form())
        .and_then(do_login_route);
    let do_logout = warp::path!("logout")
        .and(codata_filter.clone())
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and_then(do_logout_route);
    let rebuild_index = warp::path!("rebuild")
        .and(codata_filter.clone())
        .and(warp::cookie::optional::<String>("session_id"))
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and_then(rebuild_index_route);

    // TODO: change hard-coded content dir() to use the one from config
    // can't use path! macro because it ends the path
    let images = warp::path("content")
        .and(warp::path("images"))
        .and(warp::fs::dir("content/images"));

    let assets = warp::path("assets").and(warp::fs::dir("content/assets"));

    let robots_txt = warp::path!("robots.txt").map(|| "");

    let favicon16 = warp::path!("favicon16.png")
        .and(warp::fs::file("content/favicon16.png"));
    let favicon32 = warp::path!("favicon32.png")
        .and(warp::fs::file("content/favicon32.png"));
    let favicon_apple = warp::path!("favicon_apple.png")
        .and(warp::fs::file("content/favicon_apple.png"));

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

    let listen_ip: IpAddr;
    let listen_port: u16;
    {
        // ensure this mutex guard only lives inside this block, and doesn't
        // get held across the below await point
        let cd = shared_codata.lock().unwrap();
        let config_listen_ip = cd.config.listen_ip.clone();
        listen_ip = config_listen_ip.parse::<IpAddr>()
            .unwrap_or_else(|_| panic!("Failed to parse listen IP from {}", config_listen_ip));
        listen_port = cd.config.listen_port;
    }

    warp::serve(routes)
        .run((listen_ip, listen_port))
        .await;

}
