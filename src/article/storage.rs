use crate::article::view::{ContentView, IndexView};
use crate::article::builder::{Builder, ParseResult, ParseError};
use r2d2_redis::{
    redis::{
        self,
        RedisResult,
        Commands,
    },
    r2d2,
    RedisConnectionManager
};
use std::ops::DerefMut;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

pub const DEFAULT_CONTENT_DIR: &str = "./content";
const DEFAULT_REDIS_HOST: &str = "redis://127.0.0.1/";

const BASE_KEY: &str = "velum:articles:";
const BASE_TIMESTAMPS_KEY: &str = "velum:timestamps:";
const BASE_TAGS_KEY: &str = "velum:tags:";
const BASE_TAGMAP_KEY: &str = "velum_tagmap:";

const TIMESTAMP_KEY: &str = "velum:slug_timestamps";

const ALL_ARTICLES_KEY: &str = "velum:articles:*";
const ALL_TIMESTAMPS_KEY: &str = "velum:timestamps:*";
const ALL_TAGS_KEY: &str = "velum:tags:*";

const LINK_FIELDS: [&str; 3] = ["title", "slug", "timestamp"];

pub type ConPool = r2d2::Pool<RedisConnectionManager>;
type Con = r2d2::PooledConnection<RedisConnectionManager>;

struct TsMap {
    timestamp: i64,
    key: String,
}

pub fn get_connection_pool(config: &config::Config) -> ConPool {
    let host = config.get_string("redist_host").unwrap_or(DEFAULT_REDIS_HOST.to_string());
    let mgr = RedisConnectionManager::new(host).expect("Failed to create Redis con mgr");
    r2d2::Pool::builder()
        .build(mgr)
        .expect("Failed to build pool")
}

fn get_all_timestamps(con: &mut Con) -> RedisResult<Vec<TsMap>> {
    let keys = timestamp_keys(con)?;
    let mut timestamps: Vec<TsMap> = Vec::with_capacity(keys.len());
    let ts_vals: Vec<i64> = redis::cmd("MGET")
        .arg(keys.clone()).query(con.deref_mut())?;

    for (index, ts) in ts_vals.iter().enumerate() {
        if let Some(key) = keys.get(index) {
            timestamps.push(TsMap {
                key: key.clone(),
                timestamp: *ts,
            });
        }
    }

    timestamps.sort_by_key(|ts| -ts.timestamp);
    Ok(timestamps)
}

fn article_keys(con: &mut Con) -> RedisResult<Vec<String>> {
    let keys: Vec<String> = con.keys(ALL_ARTICLES_KEY)?;
    Ok(keys)
}

fn timestamp_keys(con: &mut Con) -> RedisResult<Vec<String>> {
    let keys: Vec<String> = con.keys(ALL_TIMESTAMPS_KEY)?;
    Ok(keys)
}

fn tag_keys(con: &mut Con) -> RedisResult<Vec<String>> {
    let keys: Vec<String> = con.keys(ALL_TAGS_KEY)?;
    Ok(keys)
}

fn all_keys(con: &mut Con) -> RedisResult<Vec<String>> {
    let mut keys = article_keys(con)?;
    let mut ts_keys = timestamp_keys(con)?;
    let mut tag_keys = tag_keys(con)?;

    keys.append(&mut ts_keys);
    keys.append(&mut tag_keys);

    Ok(keys)
}
fn indices_from_page(page: usize, per_page: usize) -> (isize, isize) {
    // We'll assume these values fall within isize range, and just use unwrap
    let start_index: isize = (page.saturating_sub(1) * per_page).try_into().unwrap();
    let per_page: isize = per_page.try_into().unwrap();
    let end_index = start_index + per_page - 1;
    (start_index, end_index)
}

fn paginated_views_from_key(
    key: &str,
    page: usize,
    per_page: usize,
    con: &mut Con,
) -> RedisResult<(Vec<ArticleViewLink>, usize)> {
    let all_count = con.zcard(key)?;
    let (start_index, end_index) = indices_from_page(page, per_page);
    let slugs: Vec<String> = con.zrevrange(key, start_index, end_index)?;
    let mut articles: Vec<ArticleViewLink> = Vec::new();

    for slug in slugs {
        let key = String::from(BASE_KEY) + &slug;
        let result: (String, String, i64) = con.hget(key, &LINK_FIELDS)?;
        let tags = tags_for_slug(&result.1, con);
        articles.push(ArticleViewLink::from_redis_result(result, tags));
    }

    Ok((articles, all_count))
}

pub fn fetch_article_links(
    page: usize,
    per_page: usize,
    pool: &ConPool,
) -> RedisResult<(Vec<ArticleViewLink>, usize)> {
    let mut con = pool.get().unwrap();
    paginated_views_from_key(TIMESTAMP_KEY, page, per_page, &mut con)
}

pub fn fetch_by_tag(
    tag: &str,
    page: usize,
    per_page: usize,
    pool: &ConPool,
) -> RedisResult<(Vec<ArticleViewLink>, usize)> {
    let key = String::from(BASE_TAGMAP_KEY) + tag;
    let mut con = pool.get().unwrap();
    paginated_views_from_key(&key, page, per_page, &mut con)
}

fn surrounding_keys(
    timestamp: i64,
    con: &mut Con
) -> (Option<String>, Option<String>) {
    let timestamps = get_all_timestamps(con);
    if let Err(e) = timestamps {
        println!("{}", e);
        return (None, None);
    }
    let timestamps = timestamps.unwrap();
    if let Some(index) = timestamps.iter().position(|ts| ts.timestamp == timestamp) {
        let prev = if index > 0 {
            timestamps
                .get(index - 1)
                .map(|pts| pts.key.replace(BASE_TIMESTAMPS_KEY, BASE_KEY))
        } else {
            None
        };

        let next = if index < timestamps.len() - 1 {
            timestamps
                .get(index + 1)
                .map(|nts| nts.key.replace(BASE_TIMESTAMPS_KEY, BASE_KEY))
        } else {
            None
        };
        return (prev, next);
    }

    (None, None)
}

fn tags_for_slug(slug: &str, con: &mut Con) -> Vec<String> {
    let tags_key = String::from(BASE_TAGS_KEY) + slug;
    let result: RedisResult<Vec<String>> = con.smembers(tags_key);

    match result {
        Ok(mut tags) => {
            tags.sort();
            tags
        }
        Err(_) => Vec::new(), // don't really care that tag fetch failed
    }
}

pub fn fetch_from_slug<'a >(slug: &str, articles: &'a Vec<ContentView>) -> Option<&'a ContentView> {
    for a in articles {
        if a.slug == slug { return Some(&a) }
    }

    None
}

fn set_prev_next(articles: &mut Vec<ContentView>) {
    let iter = articles.iter();
    let len = articles.len();
    for (i, a) in iter.enumerate() {
        let prev = if i > 0 { iter.nth(i - 1) } else { None };
        let next = iter.nth(i + 1);
        a.prev = prev.map(|v| v.to_prev_next_view());
        a.next = next.map(|v| v.to_prev_next_view());
    }
}

fn builder_to_content_view(builder: Builder) -> ParseResult<ContentView> {
        let title = builder.title()?;
        Ok(ContentView {
            title,
            content: builder.parsed_content(),
            slug: builder.slug(title)?,
            timestamp: builder.timestamp,
            tags: builder.tags(),
            prev: None,
            next: None,
        })
}

pub fn gather_fs_articles(config: &config::Config) -> ParseResult<Vec<ContentView>> {
    let content_dir = config
        .get_string("content_dir")
        .unwrap_or(DEFAULT_CONTENT_DIR.to_owned());
    let path = PathBuf::from(content_dir).join("articles");
    if !path.is_dir() {
        let path = path.to_string_lossy();
        return Err(ParseError { cause: format!("article path `{}` is not a directory", &path) });
    }

    let mut articles: Vec<ContentView> = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() { continue }
        let builder = Builder::from_file(&path)?;
        let view = builder_to_content_view(builder)?;
        articles.push(view);
    }
    articles.sort_by_key(|k| k.timestamp);
    set_prev_next(&mut articles);
    Ok(articles)
}
