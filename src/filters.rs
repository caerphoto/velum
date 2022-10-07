use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use core::convert::TryFrom;

use warp::{Filter, Reply, http::Uri};

use crate::MAX_ARTICLE_LENGTH;
use crate::routes::{
    index_page_route,
    tag_search_route,

    article_route,
    article_text_route,
    create_article_route,
    update_article_route,
    delete_article_route,

    comment_route,

    admin_route,
    login_page_route,
    do_login_route,
    do_logout_route,
    rebuild_index_route,

    timestamped_asset_route,
};

pub type SharedData = Arc<Mutex<crate::CommonData>>;

pub fn index_filter(codata: SharedData) -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    let codata_filter = warp::any().map(move || codata.clone());
    let home = warp::path::end().map(|| 0usize)
        .and(warp::cookie::optional::<String>("theme"))
        .and(codata_filter.clone())
        .and_then(index_page_route);
    let index_at_page = warp::path!("index" / usize)
        .and(warp::cookie::optional::<String>("theme"))
        .and(codata_filter.clone())
        .and_then(index_page_route);

    let with_tag = warp::path!("tag" / String)
        .map(|tag: String| (tag, 1) )
        .untuple_one()
        .and(warp::cookie::optional::<String>("theme"))
        .and(codata_filter.clone())
        .and_then(tag_search_route);
    let with_tag_at_page = warp::path!("tag" / String / usize)
        .and(warp::cookie::optional::<String>("theme"))
        .and(codata_filter)
        .and_then(tag_search_route);

    home.or(index_at_page).or(with_tag).or(with_tag_at_page)
}

pub fn article_filter(codata: SharedData) -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    let codata_filter = warp::any().map(move || codata.clone());
    let show = warp::path!("articles" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("Referer"))
        .and(warp::cookie::optional::<String>("theme"))
        .and(codata_filter.clone())
        .and_then(article_route);
    let create = warp::path!("articles")
        .and(warp::post())
        .and(warp::filters::body::bytes())
        .and(warp::body::content_length_limit(MAX_ARTICLE_LENGTH))
        .and(warp::cookie::optional::<String>("velum_session_id"))
        .and(codata_filter.clone())
        .and_then(create_article_route);
    let update = warp::path!("articles" / String)
        .and(warp::put())
        .and(warp::filters::body::bytes())
        .and(warp::body::content_length_limit(MAX_ARTICLE_LENGTH))
        .and(warp::cookie::optional::<String>("velum_session_id"))
        .and(codata_filter.clone())
        .and_then(update_article_route);
    let delete = warp::path!("articles" / String)
        .and(warp::delete())
        .and(warp::cookie::optional::<String>("velum_session_id"))
        .and(codata_filter.clone())
        .and_then(delete_article_route);

    let text = warp::path!("articles" / String / "text")
        .and(codata_filter)
        .and_then(article_text_route);

    show.or(create).or(update).or(delete).or(text)
}

pub fn legacy_filter() -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    // Only necessary for handling imported articles from Ghost blog.
    warp::path!(String)
        .and(warp::get())
        .map(|slug| {
            let path = Uri::try_from(format!("/articles/{}", slug));
            warp::redirect::redirect(
                path.unwrap_or_else(|_| Uri::from_static("/"))
            ).into_response()
        })
}

pub fn comment_filter(codata: SharedData) -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    let codata_filter = warp::any().map(move || codata.clone());
    warp::path!("comment" / String)
        .and(warp::post())
        .and(warp::filters::body::form())
        .and(warp::body::content_length_limit(4000))
        .and(warp::filters::addr::remote())
        .and(codata_filter)
        .and_then(comment_route)
}

pub fn admin_filter(codata: SharedData) -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    let codata_filter = warp::any().map(move || codata.clone());
    let page = warp::path!("admin")
        .and(warp::cookie::optional::<String>("velum_session_id"))
        .and(codata_filter.clone())
        .and_then(admin_route);
    let login_page = warp::path!("login")
        .and(warp::get())
        .and(codata_filter.clone())
        .and_then(login_page_route);
    let do_login = warp::path!("login")
        .and(warp::post())
        .and(warp::body::form())
        .and(warp::body::content_length_limit(2048))
        .and(codata_filter.clone())
        .and_then(do_login_route);
    let do_logout = warp::path!("logout")
        .and(codata_filter.clone())
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and_then(do_logout_route);
    let rebuild_index = warp::path!("rebuild")
        .and(warp::cookie::optional::<String>("velum_session_id"))
        .and(warp::post())
        .and(warp::body::content_length_limit(0))
        .and(codata_filter)
        .and_then(rebuild_index_route);
    page.or(login_page).or(do_login).or(do_logout).or(rebuild_index)
}

pub fn statics_filter(codata: SharedData, path: &str) -> impl Filter<
    Extract = impl warp::Reply,
    Error=warp::Rejection
> + Clone + 'static {
    let path = PathBuf::from(path);
    let codata_filter = warp::any().map(move || codata.clone());
    let images = warp::path("content")
        .and(warp::path("images"))
        .and(warp::fs::dir(path.join("images")));
    let timestamped_asset = warp::path!("assets" / String)
        .and(codata_filter)
        .and_then(timestamped_asset_route);

    let assets = warp::path("assets").and(warp::fs::dir(path.join("assets")));

    let robots_txt = warp::path!("robots.txt").map(|| "");

    let favicon16 = warp::path!("favicon16.png")
        .and(warp::fs::file(path.join("favicon16.png")));
    let favicon32 = warp::path!("favicon32.png")
        .and(warp::fs::file(path.join("favicon32.png")));
    let favicon_apple = warp::path!("favicon_apple.png")
        .and(warp::fs::file(path.join("favicon_apple.png")));
    images
        .or(timestamped_asset)
        .or(assets)
        .or(robots_txt)
        .or(favicon16)
        .or(favicon32)
        .or(favicon_apple)
}
