use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::CommonData;
use warp::Reply;
use bcrypt;

type WarpResult = Result<
    warp::reply::Response,
    warp::reject::Rejection
>;

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;

pub async fn login_page_route(data: Arc<Mutex<CommonData>>) -> WarpResult {
    let data = data.lock().unwrap();
    let body = data.hbs.render("login", &"");
    body
        .map(|b| warp::reply::html(b).into_response())
        .map_err(|e| e)
}

pub async fn do_login_route(data: Arc<Mutex<CommonData>>, _json: HashMap<String, String>) -> WarpResult {
    let mut data = data.lock().unwrap();
    let body = "login page";

    let session_id = "test";
    let cookie = format!("session_id={}; Path=/; HttpOnly; Max-Age={}", session_id, THIRTY_DAYS);

    data.session_id = Some(session_id.to_string());

    Ok(warp::http::Response::builder()
       .header("content-type", "text/html; charset=utf-8")
       .header("set-cookie", cookie)
       .status(200)
       .body(body.into())
       .unwrap()
    )
}

pub async fn admin_route(data: Arc<Mutex<CommonData>>, session_id: Option<String>) -> WarpResult {
    let data = data.lock().unwrap();
    let sid = data.session_id.as_ref();
    if sid.is_none()
        || session_id.is_none()
        || sid.unwrap() != session_id.as_ref().unwrap()
    {
        let reply = warp::redirect::found(warp::http::Uri::from_static("/login"));
        return Ok(reply.into_response());
    }

    let article_names = data.articles.iter()
        .map(|a| a.title.clone())
        .collect::<Vec<String>>();
    let reply = warp::reply::html(article_names.join("\n"));

    Ok(
        warp::reply::with_status(
            reply, warp::http::StatusCode::OK
        ).into_response()
    )
}

