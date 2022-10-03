mod commondata;
mod article;
mod hb;
mod comments;
mod errors;
mod routes;
mod filters;
mod config;
mod io;

use std::sync::{Arc, Mutex};
use std::time;
use std::env;
use std::net::IpAddr;

use warp::Filter;

use crate::config::Config;
use commondata::CommonData;
use filters::{
    index_filter,
    article_filter,
    comment_filter,
    admin_filter,
    statics_filter,
};
use routes::file_not_found_route;

#[macro_use] extern crate lazy_static;

pub const MAX_ARTICLE_LENGTH: u64 = 100_000;

fn check_args(config: &mut Config) {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || &args[1] != "register" { return; }

    config.prompt_for_password()
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let now = time::Instant::now();
    log::info!("Building articles and comments, and reading templates... ");
    let codata = CommonData::new();
    let mut config = codata.config.clone();
    let shared_codata = Arc::new(Mutex::new(codata));
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    check_args(&mut config);

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

    let routes = index_filter(shared_codata.clone())
        .or(article_filter(shared_codata.clone()))
        .or(comment_filter(shared_codata.clone()))
        .or(admin_filter(shared_codata.clone()))
        .or(statics_filter(shared_codata.clone(), &config.content_dir))
        .recover(file_not_found_route)
        .with(error_logger);

    let listen_ip = config.listen_ip.parse::<IpAddr>()
        .unwrap_or_else(|_| panic!("Failed to parse listen IP from {}", &config.listen_ip));
    let listen_port = config.listen_port;

    warp::serve(routes)
        .run((listen_ip, listen_port))
        .await;

}
