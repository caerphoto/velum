use axum::{
    extract::{ConnectInfo, Json, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use std::net::SocketAddr;

use super::create_timestamp;
use crate::{comments::Comment, typography::typogrified, SharedData};
use axum_macros::debug_handler;

#[derive(Deserialize)]
pub struct JsonComment {
    author: String,
    author_url: String,
    text: String,
}

#[debug_handler]
pub async fn comment_handler(
    Path(slug): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(data): State<SharedData>,
    Json(form_data): Json<JsonComment>,
) -> impl IntoResponse {
    let comment = Comment {
        author: form_data.author,
        author_url: form_data.author_url,
        text: typogrified(&form_data.text),
        base_text: form_data.text,
        timestamp: create_timestamp(),
    };
    let mut data = data.write();
    if let Ok(saved) = data.comments.add(&slug, comment, Some(addr)) {
        log::info!("Saved comment on article '{}'", &slug);
        (
            StatusCode::OK,
            Html(data.hbs.render("_comment", &saved).expect("Render comment")),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("Failed to save comment".to_string()),
        )
    }
}
