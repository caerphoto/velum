type HtmlReply = warp::reply::WithStatus<warp::reply::Html<String>>;

pub async fn admin_route() -> Result<HtmlReply, warp::reject::Rejection> {
    let reply = warp::reply::html("not authorised".into());
    Ok(warp::reply::with_status(reply, warp::http::StatusCode::UNAUTHORIZED))
}

