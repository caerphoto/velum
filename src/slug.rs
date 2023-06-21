use regex::Regex;
use std::fmt;
use unicode_normalization::UnicodeNormalization;

pub struct Slug {
    slug: String,
}

impl Slug {
    /// Converts the given string to a URL-safe, lowercase version
    pub fn new(s: &str) -> Self {
        lazy_static! {
            static ref SEQUENTIAL_HYPEHNS: Regex = Regex::new(r"-+").unwrap();
        }

        // Extract ASCII characters, with diacritics removed
        let simplified = s
            .nfd() // normalised form, decomposed
            .filter_map(|c| {
                if c.is_ascii_alphanumeric() {
                    Some(c.to_ascii_lowercase())
                } else if c.is_whitespace() || c == '-' {
                    Some('-')
                } else {
                    None
                }
            })
            .collect::<String>();
        let desequentialized = SEQUENTIAL_HYPEHNS.replace_all(&simplified, "-");
        Self {
            slug: String::from(desequentialized.trim_matches('-')),
        }
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.slug)
    }
}
impl From<&str> for Slug {
    fn from(item: &str) -> Self {
        Self::new(item)
    }
}
impl From<String> for Slug {
    fn from(item: String) -> Self {
        Self::new(&item)
    }
}
impl std::ops::Add<&str> for Slug {
    type Output = String;
    fn add(self, other: &str) -> Self::Output {
        self.slug + other
    }
}

impl From<Slug> for String {
    fn from(item: Slug) -> Self {
        item.slug
    }
}
