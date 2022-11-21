use std::{
    fmt,
    error::Error,
    path::{Path as OsPath, PathBuf},
    collections::HashMap
};

use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::json;
use walkdir::{DirEntry, WalkDir};
use image::{GenericImageView, ImageFormat, imageops::{resize, FilterType}};

use axum::{
    body::{Full, Bytes},
    http::StatusCode,
    extract::{Extension, Path, Form},
    response::{Html, Response, IntoResponse, Redirect},
};
use tower_cookies::Cookies;

use crate::{SharedData, commondata::CommonData};
use crate::article::storage;
use super::{
    server_error,
    empty_response,
};

const THIRTY_DAYS: i64 = 60 * 60 * 24 * 30;
const SEE_OTHER: u16 = 303;
const THUMBNAIL_SUFFIX: &str = "_thumbnail";
const THUMB_SIZE: u32 = 150;

type HtmlOrRedirect = Result<(StatusCode, Html<String>), Redirect>;


#[derive(Debug)]
pub struct ThumbNameError {
    orig_file_name: String,
}

impl fmt::Display for ThumbNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unable to extract thumbnail file name from {}", self.orig_file_name)
    }
}

impl Error for ThumbNameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Deserialize)]
pub struct LoginFormData {
    password: String,
}

#[derive(Debug, Serialize)]
pub struct ImageListEntry {
    pub thumbnail_file_name: String,
    pub orig_file_name: String,
}

impl ImageListEntry {
    fn new(orig_file_name: &str) -> Result<Self, ThumbNameError> {
        let thumbnail_file_name = Self::thumbnail_file_name(&PathBuf::from(orig_file_name))?;
        let thumbnail_file_name = thumbnail_file_name.to_string_lossy().to_string();
        let orig_file_name = String::from(orig_file_name);
        Ok(Self { orig_file_name, thumbnail_file_name })
    }
    fn thumbnail_file_name(file_name: &OsPath) -> Result<PathBuf, ThumbNameError> {
        if let (Some(stem), Some(ext)) = (file_name.file_stem(), file_name.extension()) {
            Ok(PathBuf::from(
                stem.to_string_lossy().to_string()
                + THUMBNAIL_SUFFIX
                + "."
                + &ext.to_string_lossy()
            ))
        } else {
            Err(ThumbNameError { orig_file_name: file_name.to_string_lossy().into() })
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImageListDir {
    pub dir: String,
    pub file_names: Vec<ImageListEntry>,
}

impl ImageListDir {
    fn reslash(s: &str) -> String {
        // These names are for output to HTML
        s.replace('\\', "/")
    }
    fn parts(path: &OsPath) -> (Option<String>, Option<String>) {
        let mut p: Option<String> = None;
        let mut f: Option<String> = None;
        if let Some(parent) = path.parent() {
            p = Some(parent.to_string_lossy().into());
        }
        if let Some(file_name) = path.file_name() {
            f = Some(file_name.to_string_lossy().into());
        }
        (p, f)
    }

    pub fn new(parent: &str, file_name: &str) -> Result<Self, ThumbNameError> {
        let entry = ImageListEntry::new(file_name)?;
        Ok(Self {
            dir: Self::reslash(parent),
            file_names: vec![entry],
        })
    }
    pub fn push(&mut self, file_name: String) -> Result<(), ThumbNameError> {
        let entry = ImageListEntry::new(&file_name)?;
        self.file_names.push(entry);
        Ok(())
    }
}

macro_rules! ensure_logged_in {
    ($d:ident, $c:ident) => {
        if needs_to_log_in(&$d, $c) { return redirect_to("/login"); }
    };
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

fn create_thumbnail(path: &OsPath, count: usize) {
    let ftsize = THUMB_SIZE as f64;
    let thumb_path = ImageListEntry::thumbnail_file_name(path);
    let (dir, _) = ImageListDir::parts(path);
    if thumb_path.is_err() || dir.is_none() {
        return;
    }
    let thumb_path = PathBuf::from(dir.unwrap()).join(&thumb_path.unwrap());
    if thumb_path.is_file() {
        log::info!("[{}] Skipping existing thumbnail {:?}", count, thumb_path);
        return;
    }
    match image::open(path) {
        Ok(img) => {
            let (w, h) = img.dimensions();
            let (w, h) = (w as f64, h as f64);
            let (tw, th) = if w > h {
                (ftsize as u32, (ftsize / (w / h)) as u32)
            } else {
                ((ftsize / (h / w)) as u32, ftsize as u32)
            };
            log::info!("[{}] Creating thumbnail for {:?} ...", count, path);
            let thumb = resize(&img, tw, th, FilterType::Triangle);
            if let Err(e) = thumb.save_with_format(&thumb_path, ImageFormat::Jpeg) {
                log::error!("  ...failed to save thumbnail {:?}: {:?}", thumb_path, e);
            } else {
                log::info!("  ...saved thumbnail {:?}", thumb_path);
            }
        },
        Err(e) => {
            log::error!(
                "[{}] Failed to open image {:?} for thumbnail generation: {:?}",
                count, path, e
            );
        }
    }
}

pub fn redirect_to(path: &'static str) -> HtmlOrRedirect {
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
        Err(e) => Ok(server_error(
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

pub async fn rebuild_index_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    ensure_logged_in!(data, cookies);

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
    ensure_logged_in!(data, cookies);

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
    ensure_logged_in!(data, cookies);

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

fn is_valid_image_file(entry: &DirEntry) -> bool {
    let is_image = entry.path().extension()
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "jpg" || ext == "png" || ext == "gif"
        })
        .unwrap_or(false);
    let is_thumb = entry.path().file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            stem.ends_with(THUMBNAIL_SUFFIX)
        })
        .unwrap_or(true);


    (is_image || entry.file_type().is_dir()) && !is_thumb
}

pub fn get_image_list(data: &CommonData) -> HashMap<String, ImageListDir> {
    let mut filenames: HashMap<String, ImageListDir> = HashMap::new();

    let dir = PathBuf::from(&data.config.content_dir).join("images");
    let iter = WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(is_valid_image_file);

    for (count, entry) in iter.enumerate() {
        match entry {
            Ok(e) => {
                if e.file_type().is_dir() { continue; }

                let path = e.path();
                create_thumbnail(path, count);
                if let (Some(parent), Some(file_name)) = ImageListDir::parts(path) {
                    if let Some(ild) = filenames.get_mut(&parent) {
                        if let Err(e) = ild.push(file_name) {
                            log::error!("Failed to push file name/thumbnail to dirlist: {:?}", e)
                        }
                    } else {
                        match ImageListDir::new(&parent, &file_name) {
                            Ok(ild) => { filenames.insert(parent, ild); },
                            Err(e) => { log::error!("Failed to create new image list dir: {:?}", e); },
                        }
                    }
                }
            },
            Err(e) => log::error!("Unable to read dir entry: {:?}", e),
        }
    }

    filenames
}

pub async fn image_list_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrRedirect {
    ensure_logged_in!(data, cookies);
    let data = data.lock().unwrap();
    let filenames = get_image_list(&data);
    match data.hbs.render(
        "_admin_image_list",
        &json!({
            "images": filenames,
        })
    ) {
        Ok(rendered_page) => Ok((StatusCode::OK, Html(rendered_page))),
        Err(e) => Ok(server_error(
            &format!("Failed to render image list. Error: {:?}", e))
        )
    }
}
