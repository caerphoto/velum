use crate::errors::{ParseError, ParseResult};
use pulldown_cmark::{self as cmark, Event, Tag};
use regex::Regex;
use std::io::{self, ErrorKind};
use std::path::{PathBuf, Path};
use std::{fs, time};

const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;

// See https://stackoverflow.com/questions/38461429/how-can-i-truncate-a-string-to-have-at-most-n-characters
// String::truncate can panic if the split is not on a char boundary
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

struct Typograph {
    r: Regex,
    s: &'static str,
}

// Struct for creating and managing article data
pub struct Builder {
    pub content: String,
    pub timestamp: i64,
    pub source_filename: PathBuf,
}

impl Builder {
    pub fn from_file(path: &Path) -> Result<Self, io::Error> {
        let metadata = fs::metadata(path)?;
        let content = fs::read_to_string(path)?;
        let filedate = metadata.modified()?;
        if let Ok(s) = filedate.duration_since(UNIX_EPOCH) {
            Ok(Self {
                content,
                timestamp: s.as_millis() as i64,
                source_filename: path.into(),
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

    /// Converts the given string to a URL-safe, lowercase version
    pub fn slug_from(text: &str) -> String {
        lazy_static! {
            static ref INVALID_CHARS: Regex = Regex::new(r"[^a-z0-9\-]").unwrap();
        }
        lazy_static! {
            static ref SEQUENTIAL_HYPEHNS: Regex = Regex::new(r"-+").unwrap();
        }

        let lowercase = text.to_lowercase();
        let simplified = INVALID_CHARS.replace_all(&lowercase, "-");
        let desequentialized = SEQUENTIAL_HYPEHNS.replace_all(&simplified, "-");
        String::from(desequentialized.trim_matches('-'))
    }

    pub fn slug(&self) -> ParseResult<String> {
        Ok(Builder::slug_from(&self.title()?))
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
            line.trim_matches('|')
                .split(',')
                .map(|t| Builder::slug_from(t.trim()))
                .collect()
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

    fn typogrify(text: &str) -> String {
        lazy_static! {
            static ref REPLACEMENTS: Vec<Typograph> = vec![
                Typograph { r: Regex::new("``").unwrap(), s: "“"},
                Typograph { r: Regex::new("''").unwrap(), s: "”" },

                // Decades, e.g. ’80s - may sometimes be wrong if it encounters a quote
                // that starts with a decade, e.g. '80s John Travolta was awesome.'
                Typograph { r: Regex::new(r"['‘](\d\d)s").unwrap(),  s: "’$1s" },

                // Order of these is imporant – opening quotes need to be done first.
                Typograph { r: Regex::new("`").unwrap(), s: "‘" },
                Typograph { r: Regex::new(r#"(^|\s|\()""#).unwrap(), s: "$1“" }, // ldquo
                Typograph { r: Regex::new(r#"""#).unwrap(),          s: "”" },   // rdquo

                Typograph { r: Regex::new(r"(^|\s|\()'").unwrap(),   s: "$1‘" }, // lsquo
                Typograph { r: Regex::new("'").unwrap(),             s: "’" },   // rsquo

                // Dashes
                // \u2009 = thin space
                // \u200a = hair space
                // \u2013 = en dash
                // \u2014 = em dash
                Typograph { r: Regex::new(r"\b–\b").unwrap(),   s: "\u{200a}\u{2013}\u{200a}" },
                Typograph { r: Regex::new(r"\b—\b").unwrap(),   s: "\u{200a}\u{2014}\u{200a}" },
                Typograph { r: Regex::new(" — ").unwrap(),      s: "\u{200a}\u{2014}\u{200a}" },
                Typograph { r: Regex::new("---").unwrap(),      s: "\u{200a}\u{2014}\u{200a}" },
                Typograph { r: Regex::new(" - | -- ").unwrap(), s: "\u{2009}\u{2013}\u{2009}" },
                Typograph { r: Regex::new("--").unwrap(),       s: "\u{200a}\u{2013}\u{200a}" },

                Typograph { r: Regex::new(r"\.\.\.").unwrap(), s: "…" } // hellip
            ];
        }

        let mut new_text = String::from(text);
        for typograph in REPLACEMENTS.iter() {
            new_text = typograph.r.replace_all(&new_text, typograph.s).into_owned();
        }

        new_text
    }

    pub fn content_preview(&self, max_len: usize) -> String {
        let content = self.main_content();
        let parser = cmark::Parser::new(&content);
        let mut parts: Vec<String> = Vec::new();
        for event in parser {
            if let Event::Text(text) = event {
                parts.push(text.to_string());
                if parts.len() >= max_len { break; }
            }
        }

        let truncated = Builder::typogrify(safe_truncate(&parts.join(" "), max_len));
        if truncated.len() < max_len { truncated } else { truncated + "…" }
    }

    pub fn parsed_content(&self) -> String {
        let content = self.main_content();
        let parser = cmark::Parser::new(&content);
        let mut in_code_block = false;
        let typographic_parser = parser.map(|event| {
            match event {
                Event::Start(tag) => {
                    if let Tag::CodeBlock(_) = tag {
                        in_code_block = true;
                    }
                    Event::Start(tag)

                },
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
                        Event::Text(Builder::typogrify(&text).into())
                    }
                },
                _ => event
            }
        });
        let mut parsed = String::new();
        cmark::html::push_html(&mut parsed, typographic_parser);
        parsed
    }
}

impl std::fmt::Display for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Filename: {}, slug: {}",
            self.source_filename.to_string_lossy(),
            self.slug().unwrap_or_else(|_| "<unknown>".into())
        )
    }
}
