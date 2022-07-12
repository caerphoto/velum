pub mod storage;
pub mod view;

use regex::Regex;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::{time, fs};
use pulldown_cmark as cmark;

const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;
const DEFAULT_TITLE: &str = "<no title>";

// Struct for creating and managing article data
pub struct ArticleBuilder {
    pub content: String,
    pub timestamp: i64,
}

impl ArticleBuilder {
    pub fn from_file(path: &PathBuf) -> Result<Self, io::Error> {
        let metadata = fs::metadata(path)?;
        let content = fs::read_to_string(path)?;
        let created = metadata.created()?;
        if let Ok(s) = created.duration_since(UNIX_EPOCH) {
            Ok(Self {
                content,
                timestamp: s.as_millis() as i64
            })
        } else {
            Err(io::Error::new(ErrorKind::Other, "failed to read file"))
        }
    }

    fn title(&self) -> Option<String> {
        lazy_static! { static ref H1: Regex = Regex::new(r"^#\s*").unwrap(); }
        // Assumes first line of content text is formatted exactly as '# Article Title'
        self.content.lines().next().map(|l|
            String::from(H1.replace(l, ""))
        )
    }


    fn slug(&self) -> Result<String, &'static str> {
        lazy_static! { static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z0-9\-]").unwrap(); }
        lazy_static! { static ref SEQUENTIAL_HYPEHNS: Regex = Regex::new(r"-+").unwrap(); }
        if let Some(t) = self.title() {
            let lowercase_title = t.to_lowercase();
            let simplified_key = INVALID_CHARS.replace_all(&lowercase_title, "-");
            Ok(String::from(
                SEQUENTIAL_HYPEHNS.replace_all(&simplified_key, "-")
            ))
        } else {
            Err("Unable to create key because artitcle has no title.")
        }
    }

    fn tags_line(&self) -> Option<String> {
        if let Some(line) = self.content.lines().nth(1) {
            if line.starts_with('|') && line.ends_with('|') {
                return Some(line.to_string())
            }
        }
        None
    }

    fn tags(&self) -> Vec<String> {
        if let Some(line) = self.tags_line() {
            line
                .trim_matches('|')
                .split(',')
                .map(|t| t.trim().to_string())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn parsed_content(&self) -> String {
        let skip = match self.tags_line() {
            Some(_) => 2,
            None => 1,
        };
        let mut parsed = String::new();
        let no_title: String = self.content
            .lines()
            .skip(skip)
            .collect::<Vec<&str>>()
            .join("\n");
        let parser = cmark::Parser::new(&no_title);
        cmark::html::push_html(&mut parsed, parser);
        parsed
    }


    // For passing to Redis via hset_multiple
    fn to_kv_list(&self) -> Box<[(String, String)]> {
        Box::new([
            ("title".to_string(), self.title().unwrap_or_else(|| DEFAULT_TITLE.to_string())),
            ("content".to_string(), self.parsed_content()),
            ("slug".to_string(), self.slug().unwrap_or_else(|_| "-".to_string())),
            ("timestamp".to_string(), self.timestamp.to_string()),
        ])
    }
}
