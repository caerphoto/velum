use std::{
    ffi::OsString,
    fmt,
    fs::{remove_file, self},
    io::Cursor,
    error::Error,
    path::{Path as OsPath, PathBuf},
    collections::HashMap
};

use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::json;
use walkdir::{DirEntry, WalkDir};
use image::{
    GenericImageView,
    ImageFormat,
    io::Reader as ImageReader,
    imageops::{resize, FilterType},
};

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
use futures::executor::block_on;
use chrono::prelude::*;

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
type HtmlOrStatus = Result<(StatusCode, Html<String>), StatusCode>;

#[derive(Deserialize)]
pub struct LoginFormData {
    password: String,
}

struct UploadedImageData {
    file_name: String,
    bytes: Result<Bytes, MultipartError>,
}

#[derive(Serialize)]
pub struct ThumbsRemaining {
    total: usize,
    count: usize,
}

#[derive(Clone)]
pub struct NameParts {
    path: PathBuf,
    dir: PathBuf,
    file_name: OsString,
}

impl NameParts {
    fn new<P: AsRef<OsPath>>(path: P) -> Result<Self, ThumbError> {
        let path = path.as_ref();
        match (path.parent(), path.file_name()) {
            (Some(p), Some(f)) => Ok(Self {
                    path: path.into(),
                    dir: p.into(),
                    file_name: f.into(),
                }),
            _ => Err(ThumbError::new(path))
        }
    }

    fn path_string(&self) -> String {
        self.dir.to_string_lossy().to_string()
    }
}

#[derive(Debug, PartialEq)]
enum ThumbErrorKind {
    Name,
    File,
    AlreadyExists,
}

#[derive(Debug)]
pub struct ThumbError {
    orig_file_name: String,
    kind: ThumbErrorKind,
    details: Option<image::ImageError>,
}

impl ThumbError {
    fn new<P: AsRef<OsPath>>(path: P) -> Self {
        Self {
            orig_file_name: path.as_ref().to_string_lossy().into(),
            kind: ThumbErrorKind::Name,
            details: None,
        }
    }
    fn exists<P: AsRef<OsPath>>(path: P) -> Self {
        Self {
            orig_file_name: path.as_ref().to_string_lossy().into(),
            kind: ThumbErrorKind::AlreadyExists,
            details: None,
        }
    }
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ThumbErrorKind::Name => write!(f, "thumbnail name error: {}", self.orig_file_name),
            ThumbErrorKind::File => write!(f, "thumbnail file error: {:?}", self.details),
            ThumbErrorKind::AlreadyExists => write!(f, "thumbnail {:?} already exists", self.orig_file_name),
        }
    }
}

impl Error for ThumbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl std::convert::From<image::ImageError> for ThumbError {
    fn from(error: image::ImageError) -> Self {
        Self {
            orig_file_name: String::new(),
            kind: ThumbErrorKind::File,
            details: Some(error),
        }
    }
}


#[derive(Debug, Serialize)]
pub struct ImageListEntry {
    pub thumbnail_file_name: String,
    pub orig_file_name: String,
}

impl ImageListEntry {
    fn new<P: AsRef<OsPath>>(file_name: P) -> Result<Self, ThumbError> {
        let file_name = file_name.as_ref();
        let thumbnail_file_name = Self::thumbnail_file_name(file_name)?
            .to_string_lossy()
            .to_string();
        let orig_file_name = file_name.to_string_lossy().to_string();
        Ok(Self { orig_file_name, thumbnail_file_name })
    }
    fn thumbnail_file_name<P: AsRef<OsPath>>(file_name: P) -> Result<PathBuf, ThumbError> {
        let file_name = file_name.as_ref();
        if let (Some(stem), Some(ext)) = (file_name.file_stem(), file_name.extension()) {
            Ok(PathBuf::from(
                stem.to_string_lossy().to_string()
                + THUMBNAIL_SUFFIX
                + "."
                + &ext.to_string_lossy()
            ))
        } else {
            Err(ThumbError::new(file_name))
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImageListDir {
    pub dir: String,
    pub file_names: Vec<ImageListEntry>,
}

impl ImageListDir {
    fn reslash<P: AsRef<OsPath>>(p: P) -> String {
        let s = p.as_ref().to_string_lossy();
        // These names are for output to HTML
        s.replace('\\', "/")
    }
    pub fn new(parts: &NameParts) -> Result<Self, ThumbError> {
        let entry = ImageListEntry::new(&parts.file_name)?;
        Ok(Self {
            dir: Self::reslash(&parts.dir),
            file_names: vec![entry],
        })
    }
    pub fn push<P: AsRef<OsPath>>(&mut self, file_name: P) -> Result<(), ThumbError> {
        let file_name = file_name.as_ref().to_string_lossy().to_string();
        let entry = ImageListEntry::new(&file_name)?;
        self.file_names.push(entry);
        Ok(())
    }
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

fn generate_thumb_path(parts: &NameParts) -> Result<PathBuf, ThumbError> {
    let thumb_name = ImageListEntry::thumbnail_file_name(&parts.file_name)?;
    let thumb_path = parts.dir.join(&thumb_name);
    if thumb_path.is_file() {
        Err(ThumbError::exists(&parts.path))
    } else {
        Ok(thumb_path)
    }
}

async fn create_thumbnail(parts: NameParts, index: usize, count: usize, data: SharedData) -> Result<(), ThumbError> {
    let progress_val = parts.path.clone();
    let ftsize = THUMB_SIZE as f64;
    let thumb_path = match generate_thumb_path(&parts) {
        Ok(p) => p,
        Err(e) => match e.kind {
            ThumbErrorKind::AlreadyExists => return Ok(()),
            _ => return Err(e),
        }
    };
    let result = match image::open(&parts.path) {
        Ok(img) => {
            let (w, h) = img.dimensions();
            let (w, h) = (w as f64, h as f64);
            let (tw, th) = if w > h {
                (ftsize as u32, (ftsize / (w / h)) as u32)
            } else {
                ((ftsize / (h / w)) as u32, ftsize as u32)
            };
            log::info!("[{}/{}] Creating thumbnail for {:?} ...", index, count, thumb_path);
            let thumb = resize(&img, tw, th, FilterType::Triangle);
            if let Err(e) = thumb.save_with_format(&thumb_path, ImageFormat::Jpeg) {
                log::error!("  ...failed to save thumbnail {:?}: {:?}", thumb_path, e);
                Err(e.into())
            } else {
                log::info!("  ...saved thumbnail {:?}", thumb_path);
                Ok(())
            }
        },
        Err(e) => {
            log::error!(
                "[{}/{}] Failed to open image {:?} for thumbnail generation: {:?}",
                index, count, parts.path, e
            );
            Err(e.into())
        }
    };

    data.lock().unwrap().thumb_progress.remove(&progress_val);

    result
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

fn get_current_images_dir(data: &CommonData) -> PathBuf {
    let mut dir = PathBuf::from(&data.config.content_dir).join("images");
    let dt = Local::now();
    let (y, m) = (dt.year().to_string(), dt.month().to_string());
    dir.push(&y);
    dir.push(&m);
    dir
}

fn save_file<P: AsRef<OsPath>>(file_name: P, bytes: &Bytes) -> Result<(), image::ImageError> {
    let img = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
    log::info!("Saving file {:?}", file_name.as_ref());
    img.save(file_name)?;
    Ok(())
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

fn is_valid_image_file(entry: &DirEntry) -> bool {
    let is_image = entry.path().extension()
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "jpg" || ext == "jpeg" || ext == "png" || ext == "gif"
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

pub fn get_image_list(data: &SharedData) -> (HashMap<String, ImageListDir>, ThumbsRemaining) {
    let dir = PathBuf::from(&data.lock().unwrap().config.content_dir).join("images");
    let iter = WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(is_valid_image_file);

    let mut thumbnail_futures = Vec::new();
    let mut image_files: Vec<NameParts> = Vec::new();
    for entry in iter {
        match entry {
            Ok(dir_entry) => {
                if dir_entry.file_type().is_dir() { continue; }
                let path = dir_entry.path();
                match NameParts::new(path) {
                    Ok(parts) => image_files.push(parts),
                    Err(e) => log::error!("Failed to create name parts from {:?}: {:?}", path, e),
                }
            },
            Err(e) => log::error!("Unable to read dir entry: {:?}", e),
        }
    }

    let mut existing_thumb_count = 0;
    let mut filenames: HashMap<String, ImageListDir> = HashMap::new();
    let count = image_files.len();
    for (i, parts) in image_files.iter().enumerate() {
        if let Err(e) = generate_thumb_path(parts) {
            if e.kind == ThumbErrorKind::AlreadyExists {
                existing_thumb_count += 1;
            }
        } else {
            data.lock().unwrap().thumb_progress.insert(parts.path.clone());
            thumbnail_futures.push(
                create_thumbnail(parts.clone(), i + 1, count, data.clone())
            );
        }

        if let Some(ild) = filenames.get_mut(&parts.path_string()) {
            if let Err(e) = ild.push(&parts.file_name) {
                log::error!("Failed to push file name/thumbnail to dirlist: {:?}", e)
            }
        } else {
            match ImageListDir::new(parts) {
                Ok(ild) => { filenames.insert(parts.path_string(), ild); },
                Err(e) => { log::error!("Failed to create new image list dir: {:?}", e); },
            }
        }
    }

    let remaining = image_files.len() - existing_thumb_count;
    data.lock().unwrap().initial_remaining_thumbs += remaining;

    // Generate all thumbnails in a separate thread, which is detached and left to do its thing
    tokio::task::spawn_blocking(move || {
        for f in thumbnail_futures {
            if let Err(e) = block_on(f) {
                log::error!("Failed to block on future: {:?}", e);
            }
        }
    });

    (filenames, ThumbsRemaining { count: remaining, total: remaining })
}

pub async fn image_list_handler(
    Extension(data): Extension<SharedData>,
    cookies: Cookies,
) -> HtmlOrStatus {
    ensure_authorized!(data, cookies);

    let (filenames, thumbs_remaining) = get_image_list(&data);
    let data = data.lock().unwrap();
    match data.hbs.render(
        "_admin_image_list",
        &json!({
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
