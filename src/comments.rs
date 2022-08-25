// TODO: store comments in JSONL (JSON Lines) format, so appending is simple
// and quick.
// TODO: figure out how editing comments is going to work.

use std::fs::File;
use std::path::Path;
use std::time::{Instant, Duration};
use std::net::SocketAddr;
use std::io::{self, BufRead};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::article::storage::DEFAULT_CONTENT_DIR;

const COMMENT_RATE_LIMIT: Duration = Duration::from_millis(2000);

#[derive(Serialize, Deserialize, Clone)]
pub struct Comment {
    text: String,
    author: String,
    author_url: String,
    timestamp: i64,
}

impl From<CommentLine> for Comment {
    fn from(cline: CommentLine) -> Self {
        Self {
            text: cline.text.clone(),
            author: cline.author.clone(),
            author_url: cline.author_url.clone(),
            timestamp: cline.timestamp,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommentLine {
    slug: String,
    text: String,
    author: String,
    author_url: String,
    timestamp: i64,
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

pub struct Comments {
    comments: HashMap<String, Vec<Comment>>,
    prev_instants: HashMap<String, Instant>
}

impl Comments {
    pub fn new(config: &config::Config) -> Self {
        let base_path = config
            .get_string("content_path")
            .unwrap_or(DEFAULT_CONTENT_DIR.to_string());
        let filename = Path::new(&base_path).join("comments.jsonl");

        let mut comments = HashMap::new();
        let prev_instants = HashMap::new();

        let lines = read_lines(filename);
        if lines.is_err() {
            panic!("Failed to read comments file");
        }

        for line in lines.unwrap() {
            if line.is_err() { continue; }
            let cl: Result<CommentLine, _> = serde_json::from_str(&line.unwrap());
            if cl.is_err() { continue; }
            let cl: CommentLine = cl.unwrap();
            let comments_list: Option<&mut Vec<Comment>> = comments.get_mut(&cl.slug);
            if let Some(comments_list) = comments_list {
                comments_list.push(Comment::from(cl))
            }
        }

        Self {
            comments,
            prev_instants,
        }
    }

    fn is_limited(&self, key: &str) -> bool {

        let now = Instant::now();
        if let Some(prev_instant) = self.prev_instants.get(key) {
            if now.duration_since(*prev_instant) < COMMENT_RATE_LIMIT {
                return false;
            }
        }
        true
    }

    pub fn add(&mut self, slug: &str, comment: Comment, addr: Option<SocketAddr>) -> Result<Comment, ()> {
        if addr.is_none() {
            log::error!("Attempt to comment with no supplied IP");
            return Err(());
        }

        let ip = addr.unwrap().ip();
        let key = ip.to_string() + slug;
        let now = Instant::now();
        if self.is_limited(&key) { return Err(()) }

        if let Some(article_comments) = self.comments.get_mut(&key) {
            article_comments.push(comment.clone());
        } else {
            self.comments.insert(key.clone(), vec![comment.clone()]);
        }
        self.prev_instants.insert(key, now);

        Ok(comment)
    }

    pub fn get(&self, slug: &str) -> Option<&Vec<Comment>> {
        self.comments.get(slug)
    }
}

