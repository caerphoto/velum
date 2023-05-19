use regex::{Regex, Captures};

type ReplacerFn = fn(&Captures) -> String;

struct Typograph {
    r: Regex,
    s: Option<&'static str>,
    f: Option<ReplacerFn>,
}

pub fn typogrified(text: &str) -> String {
    lazy_static! {
        static ref REPLACEMENTS: Vec<Typograph> = vec![
            Typograph { r: Regex::new("``").unwrap(), s: Some("“"), f: None },
            Typograph { r: Regex::new("''").unwrap(), s: Some("”"), f: None  },

            // Decades, e.g. ’80s - may sometimes be wrong if it encounters a quote
            // that starts with a decade, e.g. '80s John Travolta was awesome.'
            Typograph { r: Regex::new(r"['‘](\d\d)s").unwrap(),  s: Some("’$1s"), f: None  },

            // Order of these is imporant – opening quotes need to be done first.
            Typograph { r: Regex::new("`").unwrap(), s: Some("‘"), f: None  },
            Typograph { r: Regex::new(r#"(^|\s|\()""#).unwrap(), s: Some("$1“"), f: None  }, // ldquo
            Typograph { r: Regex::new(r#"""#).unwrap(),          s: Some("”"), f: None  },   // rdquo

            Typograph { r: Regex::new(r"(^|\s|\()'").unwrap(),   s: Some("$1‘"), f: None  }, // lsquo
            Typograph { r: Regex::new("'").unwrap(),             s: Some("’"), f: None  },   // rsquo

            // Dashes
            // \u2009 = thin space
            // \u200a = hair space
            // \u2013 = en dash
            // \u2014 = em dash
            Typograph { r: Regex::new(r"\b–\b").unwrap(),   s: Some("\u{200a}\u{2013}\u{200a}"), f: None  },
            Typograph { r: Regex::new(r"\b—\b").unwrap(),   s: Some("\u{200a}\u{2014}\u{200a}"), f: None  },
            Typograph { r: Regex::new(" — ").unwrap(),      s: Some("\u{200a}\u{2014}\u{200a}"), f: None  },
            Typograph { r: Regex::new("---").unwrap(),      s: Some("\u{200a}\u{2014}\u{200a}"), f: None  },
            Typograph { r: Regex::new(" - | -- ").unwrap(), s: Some("\u{2009}\u{2013}\u{2009}"), f: None  },
            Typograph { r: Regex::new("--").unwrap(),       s: Some("\u{200a}\u{2013}\u{200a}"), f: None  },

            Typograph { r: Regex::new(r"\.\.\.").unwrap(), s: Some("…"), f: None }, // hellip

            Typograph { r: Regex::new(r"\b(\d+)x").unwrap(), s: None, f: Some(|caps| String::from(caps.get(1).unwrap().as_str()) + "×") }
        ];
    }

    let mut new_text = String::from(text);
    for typograph in REPLACEMENTS.iter() {
        if typograph.f.is_some() {
            new_text = typograph.r.replace_all(&new_text, typograph.f.unwrap()).into_owned();
        } else {
            new_text = typograph.r.replace_all(&new_text, typograph.s.unwrap()).into_owned();
        }
    }

    new_text
}
