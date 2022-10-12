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
    response::Html,
};
use tower_cookies::Cookies;

pub type HtmlResponse = (StatusCode, Html<String>);

pub fn theme(cookies: Cookies) -> Option<String> {
    cookies
        .get("theme")
        .map(|c| c.value().to_string())
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
pub fn render_error_page<T: std::fmt::Display>(status: StatusCode, additional_text: Option<T>) -> String {
    if additional_text.is_some() {
        format!("Error {:?}: {}", status, additional_text.unwrap())
    } else {
        format!("Error {:?}", status)

    }
}

pub fn server_error(msg: &str) -> HtmlResponse {
    log::error!("Server error: {}", msg);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(msg.to_string()),
    )
}

pub fn empty_response(code: StatusCode) -> HtmlResponse {
    (
        code,
        Html(String::new())
    )
}

pub fn not_found(uri: Option<Uri>) -> HtmlResponse {
    (
        StatusCode::NOT_FOUND,
        Html(render_error_page(StatusCode::NOT_FOUND, uri))
    )
}

pub async fn not_found_handler(uri: Option<Uri>) -> HtmlResponse {
    not_found(uri)
}
