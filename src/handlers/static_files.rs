use std::{
    io::{
        prelude::*,
        BufReader,
        Read,
        Result as IoResult,
    },
    fs,
    path::{
        Path as FsPath,
        PathBuf,
    },
    time::{
        UNIX_EPOCH,
        Duration,
        SystemTime,
    },
};

use filetime::{
    FileTime,
    set_file_mtime,
};

use headers::{
    HeaderMapExt,
    CacheControl,
    ContentLength,
    ContentType,
    LastModified,
};
use regex::Regex;

use axum::{
    body::{
        boxed,
        Body,
        Full as FullBody,
        BoxBody
    },
    extract::{Path, Extension},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get_service,
};
use axum_macros::debug_handler;
use tower_cookies::Cookies;
use tower_http::services::ServeFile;
use tower::ServiceExt;

use crate::SharedData;
use super::{
    HtmlResponse,
    server_error,
};

const ONE_YEAR: Duration = Duration::new(31_536_000, 0);

fn read_file_bytes<P: AsRef<FsPath>>(filename: P, buf: &mut Vec<u8>) -> IoResult<SystemTime> {
    let mut f = fs::File::open(filename)?;
    let meta = f.metadata()?;
    let modified = meta.modified()?;
    f.read_to_end(buf)?;

    Ok(modified)
}

fn concat_files<P: AsRef<FsPath>>(paths: Vec<P>, buf: &mut Vec<u8>) -> Result<SystemTime, HtmlResponse> {
    let separator = b';';
    let last_modified = paths.iter()
        .map(|p| {

            if let Ok(last_modified) = read_file_bytes(p, buf) {
                buf.push(separator);
                last_modified
            } else {
                UNIX_EPOCH
            }
        })
        .fold(UNIX_EPOCH, std::cmp::max);

    Ok(last_modified)
}

fn extract_filepaths(manifest_path: &PathBuf) -> Result<(Vec<PathBuf>, String), HtmlResponse> {
    let mut filepaths: Vec<PathBuf> = Vec::new();
    let manifest = fs::File::open(manifest_path)
        .map_err(|_| server_error(
            &format!(
                "Failed to open manifest file {}",
                manifest_path.to_string_lossy()
            )))?;
    let mut manifest_code: Vec<String> = Vec::new();
    for line in BufReader::new(manifest).lines() {
        if line.is_err() { continue; }
        let line = line.unwrap();
        if let Some(p) = line.strip_prefix("//=") {
            filepaths.push((p.to_string() + ".js").into())
        } else {
            manifest_code.push(line)
        }
    }
    Ok((filepaths, String::from(";") + &manifest_code.join("\n")))
}

fn compile_manifest(manifest_path: &PathBuf, buf: &mut Vec<u8>) -> Result<SystemTime, HtmlResponse> {
    let prefix = match manifest_path.parent() {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("/")
    };
    let (paths, manifest_js) = extract_filepaths(manifest_path)
        .map_err(|_| server_error("Failed to extract file paths"))?;
    let paths = paths.iter()
        .map(|p| prefix.join(p))
        .collect();
    let last_modified = concat_files(paths, buf)
        .map_err(|_| server_error("Failed to concatenate files"))?;
    buf.append(&mut Vec::from(manifest_js.as_bytes()));
    if let Err(e) = set_file_mtime(manifest_path, FileTime::from_system_time(last_modified)) {
        log::error!("Failed to update last modified time on JS manifest: {:?}", e);
    }

    Ok(last_modified)
}

fn build_response(filename: &PathBuf, last_modified: SystemTime, buf: Vec<u8>) -> Response<BoxBody> {
    let ct = mime_guess::from_path(filename).first_or_octet_stream();
    let len = buf.len() as u64;
    let mut res = Response::builder()
        .status(200)
        .body(boxed(FullBody::from(buf)))
        .unwrap();
    let headers = res.headers_mut();
    headers.typed_insert(ContentLength(len));
    headers.typed_insert(CacheControl::new().with_max_age(ONE_YEAR));
    headers.typed_insert(LastModified::from(last_modified));
    headers.typed_insert(ContentType::from(ct));
    res
}

fn normalize_path(path: &str) -> PathBuf {
    lazy_static! {
        static ref DATE_PART: Regex = Regex::new(r"-\d{14}").unwrap();
    }

    let npath = path.trim_start_matches('/');
    if DATE_PART.is_match(npath) {
        // Note: replace returns Cow<str>, not &str
        PathBuf::from(DATE_PART.replace(npath, "").as_ref())
    } else {
        PathBuf::from(npath)
    }
}

async fn error_handler(error: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", error),
    )
}

fn js_manifest_response(path: &PathBuf) -> Result<Response<BoxBody>, HtmlResponse> {
    let mut buf = Vec::new();
    let last_modified = compile_manifest(path, &mut buf)?;
    Ok(build_response(path, last_modified, buf))
}

#[debug_handler]
pub async fn asset_handler(
    Path(path): Path<String>,
    Extension(data): Extension<SharedData>,
    _cookies: Cookies,
    req: Request<Body>,
) -> Result<Response<BoxBody>, HtmlResponse> {
    // Need to clone to ensure a reference is not held across an await.
    let dir = data.read().config.content_dir.clone();

    let npath = normalize_path(&path);
    let real_path = PathBuf::from(dir)
        .join("assets")
        .join(&npath);

    log::info!(
        "Serving assset {} from file {}",
        &path,
        &real_path.to_string_lossy()
    );

    if npath.ends_with("manifest.js") {
        js_manifest_response(&real_path)
    } else {
        let service = get_service(ServeFile::new(real_path))
            .handle_error(error_handler);
        let result = service.oneshot(req).await;
        Ok(result.unwrap())
    }
}
