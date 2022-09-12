use std::sync::{Arc, Mutex};
use crate::CommonData;

type HtmlReply = warp::reply::WithStatus<warp::reply::Html<String>>;

pub async fn login_route(data: Arc<Mutex<CommonData>>) -> Result<HtmlReply, warp::reject::Rejection> {
    let reply = warp::reply::html("ok".into());
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::OK))
}

pub async fn admin_route() -> Result<HtmlReply, warp::reject::Rejection> {
    let reply = warp::reply::html("not authorised".into());
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::UNAUTHORIZED))
}

