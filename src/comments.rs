// TODO: figure out how editing comments is going to work. maybe?

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Instant, Duration};
use std::net::SocketAddr;
use std::io::{self, BufRead, prelude::*};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::json;
use crate::config::Config;

const COMMENT_RATE_LIMIT: Duration = Duration::from_millis(2000);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Comment {
    pub text: String,
    pub author: String,
    pub author_url: String,
    pub timestamp: i64,
}

impl From<&CommentLine> for Comment {
    fn from(cline: &CommentLine) -> Self {
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

impl CommentLine {
    fn from_comment(c: &Comment, slug: &str) -> Self {
        Self {
            slug: slug.to_string(),
            text: c.text.clone(),
            author: c.author.clone(),
            author_url: c.author_url.clone(),
            timestamp: c.timestamp,
        }
    }
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}


#[derive(Clone, Debug)]
pub struct Comments {
    comments: HashMap<String, Vec<Comment>>,
    prev_instants: HashMap<String, Instant>,
    filename: PathBuf,
}

impl Comments {
    fn create_comments_file<P>(filename: P) -> bool
    where P: AsRef<Path> {
        File::create(filename).is_ok()
    }

    pub fn new(config: &Config) -> Self {
        let mut comments = HashMap::new();
        let prev_instants = HashMap::new();
        let filename = Path::new(&config.content_dir).join("comments.jsonl");
        let lines = read_lines(&filename);

        if let Ok(lines) = lines {
            for line in lines {
                if line.is_err() { continue; }
                let cl: Result<CommentLine, _> = serde_json::from_str(&line.unwrap());
                if cl.is_err() { continue; }
                let cl = cl.unwrap();
                let comment: Comment = Comment::from(&cl);
                let article_comments: Option<&mut Vec<Comment>> = comments.get_mut(&cl.slug);

                if let Some(article_comments) = article_comments {
                    article_comments.push(comment)
                } else {
                    comments.insert(cl.slug.to_string(), vec![comment]);
                }
            }
        } else if !Self::create_comments_file(&filename) {
            panic!("Failed to create comments file");
        }

        Self {
            comments,
            prev_instants,
            filename,
        }
    }

    fn save_comment(&self, slug: &str, comment: &Comment) {
        let cl = CommentLine::from_comment(comment, slug);
        let file = OpenOptions::new()
            .append(true)
            .open(&self.filename);

        if file.is_err() {
            log::error!("Failed to open comments file for appending");
        } else {
            let line = json!(cl);
            if let Err(e) = writeln!(file.unwrap(), "{}", &line) {
                log::error!("Failed to save comment:\n{}\nError: {}", &line, e);
            }
        }
    }

    fn is_limited(&self, key: &str) -> bool {

        let now = Instant::now();
        if let Some(prev_instant) = self.prev_instants.get(key) {
            if now.duration_since(*prev_instant) < COMMENT_RATE_LIMIT {
                return true;
            }
        }
        false
    }

    pub fn add(&mut self, slug: &str, comment: Comment, addr: Option<SocketAddr>) -> Result<Comment, String> {
        if addr.is_none() {
            log::error!("Attempt to comment with no supplied IP");
            return Err("No IP supplied".into());
        }

        let ip = addr.unwrap().ip();
        let instants_key = ip.to_string() + slug;
        let now = Instant::now();
        if self.is_limited(&instants_key) { return Err("IP is rate limited".into()) }

        if let Some(article_comments) = self.comments.get_mut(&slug.to_string()) {
            article_comments.push(comment.clone());
        } else {
            self.comments.insert(slug.to_string(), vec![comment.clone()]);
        }
        self.prev_instants.insert(instants_key, now);

        self.save_comment(slug, &comment);

        Ok(comment)
    }

    pub fn get(&self, slug: &str) -> Option<&Vec<Comment>> {
        self.comments.get(slug)
    }
}

