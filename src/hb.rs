use std::path::{Path, PathBuf};
use chrono::prelude::*;
use chrono::Duration;
use ordinal::Ordinal;
use handlebars::{Handlebars, handlebars_helper};
use crate::config::Config;
use crate::io::paths_with_ext_in_dir;

fn pluralize(word: &str, num: i64) -> (String, i64) {
    if num == 1 {
        (word.to_string(), num)
    } else {
        (word.to_string() + "s", num)
    }
}

handlebars_helper!(date_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    format!("{} {} {}",
        dt.format("%A"), // Day
        Ordinal(dt.day()), // Date
        dt.format("%B %Y") // Month, year, time
    )
});

handlebars_helper!(age_from_timestamp: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    let age = Utc::now().signed_duration_since(dt);
    let unit: String;
    let num: i64;
    if age.num_minutes() < 60 * 24 {
        if age.num_minutes() <= 90 {
            if age.num_minutes() == 0 {
                (unit, num) = ("Just now".to_string(), 0);
            } else {
                (unit, num) = pluralize("minute", age.num_minutes());
            }
        } else {
            (unit, num) = pluralize("hour", age.num_hours());
        }
    } else {
        match age.num_days() {
            1..=14 => (unit, num) = pluralize("day", age.num_days()),
            15..=31 => (unit, num) = pluralize("week", age.num_weeks()),
            // We'll pretend every month has 30 days, it's close enough
            32..=365 => (unit, num) = pluralize("month", age.num_days() / 30),
            _ => {
                let years = age.num_days() / 365;
                let remainder = age - Duration::days(years * 365);
                let months = remainder.num_days() / 30;
                let (yunit, _) = pluralize("year", years);
                unit = format!("{} {} {}", years, yunit, months);
                num = months;
            }
        }
    }
    if num > 0 {
        format!("{} {} ago", num, unit)
    } else {
        unit
    }
});

handlebars_helper!(is_current_tag: |this_tag: String, search_tag: String| {
    this_tag == search_tag
});

handlebars_helper!(return_text: |path: String| {
    let default_text = "Home".to_string();
    let path_parts: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .collect();
    if path == "/" {
        default_text
    } else {
        match path_parts[0] {
            "tag" => {
                let text = format!("Tag: <b>{}</b>", path_parts[1]);
                if path_parts.len() == 2 {
                    text
                } else {
                    format!("{} (page {})", text, path_parts[2])
                }
            },
            "index" => {
                if path == "/index/1" {
                    default_text
                } else {
                    format!("Articles (page {})", path_parts[1])
                }
            },
            _ => default_text
        }
    }
});

fn template_name(path: &Path) -> String {
    let stem = path.file_stem().unwrap();
    stem.to_string_lossy().to_string()
        .split('.')
        .map(|p: &str| p.to_string())
        .collect::<Vec<String>>()
        .get(0)
        .unwrap()
        .into()
}

pub fn create_handlebars(config: &Config) -> Handlebars<'static> {
    let mut hb = Handlebars::new();

    let dir = PathBuf::from(&config.content_dir).join("templates");
    if !dir.is_dir() {
        panic!("Template path {:?} is not a directory.", dir);
    }

    #[cfg(debug_assertions)]
    hb.set_dev_mode(true);

    paths_with_ext_in_dir("hbs", &dir, |path| {
        let template_name = template_name(path);
        log::info!("Registering template name: {}", &template_name);
        hb.register_template_file(&template_name, &path)
            .unwrap_or_else(|_| panic!(
                    "Failed to register template {} with path {:?}",
                    template_name,
                    path
            ))
    });

    // Not sure there's a way to automate this bit
    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));
    hb.register_helper("is_current_tag", Box::new(is_current_tag));
    hb.register_helper("age_from_timestamp", Box::new(age_from_timestamp));
    hb.register_helper("return_text", Box::new(return_text));

    hb
}
