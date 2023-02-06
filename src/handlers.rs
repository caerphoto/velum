pub mod admin;
pub mod article;
pub mod comment;
pub mod index;
pub mod static_files;

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH, Instant},
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

pub fn theme(cookies: Cookies) -> String {
    cookies
        .get("theme")
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "light".to_string())
}

pub fn log_elapsed(thing: &str, thing_name: Option<&str>, page: Option<usize>, from: Instant) {
    let elapsed = from.elapsed().as_micros();
    let (elapsed, unit) = if elapsed < 1000 {
        (elapsed, 'µ')
    } else {
        (elapsed / 1000, 'm')
    };

    let thing_name = match thing_name {
        Some(t) => format!(" `{t}`"),
        None => "".to_string(),
    };

    if let Some(page) = page {
        log::info!(
            "Rendered {}{} ({}) in {}{}s",
            thing, thing_name, page,
            elapsed, unit
        );
    } else {
        log::info!(
            "Rendered {}{} in {}{}s",
            thing, thing_name,
            elapsed, unit
        );
    }

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
        format!("HTTP error {status:?}")
    };
    let filename = ERRORS_DIR.join(status.as_u16().to_string() + ".html");
    if let Ok(content) = fs::read_to_string(filename) {
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

pub fn server_error_page(msg: &str) -> HtmlResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(render_error_page(StatusCode::INTERNAL_SERVER_ERROR, Some(msg))),
    )
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
