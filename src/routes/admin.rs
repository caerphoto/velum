use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::CommonData;
use warp::Reply;

type WarpResult = Result<
    warp::reply::Response,
    warp::reject::Rejection
>;

pub async fn login_page_route(_data: Arc<Mutex<CommonData>>) -> WarpResult {
    Ok(warp::reply::html("this is the login page").into_response())
}

pub async fn do_login_route(data: Arc<Mutex<CommonData>>, _json: HashMap<String, String>) -> WarpResult {
    let mut data = data.lock().unwrap();
    let body = "login page";

    let session_id = "test";
    let cookie = format!("session_id={}; Path=/; HttpOnly; Max-Age=1209600", session_id);

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

