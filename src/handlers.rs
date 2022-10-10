pub mod index;
pub mod article;
pub mod asset;

use std::time::{
    SystemTime,
    UNIX_EPOCH,
    Duration,
};

use axum::{
    http::StatusCode,
    response::Html,
};

const INTERNAL_SERVER_ERROR: u16 = 500;
pub const BAD_REQUEST: u16 = 400;
const ONE_YEAR: Duration = Duration::new(31_536_000, 0);



fn create_timestamp() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        // i64 is enough milliseconds for 292 million years, so coercing it like
        // this is probably fine.
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64)
    }
}

pub fn server_error(msg: &str) -> (StatusCode, Html<String>) {
    log::error!("{}", msg);
    // Possible TODO: send HTML file
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html("Internal server error :(".into())
    )
}
