mod thumbnails;

use std::{
    fs::{remove_file, self},
    io::Cursor,
    path::{Path as OsPath, PathBuf},
};

use uuid::Uuid;
use serde::Deserialize;
use serde_json::json;
use image::io::Reader as ImageReader;

use axum::{
    body::{Full, Bytes},
    http::StatusCode,
    Json,
    extract::{
        Extension,
        Path,
        Form,
        Multipart,
        multipart::MultipartError,
    },
    response::{
        Html,
        Response,
        IntoResponse,
        Redirect,
    },
};
use tower_cookies::Cookies;
use chrono::prelude::*;

use crate::{
    SharedData, commondata::CommonData,
    article::storage,
};
use super::{
    empty_response,
    server_error,
    server_error_page,
};
use thumbnails::{
    get_image_list,
    ThumbsRemaining,
    ImageListEntry,
    NameParts,
};

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;
const SEE_OTHER: u16 = 303;

type HtmlOrRedirect = Result<(StatusCode, Html<String>), Redirect>;
type HtmlOrStatus = Result<(StatusCode, Html<String>), StatusCode>;

#[derive(Deserialize)]
pub struct LoginFormData {
    password: String,
}

struct UploadedImageData {
    file_name: String,
    bytes: Result<Bytes, MultipartError>,
}

macro_rules! ensure_logged_in {
    ($d:ident, $c:ident) => {
        if needs_to_log_in(&$d, &$c) { return redirect_to("/login"); }
    };
}

macro_rules! ensure_authorized {
    ($d:ident, $c:ident) => {
        if needs_to_log_in(&$d, &$c) { return Err(StatusCode::UNAUTHORIZED); }
    };
}

fn needs_to_log_in(data: &SharedData, cookies: &Cookies) -> bool {
    let data = data.lock().unwrap();
    let session_id = cookies
        .get("velum_session_id")
        .map(|c| c.value().to_string());
    let sid = data.session_id.as_ref();
    sid.is_none()
        || session_id.is_none()
        || sid.unwrap() != session_id.as_ref().unwrap()
}

fn sanitize_file_name(file_name: &str) -> String {
    file_name.replace(' ', "-")
}

pub fn redirect_to<T>(path: &'static str) -> Result<T, Redirect> {
    Err(Redirect::to(path))
}

fn render_login_page(
    data: &SharedData,
    error_msg: Option<&str>,
) -> HtmlOrRedirect  {
    let data = data.lock().unwrap();
    let blog_title = &data.config.blog_title;
    match data.hbs.render(
        "login",
        &json!({
            "body_class": "admin",
            "title": "Admin Login",
            "blog_title": blog_title,
            "error_msg": error_msg,
            "content_dir": &data.config.content_dir,
        })
    ) {
        Ok(rendered_page) => Ok((StatusCode::OK, Html(rendered_page))),
        Err(e) => Ok(server_error_page(
            &format!("Failed to render article in index. Error: {:?}", e))
        )
    }
}

pub async fn login_page_handler(
    Extension(data): Extension<SharedData>,
) -> impl IntoResponse {
    render_login_page(&data, None)
}

pub async fn do_login_handler(
    Form(form_data): Form<LoginFormData>,
    Extension(data): Extension<SharedData>,
) -> Result<Response<Full<Bytes>>, impl IntoResponse> {
    let mut mdata = data.lock().unwrap();

    let hash = mdata.config.secrets.admin_password_hash.as_ref();
    let hash = if hash.is_none() { "" } else { hash.unwrap().as_str() };
    let verified = bcrypt::verify(&form_data.password, hash).unwrap_or(false);

    if !verified {
        return Err(render_login_page(&data, Some("Incorrect password")));
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

pub async fn rebuild_index_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    ensure_logged_in!(data, cookies);

    let mut data = data.lock().unwrap();

    if let Err(e) = data.rebuild() {
        log::error!("Failed to rebuild article index index: {:?}", e);
        Ok(server_error_page(
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
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);

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
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);

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
    ensure_logged_in!(data, cookies);

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

pub async fn delete_image_handler(
    Path(path): Path<String>,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);
    let path = path.trim_start_matches('/');
    match NameParts::new(path) {
        Ok(parts) => {
            match ImageListEntry::thumbnail_file_name(&parts.file_name) {
                Ok(thumb_name) => {
                    let thumb_path = parts.dir.join(&thumb_name);
                    let (ri, rt) = (remove_file(path), remove_file(thumb_path));
                    if ri.is_err() {
                        log::error!("Failed to delete image {:?}: {:?}", path, ri.unwrap_err());
                        return Ok(server_error("Error deleting image"));
                    }
                    log::info!("Deleted image {:?}", path);
                    if rt.is_err() {
                        log::error!("Failed to delete thumbnail {:?}: {:?}", path, rt.unwrap_err());
                        return Ok(server_error("Error deleting image"));
                    }
                    log::info!("Deleted thumbnail {:?}", thumb_name);
                },
                Err(e) => {
                    log::error!("Failed to get thumbnail name from {:?}: {:?}", parts.file_name, e);
                    return Ok(server_error("Error deleting image"));
                }
            }
        },
        Err(e) => {
            log::error!("Failed to extract parts from {:?}: {:?}", path, e);
            return Ok(server_error("Error deleting image"));
        }
    }

    image_list_handler(Extension(data), cookies).await
}

fn get_thumbs_remaining(data: &CommonData) -> ThumbsRemaining {
    let count = data.thumb_progress.len();
    let total = data.initial_remaining_thumbs;
    ThumbsRemaining { total, count }
}

pub async fn check_thumb_progress (
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> Result<Json<ThumbsRemaining>, StatusCode> {
    ensure_authorized!(data, cookies);
    Ok(Json(get_thumbs_remaining(&data.lock().unwrap())))
}

fn get_current_images_dir(data: &CommonData) -> PathBuf {
    let mut dir = PathBuf::from(&data.config.content_dir).join("images");
    let dt = Local::now();
    let (y, m) = (dt.year().to_string(), dt.month().to_string());
    dir.push(&y);
    dir.push(&m);
    dir
}

async fn gather_fields(mut form_data: Multipart) -> Vec<UploadedImageData> {
    let mut fields = Vec::new();

    while let Ok(Some(field)) = form_data.next_field().await {
        let file_name = sanitize_file_name(field.file_name()
            .expect("Read image file name from form data"));
        let bytes = field.bytes().await;
        fields.push(UploadedImageData { file_name, bytes })
    }

    fields
}

fn save_file<P: AsRef<OsPath>>(file_name: P, bytes: &Bytes) -> Result<(), image::ImageError> {
    let img = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
    log::info!("Saving file {:?}", file_name.as_ref());
    img.save(file_name)?;
    Ok(())
}

pub async fn upload_image_handler (
    form_data: Multipart,
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);
    let dir = get_current_images_dir(&data.lock().unwrap());
    let fields = gather_fields(form_data).await;

    for field in fields.iter() {
        let path = dir.join(&field.file_name);

        if let Ok(bytes) = &field.bytes {
            if let Err(e) = fs::create_dir_all(&dir) {
                log::error!("Error creating image directory {:?}: {:?}", dir, e);
                return Ok(server_error("Error creating image directory"));
            } else if let Err(e) = save_file(&path, bytes) {
                log::error!("Error saving file {:?}: {:?}", path, e);
                return Ok(server_error("Error saving image file"));
            }
        } else {
            return Ok(server_error("Error reading uploaded form data"));
        }
    }

    image_list_handler(Extension(data), cookies).await
}

pub async fn admin_page_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    ensure_logged_in!(data, cookies);

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

pub async fn image_list_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);

    let (filenames, thumbs_remaining) = get_image_list(&data);
    let data = data.lock().unwrap();
    let keys: Vec<String> = filenames.keys().map(|k| k.0.to_string_lossy().to_string()).collect();
    match data.hbs.render(
        "_admin_image_list",
        &json!({
            "image_keys": keys,
            "images": filenames,
            "thumbs_remaining": thumbs_remaining,
        })
    ) {
        Ok(rendered_page) => {
            Ok((StatusCode::OK, Html(rendered_page)))
        },
        Err(e) => Ok(server_error(
            &format!("Failed to render image list. Error: {:?}", e))
        )
    }
}
