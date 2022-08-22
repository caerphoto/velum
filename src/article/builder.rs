use regex::Regex;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::{time, fs};
use pulldown_cmark as cmark;
use crate::errors::{ParseError, ParseResult};

const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;


// Struct for creating and managing article data
pub struct Builder {
    pub content: String,
    pub timestamp: i64,
}

impl Builder {
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

    pub fn title(&self) -> ParseResult<String> {
        lazy_static! { static ref H1: Regex = Regex::new(r"^#\s*").unwrap(); }
        // Assumes first line of content text is formatted exactly as '# Article Title'
        self
            .content
            .lines()
            .next()
            .map(|l| String::from(H1.replace(l, "")))
            .ok_or(ParseError { cause: "unable to parse title".to_string() })
    }

    /// Converts the given string to a URL-safe, lowercase version
    fn slug_from(text: &str) -> String {
        lazy_static! { static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z0-9\-]").unwrap(); }
        lazy_static! { static ref SEQUENTIAL_HYPEHNS: Regex = Regex::new(r"-+").unwrap(); }

        let lowercase_text = text.to_lowercase();
        let simplified_text = INVALID_CHARS.replace_all(&lowercase_text, "-");
        String::from(SEQUENTIAL_HYPEHNS.replace_all(&simplified_text, "-"))
    }

    pub fn slug(&self) -> ParseResult<String> {
        Ok(Builder::slug_from(&self.title()?))
    }

    fn tags_line(&self) -> Option<String> {
        if let Some(line) = self.content.lines().nth(1) {
            if line.starts_with('|') && line.ends_with('|') {
                return Some(line.to_string())
            }
        }
        None
    }

    pub fn tags(&self) -> Vec<String> {
        if let Some(line) = self.tags_line() {
            line
                .trim_matches('|')
                .split(',')
                .map(|t| Builder::slug_from(&t.trim().to_string()))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn parsed_content(&self) -> String {
        let skip = match self.tags_line() {
            Some(_) => 2,
            None => 1,
        };
        let mut parsed = String::new();
        let without_title: String = self.content
            .lines()
            .skip(skip)
            .collect::<Vec<&str>>()
            .join("\n");
        let parser = cmark::Parser::new(&without_title);
        cmark::html::push_html(&mut parsed, parser);
        parsed
    }
}
