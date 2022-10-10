use std::path::PathBuf;
use axum::{
    extract::Extension,
    handler::Handler,
    http::StatusCode,
    response::IntoResponse,
    Router,
    routing::{
        get,
        get_service,
        post,
    },
};
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;

use crate::handlers::{
    index::{
        home_handler,
        index_handler,
        tag_home_handler,
        tag_handler,
    },
    article::{
        article_handler,
        article_text_handler,
    },
    comment::comment_handler,
    static_files::asset_handler,
    not_found_handler,
};

use crate::SharedData;

async fn error_handler(error: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", error),
    )
}

pub fn init(shared_data: SharedData) -> Router {
    let dir = PathBuf::from(shared_data.lock().unwrap().config.content_dir.clone());
    let dir_service = ServeDir::new(dir.join("images"));

    Router::new()
        .route("/",                         get(home_handler))
        .route("/index/:page",              get(index_handler))
        .route("/tag/:tag",                 get(tag_home_handler))
        .route("/tag/:tag/:page",           get(tag_handler))
        .route("/article/:slug",            get(article_handler))
        .route("/article/:slug/text",       get(article_text_handler))
        .route("/comment/:slug",            post(comment_handler))
        .route("/assets/:filename",         get(asset_handler))
        .nest(
            "/content/images/",
            get_service(dir_service).handle_error(error_handler)
        )
        .layer(Extension(shared_data))
        .layer(CookieManagerLayer::new())
        .fallback(not_found_handler.into_service())
}
