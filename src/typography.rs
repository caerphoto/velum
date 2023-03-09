use regex::Regex;

struct Typograph {
    r: Regex,
    s: &'static str,
}

pub fn typogrified(text: &str) -> String {
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
