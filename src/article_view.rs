use std::collections::BTreeMap;
use serde::Serialize;
use pulldown_cmark as cmark;
use redis;
use redis::Commands;

pub const BASE_KEY: &str = "velum:articles:";
pub const BASE_TS_KEY: &str = "velum:timestamps:";
const ALL_ARTICLES_KEY: &str = "velum:articles:*";
const ALL_TIMESTAMPS_KEY: &str = "velum:timestamps:*";

pub fn article_keys(con: &mut redis::Connection) -> Result<Vec<String>, redis::RedisError> {
    let keys: Vec<String> = con.keys(ALL_ARTICLES_KEY)?;
    Ok(keys)
}

fn timestamp_keys(con: &mut redis::Connection) -> Result<Vec<String>, redis::RedisError> {
    let keys: Vec<String> = con.keys(ALL_TIMESTAMPS_KEY)?;
    Ok(keys)
}

struct TsMap {
    timestamp: i64,
    key: String,
}

fn get_all_timestamps(con: &mut redis::Connection) -> Result<Vec<TsMap>, redis::RedisError> {
    let keys = timestamp_keys(con)?;
    let mut timestamps: Vec<TsMap> = Vec::with_capacity(keys.len());
    let ts_vals: Result<Vec<String>, redis::RedisError> =
        redis::cmd("MGET").arg(&keys).query(con);

    match ts_vals {
        Ok(vals) => {
            for (index, ts) in vals.iter().enumerate() {
                if let Some(key) = keys.get(index) {
                    timestamps.push(TsMap {
                        key: key.clone(),
                        timestamp: ts.parse::<i64>().unwrap_or(0),
                    });
                }
            }
        },
        Err(e) => return Err(e)
    }

    timestamps.sort_by_key(|ts| -ts.timestamp);
    Ok(timestamps)
}


#[derive(Serialize, Clone, Debug)]
pub struct ArticleView {
    pub title: String,
    pub content: String,
    pub route: String,
    pub timestamp: i64,
}

impl ArticleView {
    fn parse_content(content: &str) -> String {
        let mut parsed_article = String::new();
        let no_title: String = content
            .lines()
            .skip(1)
            .collect::<Vec<&str>>()
            .join("\n");
        let parser = cmark::Parser::new(&no_title);
        cmark::html::push_html(&mut parsed_article, parser);
        parsed_article
    }


    pub fn from_redis(a: &BTreeMap<String, String>, parse_content: bool) -> Self {
        let timestamp = a.get("timestamp").unwrap();
        let content = if parse_content {
            ArticleView::parse_content(a.get("content").unwrap())
        } else {
            String::from("")
        };

        Self {
            title: a.get("title").unwrap().to_string(),
            content,
            route: a.get("route").unwrap().to_string(),
            timestamp: timestamp.parse::<i64>().unwrap_or(0),
        }
    }

    pub fn from_redis_key(
        key: &str,
        con: &mut redis::Connection,
        parse_content: bool
    ) -> Option<Self> {
        match con.hgetall(key) {
            Ok(result) => Some(Self::from_redis(&result, parse_content)),
            Err(_) => None
        }
    }

    fn surrounding_keys(&self, con: &mut redis::Connection) -> (Option<String>, Option<String>) {

        let timestamps = get_all_timestamps(con);
        if let Err(e) = timestamps {
            println!("{}", e);
            return (None, None)

        }
        let timestamps = timestamps.unwrap();
        if let Some(index) = timestamps.iter().position(
            |ts| ts.timestamp == self.timestamp

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

    pub fn surrounding(
        &self,
        con: &mut redis::Connection
    ) -> (Option<Self>, Option<Self>) {
        let (prev_key, next_key) = self.surrounding_keys(con);
        (
            match prev_key {
                Some(key) => ArticleView::from_redis_key(&key, con, false),
                None => None
            },
            match next_key {
                Some(key) => ArticleView::from_redis_key(&key, con, false),
                None => None
            }
        )
    }
}
