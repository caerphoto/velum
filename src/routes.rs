use std::{
    time::Duration,
    path::PathBuf,
};
use axum::{
    extract::{Path, DefaultBodyLimit},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Router,
    routing::{
        delete,
        get,
        get_service,
        post,
        put,
    },
};
use tower::ServiceBuilder;
use tower_cookies::CookieManagerLayer;
use tower_http::{
    services::ServeDir,
    compression::CompressionLayer,
    timeout::TimeoutLayer,
};

use crate::handlers::{
    index::{
        home_handler,
        index_handler,
        rss_handler,
        tag_home_handler,
        tag_handler,
    },
    article::{
        article_handler,
        article_text_handler,
    },
    comment::comment_handler,
    admin::{
        login_page_handler,
        do_login_handler,
        do_logout_handler,
        admin_page_handler,
        rebuild_index_handler,
        create_article_handler,
        update_article_handler,
        delete_article_handler,
        image_list_handler,
        upload_image_handler,
        delete_image_handler,
        check_thumb_progress,
    },
    static_files::asset_handler,
    not_found_handler,
};

use crate::SharedData;

async fn error_handler(error: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {error}"),
    )
}

pub fn init(shared_data: SharedData) -> Router {
    let dir = PathBuf::from(shared_data.read().config.content_dir.clone());
    let image_dir_service = get_service(ServeDir::new(dir.join("images")))
        .handle_error(error_handler);

    let middleware = ServiceBuilder::new()
        .layer(CompressionLayer::new())
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        ;

    let img_upload_route = Router::new()
        .route("/images", post(upload_image_handler))
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024));

    Router::new()
        .route("/",                       get(home_handler))
        .route("/articles/:page_or_slug", get(index_handler))
        .route("/article/:slug",          get(article_handler))
        .route("/article/:slug/text",     get(article_text_handler))

        .route("/tag/:tag",               get(tag_home_handler))
        .route("/tag/:tag/:page",         get(tag_handler))
        .route("/tag/:tag/",              get(|Path(tag): Path<String>| async move {
            // Legacy tag support, since my blog still gets hits on this path
            Redirect::permanent(&(String::from("/tag/") + &tag))
        }))
        .route("/rss",                    get(rss_handler))

        .route("/comment/:slug",          post(comment_handler))

        .route("/login",                  get(login_page_handler))
        .route("/login",                  post(do_login_handler))
        .route("/logout",                 post(do_logout_handler))
        .route("/admin",                  get(admin_page_handler))
        .route("/rebuild_index",          post(rebuild_index_handler))
        .route("/articles",               post(create_article_handler))
        .route("/article/:slug",          put(update_article_handler))
        .route("/article/:slug",          delete(delete_article_handler))
        .route("/all_images",             get(image_list_handler))
        .merge(img_upload_route)
        .route("/check_thumb_progress",   get(check_thumb_progress))
        .route("/images/*path",           delete(delete_image_handler))

        .route("/assets/*path",           get(asset_handler))
        .nest_service("/content/images/", image_dir_service)

        .with_state(shared_data)
        .layer(CookieManagerLayer::new())
        .layer(middleware)
        .fallback(not_found_handler)
}
