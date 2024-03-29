use crate::errors::{ParseError, ParseResult};
use crate::slug::Slug;
use crate::typography::typogrified;
use pulldown_cmark::{self as cmark, Event, Tag};
use regex::Regex;
use serde::Serialize;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::{fmt, fs, time};

const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;

// See https://stackoverflow.com/questions/38461429/how-can-i-truncate-a-string-to-have-at-most-n-characters
// String::truncate can panic if the split is not on a char boundary
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

// Struct for creating and managing article data
pub struct Builder {
    pub content: String,
    pub timestamp: i64,
    pub source_filename: PathBuf,
    pub max_preview_length: usize,
}

impl Builder {
    pub fn from_file(path: &Path, max_preview_length: usize) -> Result<Self, io::Error> {
        let metadata = fs::metadata(path)?;
        let content = fs::read_to_string(path)?;
        let filedate = metadata.modified()?;
        if let Ok(s) = filedate.duration_since(UNIX_EPOCH) {
            Ok(Self {
                content,
                timestamp: s.as_millis() as i64,
                source_filename: path.into(),
                max_preview_length,
            })
        } else {
            Err(io::Error::new(ErrorKind::Other, "failed to read file"))
        }
    }

    pub fn title(&self) -> ParseResult<String> {
        lazy_static! {
            static ref H1: Regex = Regex::new(r"^#\s*").unwrap();
        }
        // Assumes first line of content text is formatted exactly as '# Article Title'
        self.content
            .lines()
            .next()
            .map(|l| String::from(H1.replace(l, "")))
            .ok_or(ParseError {
                cause: "unable to parse title".to_string(),
            })
    }

    pub fn slug(&self) -> ParseResult<Slug> {
        Ok(Slug::new(&self.title()?))
    }

    fn tags_line(&self) -> Option<String> {
        if let Some(line) = self.content.lines().nth(1) {
            if line.starts_with('|') && line.ends_with('|') {
                return Some(line.to_string());
            }
        }
        None
    }

    pub fn tags(&self) -> Vec<String> {
        if let Some(line) = self.tags_line() {
            let mut tags: Vec<String> = line
                .trim_matches('|')
                .split(',')
                .map(|t| Slug::new(t.trim()).into())
                .collect();
            tags.sort();
            tags
        } else {
            Vec::new()
        }
    }

    fn main_content(&self) -> String {
        let skip = match self.tags_line() {
            Some(_) => 2,
            None => 1,
        };
        self.content
            .lines()
            .skip(skip)
            .collect::<Vec<&str>>()
            .join("\n")
    }

    pub fn content_preview(&self, max_len: usize) -> String {
        let content = self.main_content();
        let parser = cmark::Parser::new(&content);
        let mut parts: Vec<String> = Vec::new();
        for event in parser {
            if let Event::Text(text) = event {
                parts.push(text.to_string());
                if parts.len() >= max_len {
                    break;
                }
            }
        }

        let truncated = typogrified(safe_truncate(&parts.join(" "), max_len));
        if truncated.len() < max_len {
            truncated
        } else {
            truncated + "…"
        }
    }

    pub fn parsed_content(&self) -> String {
        let content = self.main_content();
        let parser = cmark::Parser::new(&content);
        let mut in_code_block = false;
        let typographic_parser = parser.map(|event| match event {
            Event::Start(tag) => {
                if let Tag::CodeBlock(_) = tag {
                    in_code_block = true;
                }
                Event::Start(tag)
            }
            Event::End(tag) => {
                if let Tag::CodeBlock(_) = tag {
                    if in_code_block {
                        in_code_block = false;
                    }
                }
                Event::End(tag)
            }
            Event::Text(text) => {
                if in_code_block {
                    Event::Text(text)
                } else {
                    Event::Text(typogrified(&text).into())
                }
            }
            _ => event,
        });
        let mut parsed = String::new();
        cmark::html::push_html(&mut parsed, typographic_parser);
        parsed
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ArticlePrevNext {
    pub title: String,
    pub slug: String,
}

impl From<&ParsedArticle> for ArticlePrevNext {
    fn from(value: &ParsedArticle) -> Self {
        Self {
            title: value.title.clone(),
            slug: value.slug.clone(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ParsedArticle {
    pub title: String,
    pub parsed_content: String,
    pub base_content: String,
    pub preview: String,
    pub slug: String,
    pub source_filename: std::path::PathBuf,
    pub timestamp: i64,
    pub tags: Vec<String>,
    pub comment_count: usize,
    pub prev: Option<ArticlePrevNext>,
    pub next: Option<ArticlePrevNext>,
}

impl TryFrom<&Builder> for ParsedArticle {
    type Error = ParseError;
    fn try_from(b: &Builder) -> Result<Self, Self::Error> {
        let title = b.title()?;
        Ok(ParsedArticle {
            slug: b.slug()?.into(), // borrow here before
            title,                  // move here
            parsed_content: b.parsed_content(),
            base_content: b.content.clone(),
            preview: b.content_preview(b.max_preview_length),
            source_filename: b.source_filename.clone(),
            timestamp: b.timestamp,
            tags: b.tags(),
            comment_count: 0,
            prev: None,
            next: None,
        })
    }
}

impl TryFrom<Builder> for ParsedArticle {
    type Error = ParseError;
    fn try_from(value: Builder) -> Result<Self, Self::Error> {
        ParsedArticle::try_from(&value)
    }
}

impl fmt::Display for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Filename: {}, slug: {}",
            self.source_filename.to_string_lossy(),
            self.slug().unwrap_or_else(|_| "<unknown>".into())
        )
    }
}
