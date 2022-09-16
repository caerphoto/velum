use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use bytes::Bytes;
use crate::CommonData;
use warp::{Reply, http::Uri};
use serde_json::json;
use uuid::Uuid;
use super::WarpResult;
use crate::article::storage;

const OK: u16 = 200;
const SEE_OTHER: u16 = 303;
const BAD_REQUEST: u16 = 400;
const NOT_FOUND: u16 = 404;
const INTERNAL_SERVER_ERROR: u16 = 500;

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;

fn server_error(msg: &str) -> WarpResult {
    log::error!("{}", msg);
    Ok(warp::http::Response::builder()
        .status(INTERNAL_SERVER_ERROR)
        .body("Internal server error :(".into())
        .unwrap()
    )
}

fn redirect_to(path: &'static str) -> WarpResult {
    Ok(warp::redirect::found(Uri::from_static(path)).into_response())
}

fn empty_response(status: u16) -> WarpResult {
    Ok(
        warp::reply::with_status(
            warp::reply(),
            warp::http::StatusCode::from_u16(status).unwrap()
        ).into_response()
    )
}

fn needs_to_log_in(data: &CommonData, session_id: Option<String>) -> bool {
    let sid = data.session_id.as_ref();
    sid.is_none()
        || session_id.is_none()
        || sid.unwrap() != session_id.as_ref().unwrap()
}

fn render_login_page(hbs: &handlebars::Handlebars, error_msg: Option<&str>) -> WarpResult {
    let body = hbs.render(
        "login",
        &json!({
            "body_class": "login",
            "title": "Admin Login",
            "error_msg": error_msg
        })
    );
    match body {
        Ok(b) => Ok(warp::reply::html(b).into_response()),
        Err(e) => {
            log::error!("Failed to render login page: {:?}", e);
            server_error("Error rendering login page")
        }
    }
}

pub async fn login_page_route(data: Arc<Mutex<CommonData>>) -> WarpResult {
    render_login_page(&data.lock().unwrap().hbs, None)
}

pub async fn do_login_route(data: Arc<Mutex<CommonData>>, form_data: HashMap<String, String>) -> WarpResult {
    let mut data = data.lock().unwrap();

    let password = form_data.get("password");
    let password = if password.is_none() { "" } else { password.unwrap().as_str() };
    let hash = data.config.admin_password_hash.as_ref();
    let hash = if hash.is_none() { "" } else { hash.unwrap().as_str() };
    let verified = bcrypt::verify(&password, hash).unwrap_or(false);

    if !verified {
        return render_login_page(&data.hbs, Some("Incorrect password"));
    }

    let session_id = Uuid::new_v4();
    let cookie = format!(
        "session_id={}; Path=/; HttpOnly; Max-Age={}",
        session_id,
        THIRTY_DAYS
    );
    data.session_id = Some(session_id.to_string());

    Ok(warp::http::Response::builder()
        .header("Location", "/admin")
        .header("Set-Cookie", cookie)
        .status(SEE_OTHER)
        .body("".into()) // body can't be () because we might render login
        .unwrap()
    )
}

pub async fn do_logout_route(data: Arc<Mutex<CommonData>>) -> WarpResult {
    let mut data = data.lock().unwrap();

    // Note expiry date: setting a date in the past is the spec-compliant way
    // to force the browser to delete the cookie.
    let cookie = "session_id=; Path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
    data.session_id = None;

    Ok(warp::http::Response::builder()
        .header("Location", "/")
        .header("Set-Cookie", cookie)
        .status(SEE_OTHER)
        .body("".into())
        .unwrap()
    )
}

pub async fn admin_route(data: Arc<Mutex<CommonData>>, session_id: Option<String>) -> WarpResult {
    let data = data.lock().unwrap();
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let body = data.hbs.render(
        "admin",
        &json!({
            "body_class": "admin",
            "title": "Blog Admin",
            "articles": &data.articles,
        })
    );
    match body {
        Ok(b) => Ok(warp::reply::html(b).into_response()),
        Err(e) => {
            log::error!("Failed to render admin page: {:?}", e);
            server_error("Error rendering admin page")
        }
    }
}

pub async fn rebuild_index_route(data: Arc<Mutex<CommonData>>, session_id: Option<String>) -> WarpResult {
    let mut data = data.lock().unwrap();
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    if let Err(e) = data.rebuild() {
        log::error!("Failed to rebuild article index index: {:?}", e);
        server_error("Error rebuilding article index")
    } else {
        redirect_to("/admin")
    }

}

pub async fn update_article_route(
    slug: String,
    new_content: Bytes,
    data: Arc<Mutex<CommonData>>,
    session_id: Option<String>,
) -> WarpResult {
    let mut data = data.lock().unwrap();
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    if let Ok(new_content) = String::from_utf8(new_content.to_vec()) {
        if let Err(err) = storage::update_article(&slug, &new_content, &mut data) {
            log::error!("Failed to update article: {:?}", err);
            server_error("Error upating article")
        } else {
            empty_response(OK)
        }
    } else {
        empty_response(BAD_REQUEST)
    }
}

pub async fn delete_article_route(
    slug: String,
    data: Arc<Mutex<CommonData>>,
    session_id: Option<String>,
) -> WarpResult {
    let mut data = data.lock().unwrap();
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    if let Some(article) = storage::fetch_by_slug(&slug, &data.articles) {
        if let Err(err) = storage::delete_article(article) {
            log::error!("Failed to delete article: {:?}", err);
            server_error("Error deleting article")
        } else {
            log::info!("Deleted article '{}' from disk.", &slug);
            if let Err(err) = data.rebuild() {
                log::error!("Failed to rebuild article index: {:?}", err);
                server_error("Error rebuilding article index")
            } else {
                empty_response(OK)
            }
        }
    } else {
        empty_response(NOT_FOUND)
    }
}
