pub mod admin;
pub mod article;
pub mod comment;
pub mod index;
pub mod static_files;

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    http::{StatusCode, Uri},
    response::Html,
};
use tower_cookies::Cookies;

pub type HtmlResponse = (StatusCode, Html<String>);

pub fn create_timestamp() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        // i64 is enough milliseconds for 292 million years, so coercing it like
        // this is probably fine.
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

pub fn theme(cookies: Cookies) -> Option<String> {
    cookies.get("theme").map(|c| {
        format!("themes/{}", c.value())
    })
}

// TODO: render HTML from file
pub fn render_error_page<T: std::fmt::Display>(
    status: StatusCode,
    additional_text: Option<T>,
) -> String {
    lazy_static! {
        static ref ERRORS_DIR: PathBuf = {
            let c = crate::config::Config::load().expect("Failed to load config");
            PathBuf::from(c.content_dir).join("errors")
        };
    }
    let message = if additional_text.is_some() {
        format!("HTTP error {:?}: {}", status, additional_text.unwrap())
    } else {
        format!("HTTP error {:?}", status)
    };
    let filename = ERRORS_DIR.join(status.as_u16().to_string() + ".html");
    if let Ok(content) = fs::read_to_string(&filename) {
        content.replace("####", &message)
    } else {
        message
    }
}

pub fn server_error(msg: &str) -> HtmlResponse {
    log::error!("Server error: {}", msg);
    (StatusCode::INTERNAL_SERVER_ERROR, Html(msg.to_string()))
}

pub fn empty_response(code: StatusCode) -> HtmlResponse {
    (code, Html(String::new()))
}

pub fn not_found(uri: Option<Uri>) -> HtmlResponse {
    (
        StatusCode::NOT_FOUND,
        Html(render_error_page(StatusCode::NOT_FOUND, uri)),
    )
}

pub async fn not_found_handler(uri: Option<Uri>) -> HtmlResponse {
    not_found(uri)
}
