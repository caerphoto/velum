mod article;
mod comments;
mod commondata;
mod errors;
mod handlers;
mod hb;
mod routes;
mod slug;
mod typography;
// mod filters;
mod config;
mod io;

use std::{
    env,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time,
};

use parking_lot::RwLock;

use crate::config::Config;
use commondata::CommonData;

pub type SharedData = Arc<RwLock<CommonData>>;

#[macro_use]
extern crate lazy_static;

pub const MAX_ARTICLE_LENGTH: u64 = 100_000;

fn check_args(config: &mut Config) {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || &args[1] != "register" {
        return;
    }

    config.prompt_for_password()
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let now = time::Instant::now();
    log::info!("Building articles and comments, and reading templates... ");
    let codata = CommonData::new();
    let mut config = codata.config.clone();
    let shared_codata = Arc::new(RwLock::new(codata));
    log::info!("...done in {}ms.", now.elapsed().as_millis());

    check_args(&mut config);

    let app = routes::init(shared_codata.clone());

    let listen_ip = config
        .listen_ip
        .parse::<IpAddr>()
        .unwrap_or_else(|_| panic!("Failed to parse listen IP from {}", &config.listen_ip));
    let listen_port = config.listen_port;
    log::info!("Listening on http://{}:{}/ ...", &listen_ip, &listen_port);
    let listen = SocketAddr::new(listen_ip, listen_port);

    axum::Server::bind(&listen)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
