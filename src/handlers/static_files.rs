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
    },
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
    body::{Full, Bytes},
    extract::{Path, Extension},
    response::Response,
};
use tower_cookies::Cookies;

use crate::SharedData;
use super::{
    HtmlResponse,
    server_error,
};

const ONE_YEAR: Duration = Duration::new(31_536_000, 0);

fn read_file_bytes<P: AsRef<FsPath>>(filename: P, buf: &mut Vec<u8>) -> IoResult<LastModified> {
    let mut f = fs::File::open(filename)?;
    let meta = f.metadata()?;
    let modified = meta.modified()?;
    f.read_to_end(buf)?;

    Ok(LastModified::from(modified))
}

fn extract_filepaths<P: AsRef<FsPath>>(manifest_path: P) -> Result<(Vec<PathBuf>, String), HtmlResponse> {

    let mut filepaths: Vec<PathBuf> = Vec::new();
    let manifest = fs::File::open(manifest_path)
        .map_err(|_| server_error("Failed to open manifest file"))?;
    let mut manifest_code: Vec<String> = Vec::new();
    for line in BufReader::new(manifest).lines() {
        if line.is_err() { continue; }
        let line = line.unwrap();
        if let Some(p) = line.strip_prefix("//=") {
            filepaths.push((p.to_string() + ".js").into())
        } else {
            manifest_code.push(line.into())
        }
    }
    Ok((filepaths, String::from(";") + &manifest_code.join("\n")))
}

fn concat_files<P: AsRef<FsPath>>(paths: Vec<P>, mut buf: &mut Vec<u8>) -> Result<LastModified, HtmlResponse> {
    let init = LastModified::from(UNIX_EPOCH);
    let last_modified = paths.iter()
        .map(|p| {

            if let Ok(last_modified) = read_file_bytes(p, &mut buf) {
                last_modified
            } else {
                init
            }
        })
        .fold(init, std::cmp::max);

    Ok(last_modified)
}

fn read_manifest_file_bytes(manifest_path: PathBuf, buf: &mut Vec<u8>) -> Result<LastModified, HtmlResponse> {
    let prefix = match manifest_path.parent() {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("/")
    };
    let (paths, manifest_js) = extract_filepaths(manifest_path)
        .map_err(|_| server_error("Failed to extract file paths"))?;
    let paths = paths.iter()
        .map(|p| prefix.join(p))
        .collect();
    log::info!("Extracted file paths: {:?}", &paths);
    let last_modified = concat_files(paths, buf)
        .map_err(|_| server_error("Failed to concatenate files"))?;
    buf.append(&mut Vec::from(manifest_js.as_bytes()));

    Ok(last_modified)
}

pub async fn asset_handler(
    Path(filename): Path<String>,
    Extension(data): Extension<SharedData>,
    _cookies: Cookies,
) -> Result<Response<Full<Bytes>>, HtmlResponse> {
    lazy_static! {
        static ref DATE_PART: Regex = Regex::new(r"-\d{14}").unwrap();
    }

    let data = data.lock().unwrap();
    let filename = filename.trim_start_matches('/').to_string();
    let new_name = if DATE_PART.is_match(&filename) {
        DATE_PART.replace(&filename, "").to_string()
    } else {
        filename.clone()
    };

    let real_path = PathBuf::from(&data.config.content_dir)
        .join("assets")
        .join(&new_name);

    log::info!(
        "Serving timestamped assset {} from file {}",
        &filename,
        &real_path.to_string_lossy()
    );

    // This is simplified version of what Warp's private function `file_reply` does. See:
    // https://github.com/seanmonstar/warp/blob/master/src/filters/fs.rs#L261
    let mut buf = Vec::new();
    let last_modified;
    if real_path.ends_with("manifest.js") {
        last_modified = read_manifest_file_bytes(real_path, &mut buf)?;
    } else{
        last_modified = read_file_bytes(&real_path, &mut buf)
            .map_err(|_| {
                log::error!("Failed to read bytes of {:?}", real_path);
                server_error("Failed to read bytes of file")
            })?;
    }

    let ct = mime_guess::from_path(&new_name).first_or_octet_stream();
    let len = buf.len() as u64;
    let mut res = Response::builder()
        .status(200)
        .body(Full::from(buf))
        .unwrap();
    let headers = res.headers_mut();
    headers.typed_insert(ContentLength(len));
    headers.typed_insert(CacheControl::new().with_max_age(ONE_YEAR));
    headers.typed_insert(last_modified);
    headers.typed_insert(ContentType::from(ct));
    Ok(res)
}
