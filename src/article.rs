use regex::Regex;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::{time, fs};

const UNIX_EPOCH: time::SystemTime = time::SystemTime::UNIX_EPOCH;
const DEFAULT_TITLE: &str = "<no title>";

// Struct for creating and managing article data
pub struct Article {
    pub content: String,
    pub timestamp: i64,
}

impl Article {
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

    pub fn title(&self) -> Option<String> {
        lazy_static! { static ref H1: Regex = Regex::new(r"^#\s*").unwrap(); }
        // Assumes first line of content text is formatted exactly as '# Article Title'
        if let Some(l) = self.content.lines().nth(0) {
            Some(String::from(
                H1.replace(l, "")
            ))
        } else {
            None
        }
    }

    pub fn slug(&self) -> Result<String, &'static str> {
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

    fn route(&self) -> Result<String, &'static str> {
        if let Ok(slug) = self.slug() {
            Ok(String::from("/articles/") + &slug)
        } else {
            Err("Unable to create route due to error in slug")
        }
    }

    // For passing to Redis via hset_multiple
    pub fn to_kv_list(&self) -> Box<[(String, String)]> {
        Box::new([
            ("title".to_string(), self.title().unwrap_or(DEFAULT_TITLE.to_string())),
            ("content".to_string(), self.content.clone()),
            ("route".to_string(), self.route().unwrap_or("/".to_string())),
            ("timestamp".to_string(), self.timestamp.to_string()),
        ])
    }
}
