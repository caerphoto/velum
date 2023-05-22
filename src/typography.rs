use regex::{Regex, Captures, Replacer};

type ReplacerFn = fn(&Captures) -> String;

enum Rep {
    Str(&'static str),
    Fn(ReplacerFn),
}

impl Replacer for &Rep {
    fn replace_append(&mut self, caps: &Captures, dst: &mut String) {
        match self {
            Rep::Str(s) => caps.expand(s, dst),
            Rep::Fn(f) => dst.push_str(f(caps).as_ref()),
        }
    }
}


struct Typograph {
    rx: Regex,
    rep: Rep,
}

pub fn typogrified(text: &str) -> String {
    lazy_static! {
        static ref REPLACEMENTS: Vec<Typograph> = vec![
            Typograph { rx: Regex::new("``").unwrap(), rep: Rep::Str("“") },
            Typograph { rx: Regex::new("''").unwrap(), rep: Rep::Str("”") },

            // Decades, e.g. ’80s - may sometimes be wrong if it encounters a quote
            // that starts with a decade, e.g. '80s John Travolta was awesome.'
            Typograph { rx: Regex::new(r"['‘](\d\d)s").unwrap(),  rep: Rep::Str("’$1s")  },

            // Order of these is imporant – opening quotes need to be done first.
            Typograph { rx: Regex::new("`").unwrap(), rep: Rep::Str("‘")  },
            Typograph { rx: Regex::new(r#"(^|\s|\()""#).unwrap(), rep: Rep::Str("$1“")  }, // ldquo
            Typograph { rx: Regex::new(r#"""#).unwrap(),          rep: Rep::Str("”")  },   // rdquo

            Typograph { rx: Regex::new(r"(^|\s|\()'").unwrap(),   rep: Rep::Str("$1‘")  }, // lsquo
            Typograph { rx: Regex::new("'").unwrap(),             rep: Rep::Str("’")  },   // rsquo

            // Dashes
            // \u2009 = thin space
            // \u200a = hair space
            // \u2013 = en dash
            // \u2014 = em dash
            Typograph { rx: Regex::new(r"\b–\b").unwrap(),   rep: Rep::Str("\u{200a}\u{2013}\u{200a}")  },
            Typograph { rx: Regex::new(r"\b—\b").unwrap(),   rep: Rep::Str("\u{200a}\u{2014}\u{200a}")  },
            Typograph { rx: Regex::new(" — ").unwrap(),      rep: Rep::Str("\u{200a}\u{2014}\u{200a}")  },
            Typograph { rx: Regex::new("---").unwrap(),      rep: Rep::Str("\u{200a}\u{2014}\u{200a}")  },
            Typograph { rx: Regex::new(" - | -- ").unwrap(), rep: Rep::Str("\u{2009}\u{2013}\u{2009}")  },
            Typograph { rx: Regex::new("--").unwrap(),       rep: Rep::Str("\u{200a}\u{2013}\u{200a}")  },

            Typograph { rx: Regex::new(r"\.\.\.").unwrap(), rep: Rep::Str("…") }, // hellip

            Typograph {
                rx: Regex::new(r"\b(\d+)x").unwrap(),
                rep: Rep::Fn(|caps| String::from(caps.get(1).unwrap().as_str()) + "×"),
            },
        ];
    }

    let mut new_text = String::from(text);
    for typograph in REPLACEMENTS.iter() {
        new_text = typograph.rx.replace_all(&new_text, &typograph.rep).into_owned();
    }

    new_text
}
