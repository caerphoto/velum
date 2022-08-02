use crate::article::view::{ArticleView, ArticleViewLink};
use crate::article::ArticleBuilder;
use crate::BASE_PATH;
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

pub const REDIS_HOST: &str = "redis://127.0.0.1/";

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

pub fn get_connection_pool() -> ConPool {
    let mgr = RedisConnectionManager::new(REDIS_HOST).expect("Failed to create Redis con mgr");
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
fn gather_fs_articles() -> Result<Vec<ArticleBuilder>, io::Error> {
    let dir = PathBuf::from(BASE_PATH).join("articles");
    if !dir.is_dir() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Article path is not a directory",
        ));
    }

    let mut articles: Vec<ArticleBuilder> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            if let Ok(article) = ArticleBuilder::from_file(&path) {
                articles.push(article);
            }
        }
    }
    Ok(articles)
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

pub fn fetch_from_slug(
    slug: &str,
    pool: &ConPool
) -> RedisResult<ArticleView> {
    let mut con = pool.get().unwrap();
    let key = String::from(BASE_KEY) + slug;
    let tags = tags_for_slug(slug, &mut con);
    let timestamp: i64 = con.hget(&key, "timestamp")?;
    let (prev_key, next_key) = surrounding_keys(timestamp, &mut con);
    let prev_map: RedisResult<(String, String, i64)> = con.hget(prev_key, &LINK_FIELDS);
    let next_map: RedisResult<(String, String, i64)> = con.hget(next_key, &LINK_FIELDS);

    let prev: Option<ArticleViewLink> = match prev_map {
        Ok(m) => Some(ArticleViewLink::from_redis_result(m, Vec::new())),
        Err(_) => None,
    };
    let next: Option<ArticleViewLink> = match next_map {
        Ok(m) => Some(ArticleViewLink::from_redis_result(m, Vec::new())),
        Err(_) => None,
    };

    let article_map = con.hgetall(&key)?;
    let article = ArticleView::from_redis_result(&article_map, tags, prev, next);
    Ok(article)
}

fn destroy_keys(keys: Vec<String>, con: &mut Con) -> RedisResult<()> {
    for key in keys {
        con.del(key)?;
    }
    Ok(())
}

pub fn rebuild_data(pool: &ConPool) -> RedisResult<()> {
    let mut con = pool.get().unwrap();

    // Need to fetch keys before beginning transaction, as reads from within a
    // transaction will just return "QUEUED".
    let keys = all_keys(&mut con)?;
    let mut tag_map: HashMap<String, Vec<TsMap>> = HashMap::new();

    // Rebuild everything atomically within a transaction
    redis::cmd("MULTI").query(con.deref_mut())?;

    destroy_keys(keys, &mut con)?;

    // TODO: handle potential failure
    if let Ok(articles) = gather_fs_articles() {
        for article in articles {
            if let Ok(slug) = article.slug() {
                let key = String::from(BASE_KEY) + slug.as_str();
                con.hset_multiple(&key, &article.to_kv_list())?;

                let ts_key = String::from(BASE_TIMESTAMPS_KEY) + slug.as_str();
                con.set(ts_key, article.timestamp)?;

                let tag_key = String::from(BASE_TAGS_KEY) + slug.as_str();
                for tag in article.tags() {
                    con.sadd(&tag_key, tag.clone())?;
                    let tsmap = TsMap {
                        key: slug.clone(),
                        timestamp: article.timestamp,
                    };
                    if let Some(slugs) = tag_map.get_mut(&tag) {
                        slugs.push(tsmap);
                    } else {
                        tag_map.insert(tag, vec![tsmap]);
                    }
                }

                con.zadd(String::from(TIMESTAMP_KEY), slug, article.timestamp)?;
            }
        }
    }

    // Tag-to-slugs mapping
    for (tag, tsmaps) in tag_map.iter() {
        let key = String::from(BASE_TAGMAP_KEY) + tag;
        for tsmap in tsmaps {
            con.zadd(&key, &tsmap.key, tsmap.timestamp)?;
        }
    }

    redis::cmd("EXEC").query(con.deref_mut())?;

    Ok(())
}
