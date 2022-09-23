use std::collections::HashMap;
use bytes::Bytes;
use warp::Reply;
use serde_json::json;
use uuid::Uuid;
use super::{
    WarpResult,
    SharedData,
    BAD_REQUEST,
    server_error,
    redirect_to,
    empty_response,
};
use crate::article::storage;

const OK: u16 = 200;
const SEE_OTHER: u16 = 303;
const NOT_FOUND: u16 = 404;

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;


fn needs_to_log_in(data: &SharedData, session_id: Option<String>) -> bool {
    let data = data.lock().unwrap();
    let sid = data.session_id.as_ref();
    sid.is_none()
        || session_id.is_none()
        || sid.unwrap() != session_id.as_ref().unwrap()
}

fn render_login_page(data: &SharedData, error_msg: Option<&str>) -> WarpResult {
    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;
    let body = data.hbs.render(
        "login",
        &json!({
            "body_class": "login",
            "title": "Admin Login",
            "blog_title": blog_title,
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

pub async fn login_page_route(data: SharedData) -> WarpResult {
    render_login_page(&data, None)
}

pub async fn do_login_route(form_data: HashMap<String, String>, data: SharedData) -> WarpResult {
    let mut mdata = data.lock().unwrap();

    let password = form_data.get("password");
    let password = if password.is_none() { "" } else { password.unwrap().as_str() };
    let hash = mdata.config.secrets.admin_password_hash.as_ref();
    let hash = if hash.is_none() { "" } else { hash.unwrap().as_str() };
    let verified = bcrypt::verify(&password, hash).unwrap_or(false);

    if !verified {
        return render_login_page(&data, Some("Incorrect password"));
    }

    let session_id = Uuid::new_v4();
    let cookie = format!(
        "session_id={}; Path=/; HttpOnly; Max-Age={}",
        session_id,
        THIRTY_DAYS
    );
    mdata.session_id = Some(session_id.to_string());

    Ok(warp::http::Response::builder()
        .header("Location", "/admin")
        .header("Set-Cookie", cookie)
        .status(SEE_OTHER)
        .body("".into()) // body can't be () because we might render login
        .unwrap()
    )
}

pub async fn do_logout_route(data: SharedData) -> WarpResult {
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

pub async fn admin_route(session_id: Option<String>, data: SharedData) -> WarpResult {
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;
    let body = data.hbs.render(
        "admin",
        &json!({
            "body_class": "admin",
            "title": "Blog Admin",
            "blog_title": blog_title,
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

pub async fn rebuild_index_route(session_id: Option<String>, data: SharedData) -> WarpResult {
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Err(e) = data.rebuild() {
        log::error!("Failed to rebuild article index index: {:?}", e);
        server_error("Error rebuilding article index")
    } else {
        redirect_to("/admin")
    }

}

pub async fn create_article_route(
    content: Bytes,
    session_id: Option<String>,
    data: SharedData,
) -> WarpResult {
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Ok(content) = String::from_utf8(content.to_vec()) {
        match storage::create_article(&content, &mut data) {
            Ok(view) => {
                log::info!("Created article '{}' on disk.", view.slug);
                if let Err(err) = data.rebuild() {
                    log::error!("Failed to rebuild article index: {:?}", err);
                    server_error("Error rebuilding article index")
                } else {
                    let body = data.hbs.render(
                        "admin_article_list_item",
                        &view
                    );
                    match body {
                        Ok(b) =>  Ok(warp::reply::html(b).into_response()),
                        Err(e) => {
                            log::error!("Failed to render list item: {:?}", e);
                            server_error("Error rendering new item for list")
                        }
                    }
                }
            },
            Err(err) => {
                log::error!("Failed to create article: {:?}", err);
                server_error("Error creating article")
            }
        }
    } else {
        empty_response(BAD_REQUEST)
    }
}

pub async fn update_article_route(
    slug: String,
    new_content: Bytes,
    session_id: Option<String>,
    data: SharedData,
) -> WarpResult {
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Ok(new_content) = String::from_utf8(new_content.to_vec()) {
        if let Err(err) = storage::update_article(&slug, &new_content, &mut data) {
            log::error!("Failed to update article: {:?}", err);
            server_error("Error upating article")
        } else {
            log::info!("Updated article '{}' on disk.", &slug);
            if let Err(err) = data.rebuild() {
                log::error!("Failed to rebuild article index: {:?}", err);
                server_error("Error rebuilding article index")
            } else {
                empty_response(OK)
            }
        }
    } else {
        empty_response(BAD_REQUEST)
    }
}

pub async fn delete_article_route(
    slug: String,
    session_id: Option<String>,
    data: SharedData,
) -> WarpResult {
    if needs_to_log_in(&data, session_id) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

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
