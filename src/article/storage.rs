use std::io::{self, ErrorKind};
use std::fs;
use std::path::PathBuf;
use redis::{self, RedisResult};
use redis::Commands;
use crate::article::Article;
use crate::article::view::{ArticleView, ArticleViewLink};
use crate::BASE_PATH;

const REDIS_HOST: &str = "redis://127.0.0.1/";
const BASE_KEY: &str = "velum:articles:";
const BASE_TS_KEY: &str = "velum:timestamps:";
const ALL_ARTICLES_KEY: &str = "velum:articles:*";
const ALL_TIMESTAMPS_KEY: &str = "velum:timestamps:*";
const LINK_FIELDS: [&str; 3] = ["title", "route", "timestamp"];


struct TsMap {
    timestamp: i64,
    key: String,
}

fn get_all_timestamps(con: &mut redis::Connection) -> Result<Vec<TsMap>, redis::RedisError> {
    let keys = timestamp_keys(con)?;
    let mut timestamps: Vec<TsMap> = Vec::with_capacity(keys.len());
    let ts_vals: Vec<i64> = redis::cmd("MGET").arg(&keys).query(con)?;

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

fn article_keys(con: &mut redis::Connection) -> Result<Vec<String>, redis::RedisError> {
    let keys: Vec<String> = con.keys(ALL_ARTICLES_KEY)?;
    Ok(keys)
}

fn timestamp_keys(con: &mut redis::Connection) -> Result<Vec<String>, redis::RedisError> {
    let keys: Vec<String> = con.keys(ALL_TIMESTAMPS_KEY)?;
    Ok(keys)
}

fn gather_fs_articles() -> Result<Vec<Article>, io::Error> {
    let dir = PathBuf::from(BASE_PATH).join("articles");
    if !dir.is_dir() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Article path is not a directory"));
    }

    let mut articles: Vec<Article> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            if let Ok(article) = Article::from_file(&path) {
                articles.push(article);
            }
        }
    }
    Ok(articles)
}

pub fn gather_article_links() -> Result<Vec<ArticleViewLink>, redis::RedisError> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;
    let mut articles: Vec<ArticleViewLink> = Vec::new();
    let keys = article_keys(&mut con)?;
    for key in keys {
        // Reading only the fields we need into a tuple is quicker than reading
        // all of the fields via hgetall()
        let result: (String, String, i64) = con.hget(key, &LINK_FIELDS)?;
        articles.push(ArticleViewLink::from_redis_result(result))
    }

    articles.sort_by_key(|a| -a.timestamp);

    Ok(articles)
}

fn surrounding_keys(timestamp: i64, con: &mut redis::Connection) -> (Option<String>, Option<String>) {
    let timestamps = get_all_timestamps(con);
    if let Err(e) = timestamps {
        println!("{}", e);
        return (None, None)

    }
    let timestamps = timestamps.unwrap();
    if let Some(index) = timestamps.iter().position(
        |ts| ts.timestamp == timestamp

    ) {
        let prev = if index > 0 {
            match timestamps.get(index - 1) {
                Some(pts) => Some(pts.key.replace(BASE_TS_KEY, BASE_KEY)),
                None => None
            }
        } else { None };

        let next = if index < timestamps.len() - 1 {
            match timestamps.get(index + 1) {
                Some(nts) => Some(nts.key.replace(BASE_TS_KEY, BASE_KEY)),
                None => None
            }
        } else { None };
        return (prev, next);
    }

    (None, None)
}

pub fn fetch_from_slug(slug: &str) -> RedisResult<ArticleView> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;
    let key = String::from(BASE_KEY) + slug;
    let timestamp: i64 = con.hget(&key, "timestamp")?;
    let (prev_key, next_key) = surrounding_keys(timestamp, &mut con);
    let prev_map: RedisResult<(String, String, i64)> = con.hget(prev_key, &LINK_FIELDS);
    let next_map: RedisResult<(String, String, i64)> = con.hget(next_key, &LINK_FIELDS);

    let prev: Option<ArticleViewLink> = match prev_map {
        Ok(m) => Some(ArticleViewLink::from_redis_result(m)),
        Err(_) => None
    };
    let next: Option<ArticleViewLink> = match next_map {
        Ok(m) => Some(ArticleViewLink::from_redis_result(m)),
        Err(_) => None
    };

    let article_map = con.hgetall(&key)?;
    let article = ArticleView::from_redis_result(&article_map, prev, next);
    Ok(article)
}


fn destroy_all_keys(con: &mut redis::Connection) -> redis::RedisResult<()> {
    let mut keys: Vec<String> = con.keys(String::from(BASE_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }
    keys = con.keys(String::from(BASE_TS_KEY) + "*")?;
    for key in keys {
        con.del(key)?;
    }

    Ok(())
}

pub fn rebuild_redis_data() -> redis::RedisResult<()> {
    let client = redis::Client::open(REDIS_HOST)?;
    let mut con = client.get_connection()?;

    destroy_all_keys(&mut con)?;

    // TODO: handle potential failure
    if let Ok(articles) = gather_fs_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                con.hset_multiple(&key, &article.to_kv_list())?;
                let ts_key = String::from(BASE_TS_KEY) + slug.as_str();
                con.set(ts_key, article.timestamp)?;
            }
        }
    }
    Ok(())
}

