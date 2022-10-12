use uuid::Uuid;
use serde::Deserialize;
use serde_json::json;

use axum::{
    body::{Full, Bytes},
    http::StatusCode,
    extract::{Extension, Path, Form},
    response::{Html, Response, IntoResponse, Redirect},
};
use tower_cookies::Cookies;

use crate::SharedData;
use crate::article::storage;
use super::{
    server_error,
    empty_response,
    theme,
};

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;
const SEE_OTHER: u16 = 303;

type HtmlOrRedirect = Result<(StatusCode, Html<String>), Redirect>;

#[derive(Deserialize)]
pub struct LoginFormData {
    password: String,
}

fn needs_to_log_in(data: &SharedData, cookies: Cookies) -> bool {
    let data = data.lock().unwrap();
    let session_id = cookies
        .get("velum_session_id")
        .map(|c| c.value().to_string());
    let sid = data.session_id.as_ref();
    sid.is_none()
        || session_id.is_none()
        || sid.unwrap() != session_id.as_ref().unwrap()
}

pub fn redirect_to(path: &'static str) -> HtmlOrRedirect {
    Err(Redirect::to(path))
}

fn render_login_page(
    data: &SharedData,
    error_msg: Option<&str>,
    theme: Option<String>,
) -> HtmlOrRedirect  {
    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;
    match data.hbs.render(
        "login",
        &json!({
            "body_class": "login",
            "title": "Admin Login",
            "blog_title": blog_title,
            "error_msg": error_msg,
            "content_dir": &data.config.content_dir,
            "theme": theme,
        })
    ) {
        Ok(rendered_page) => Ok((StatusCode::OK, Html(rendered_page))),
        Err(e) => Ok(server_error(
            &format!("Failed to render article in index. Error: {:?}", e))
        )
    }
}

pub async fn login_page_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> impl IntoResponse {
    render_login_page(&data, None, theme(cookies))
}

pub async fn do_login_handler(
    Form(form_data): Form<LoginFormData>,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> Result<Response<Full<Bytes>>, impl IntoResponse> {
    let mut mdata = data.lock().unwrap();

    let hash = mdata.config.secrets.admin_password_hash.as_ref();
    let hash = if hash.is_none() { "" } else { hash.unwrap().as_str() };
    let verified = bcrypt::verify(&form_data.password, hash).unwrap_or(false);

    if !verified {
        return Err(render_login_page(&data, Some("Incorrect password"), theme(cookies)));
    }

    let session_id = Uuid::new_v4();
    let cookie = format!(
        "velum_session_id={}; Path=/; HttpOnly; Max-Age={}",
        session_id,
        THIRTY_DAYS
    );
    mdata.session_id = Some(session_id.to_string());

    Ok(Response::builder()
        .header("Location", "/admin")
        .header("Set-Cookie", cookie)
        .status(SEE_OTHER)
        .body("".into()) // body can't be () because we might render login
        .unwrap()
    )
}

pub async fn do_logout_handler(
    Extension(data): Extension<SharedData>,
) -> Response<Full<Bytes>> {
    let mut data = data.lock().unwrap();

    // Note expiry date: setting a date in the past is the spec-compliant way
    // to force the browser to delete the cookie.
    let cookie = "velum_session_id=; Path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
    data.session_id = None;

    Response::builder()
        .header("Location", "/")
        .header("Set-Cookie", cookie)
        .status(SEE_OTHER)
        .body("".into())
        .unwrap()
}

pub async fn admin_page_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    if needs_to_log_in(&data, cookies) { return redirect_to("/login"); }

    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;
    match data.hbs.render(
        "admin",
        &json!({
            "body_class": "admin",
            "title": "Blog Admin",
            "blog_title": blog_title,
            "articles": &data.articles,
            "content_dir": &data.config.content_dir,
        })
    ) {
        Ok(rendered_page) => Ok((
            StatusCode::OK,
            Html(rendered_page),
        )),
        Err(e) => Ok(server_error(
            &format!("Failed to render article in index. Error: {:?}", e))
        )
    }
}

pub async fn rebuild_index_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    if needs_to_log_in(&data, cookies) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Err(e) = data.rebuild() {
        log::error!("Failed to rebuild article index index: {:?}", e);
        Ok(server_error(
            &format!("Failed to render article in index. Error: {:?}", e)
        ))
    } else {
        redirect_to("/admin")
    }
}

pub async fn create_article_handler(
    content: String,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    if needs_to_log_in(&data, cookies) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    match storage::create_article(&content, &mut data) {
        Ok(view) => {
            log::info!("Created article '{}' on disk.", view.slug);
            if let Err(err) = data.rebuild() {
                log::error!("Failed to rebuild article index: {:?}", err);
                Ok(server_error("Error rebuilding article index"))
            } else {
                match data.hbs.render(
                    "_admin_article_list_item",
                    &view
                ) {
                    Ok(b) =>  Ok((StatusCode::OK, Html(b))),
                    Err(e) => {
                        log::error!("Failed to render list item: {:?}", e);
                        Ok(server_error("Error rendering new item for list"))
                    }
                }
            }
        },
        Err(err) => {
            log::error!("Failed to create article: {:?}", err);
            Ok(server_error("Error creating article"))
        }
    }
}

pub async fn update_article_handler(
    Path(slug): Path<String>,
    new_content: String,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    if needs_to_log_in(&data, cookies) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Err(err) = storage::update_article(&slug, &new_content, &mut data) {
        log::error!("Failed to update article: {:?}", err);
        
        Ok(server_error("Error upating article"))
    } else {
        log::info!("Updated article '{}' on disk.", &slug);
        if let Err(err) = data.rebuild() {
            log::error!("Failed to rebuild article index: {:?}", err);
            Ok(server_error("Error rebuilding article index"))
        } else {
            Ok(empty_response(StatusCode::OK))
        }
    }
}

pub async fn delete_article_handler(
    Path(slug): Path<String>,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    if needs_to_log_in(&data, cookies) { return redirect_to("/login"); }

    let mut data = data.lock().unwrap();

    if let Some(article) = storage::fetch_by_slug(&slug, &data.articles) {
        if let Err(err) = storage::delete_article(article) {
            log::error!("Failed to delete article: {:?}", err);
            Ok(server_error("Error deleting article"))
        } else {
            log::info!("Deleted article '{}' from disk.", &slug);
            if let Err(err) = data.rebuild() {
                log::error!("Failed to rebuild article index: {:?}", err);
                Ok(server_error("Error rebuilding article index"))
            } else {
                Ok(empty_response(StatusCode::OK))
            }
        }
    } else {
        Ok(empty_response(StatusCode::NOT_FOUND))
    }
}
