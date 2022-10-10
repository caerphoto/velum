use std::{
    io::{

        Read,
        Result as IoResult,
    },
    fs,
    path::PathBuf,
    time::{
        self,
        Duration,
        SystemTime,
        UNIX_EPOCH
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
    http::StatusCode,
    response::{Html, Response, IntoResponse},
    extract::{Path, Extension},
    body::{Full, Bytes}
};
use tower_cookies::Cookies;

use crate::SharedData;

const ONE_YEAR: Duration = Duration::new(31_536_000, 0);
fn read_file_bytes(filename: &PathBuf, buf: &mut Vec<u8>) -> IoResult<LastModified> {
    let mut f = fs::File::open(filename)?;
    let meta = f.metadata()?;
    let modified = meta.modified()?;
    f.read_to_end(buf)?;

    Ok(LastModified::from(modified))
}

pub async fn asset_handler(

    Path(timestamped_name): Path<String>,
    Extension(data): Extension<SharedData>,
    _cookies: Cookies,
)-> Result<Response<Full<Bytes>>, impl IntoResponse> {
    lazy_static! {
        static ref DATE_PART: Regex = Regex::new(r"-\d{14}").unwrap();
    }

    if !DATE_PART.is_match(&timestamped_name) {
        return Err((StatusCode::NOT_FOUND, Html("not found")))
    }

    let data = data.lock().unwrap();
    let new_name = DATE_PART.replace(&timestamped_name, "").to_string();
    let real_path = PathBuf::from(&data.config.content_dir)
        .join("assets")
        .join(&new_name);

    log::info!(
        "Serving timestamped assset {} from file {}",
        &timestamped_name,
        &real_path.to_string_lossy()
    );

    // This is simplified version of what Warp's private function `file_reply` does. See:
    // https://github.com/seanmonstar/warp/blob/master/src/filters/fs.rs#L261
    let mut buf = Vec::new();
    if let Ok(last_modified) = read_file_bytes(&real_path, &mut buf) {
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
    } else {
        Err((StatusCode::NOT_FOUND, Html("not found")))
    }
}
