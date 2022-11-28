use std::net::SocketAddr;
use axum::{
    http::StatusCode,
    response::{
        Html,
        IntoResponse,
    },
    extract::{
        ConnectInfo,
        Json,
        Path,
        Extension
    },
};
use serde::Deserialize;

use crate::{
    SharedData,
    comments::Comment,
};
use super::create_timestamp;

#[derive(Deserialize)]
pub struct JsonComment {
    author: String,
    author_url: String,
    text: String,
}

pub async fn comment_handler(
    Path(slug): Path<String>,
    Json(form_data): Json<JsonComment>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(data): Extension<SharedData>,
) -> impl IntoResponse {
    let comment = Comment {
        author: form_data.author,
        author_url: form_data.author_url,
        text: form_data.text,
        timestamp: create_timestamp(),
    };
    let mut data = data.write();
    if let Ok(saved) = data.comments.add(&slug, comment, Some(addr)) {
        log::info!("Saved comment on article '{}'", &slug);
        (
            StatusCode::OK,
            Html(data.hbs.render("_comment", &saved).expect("Render comment"))
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("Failed to save comment".to_string())
        )
    }

}
