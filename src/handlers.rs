pub mod index;
pub mod article;
pub mod comment;
pub mod admin;
pub mod static_files;

use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use axum::{
        http::{StatusCode, Uri},
    response::{Html, Redirect},
};
use tower_cookies::Cookies;

pub fn theme(cookies: Cookies) -> Option<String> {
    cookies
        .get("theme")
        .and_then(|c| Some(c.value().to_string()))
}

pub fn create_timestamp() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        // i64 is enough milliseconds for 292 million years, so coercing it like
        // this is probably fine.
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64)
    }
}

// TODO: render HTML from file
pub fn render_not_found(uri: Option<Uri>) -> String {
    format!("No route found for {:?}", uri)
}

pub fn render_server_error(msg: &str) -> String {
    log::error!("{}", msg);
    format!("Internal server error :(")
}

pub fn server_error(msg: &str) -> Result<(StatusCode, Html<String>), Redirect> {
    Ok((
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(msg.to_string()),
    ))
}

pub fn empty_response(code: StatusCode) -> Result<(StatusCode, Html<String>), Redirect> {
    Ok((
        code,
        Html(String::new())
    ))
}

pub async fn not_found_handler(uri: Option<Uri>) -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(render_not_found(uri))
    )
}
