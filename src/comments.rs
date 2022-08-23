// TODO: store comments in JSONL (JSON Lines) format, so appending is simple
// and quick.
// TODO: figure out how editing comments is going to work.

use std::fs::File;
use std::path::Path;
use std::io::{self, BufRead};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::article::storage::DEFAULT_CONTENT_DIR;

#[derive(Serialize, Clone)]
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

pub fn load_comments(config: &config::Config) -> HashMap<String, Vec<Comment>> {
    let base_path = config
        .get_string("content_path")
        .unwrap_or(DEFAULT_CONTENT_DIR.to_string());
    let filename = Path::new(&base_path).join("comments.jsonl");

    let mut comments_map = HashMap::new();

    let lines = read_lines(filename);
    if lines.is_err() {
        log::error!("Failed to read comments file");
        return comments_map;
    }

    for line in lines.unwrap() {
        if line.is_err() { continue; }
        let cl: Result<CommentLine, _> = serde_json::from_str(&line.unwrap());
        if cl.is_err() { continue; }
        let cl: CommentLine = cl.unwrap();
        let comments: Option<&mut Vec<Comment>> = comments_map.get_mut(&cl.slug);
        if let Some(comments) = comments {
            comments.push(Comment::from(cl))
        }
    }
    comments_map
}
