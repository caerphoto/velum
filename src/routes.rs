use axum::{
    extract::Extension,
    Router,
    routing::get,
};
use tower_cookies::CookieManagerLayer;

use crate::handlers::{
    asset::asset_handler,
    index::{
        home_handler,
        index_handler,
        tag_home_handler,
        tag_handler,
    },
};

use crate::SharedData;

pub fn init(shared_data: SharedData) -> Router {
    Router::new()
        .route("/",get(home_handler))
        .route("/index/:page", get(index_handler))
        .route("/tag/:tag", get(tag_home_handler))
        .route("/tag/:tag/:page", get(tag_handler))
        .route("/assets/:page", get(asset_handler))
        .layer(Extension(shared_data))
        .layer(CookieManagerLayer::new())
}
