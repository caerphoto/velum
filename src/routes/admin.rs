use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::CommonData;
use warp::Reply;
use bcrypt;
use serde_json::json;

type WarpResult = Result<
    warp::reply::Response,
    warp::reject::Rejection
>;

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;

fn render_login_page(hbs: &handlebars::Handlebars, error_msg: Option<&str>) -> WarpResult {
    let body = hbs.render(
        "login",
        &json!({
            "body_class": "login",
            "error_msg": error_msg
        })
    );
    match body {
        Ok(b) => Ok(warp::reply::html(b).into_response()),
        Err(e) => {
            let body = format!("Error rendering login page: {:?}", e);
            Ok(warp::http::Response::builder()
                .status(500)
                .body(body.into())
                .unwrap()
            )
        }
    }
}

pub async fn login_page_route(data: Arc<Mutex<CommonData>>) -> WarpResult {
    render_login_page(&data.lock().unwrap().hbs, None)
}

pub async fn do_login_route(data: Arc<Mutex<CommonData>>, form_data: HashMap<String, String>) -> WarpResult {
    let mut data = data.lock().unwrap();
    let password = form_data.get("password".into());
    if password.is_none() || password.unwrap() != "super secret" {
        log::info!("Password given: {:?}", &password);
        return render_login_page(&data.hbs, Some("Incorrect password"));
    }

    let session_id = "test";
    let cookie = format!("session_id={}; Path=/; HttpOnly; Max-Age={}", session_id, THIRTY_DAYS);
    data.session_id = Some(session_id.to_string());

    Ok(warp::http::Response::builder()
        .header("Location", "/admin")
        .header("content-type", "text/html; charset=utf-8")
        .header("set-cookie", cookie)
        .status(303)
        .body("".into())
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

