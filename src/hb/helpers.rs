use chrono::prelude::*;
use chrono::{DateTime, Duration};
use ordinal::Ordinal;
use std::path::PathBuf;
use std::fs;
use handlebars::{Handlebars, handlebars_helper};

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

handlebars_helper!(rfc822_date: |ts: i64| {
    let dt = Utc.timestamp_millis(ts);
    dt.to_rfc2822()
});

handlebars_helper!(is_current_tag: |this_tag: String, search_tag: String| {
    this_tag == search_tag
});

handlebars_helper!(render_tags: |tags: Vec<String>, search_tag: Option<String>| {
    let mut html = String::from("<ul class=\"tags\">");
    if let Some(search_tag) = search_tag {
        for tag in tags {
            html.push_str("<li>");
            if tag == search_tag {
                html.push_str("<span class=\"current-tag\">");
                html.push_str(&tag);
                html.push_str("</span>");
            } else {
                html.push_str("<a href=\"/tag/");
                html.push_str(&tag);
                html.push_str("\">");
                html.push_str(&tag);
                html.push_str("</a>");
            }
            html.push_str("</li>");
        }
        html.push_str("</ul>");
    } else {
        for tag in tags {
            html.push_str("<li><a href=\"/tag/");
            html.push_str(&tag);
            html.push_str("\">");
            html.push_str(&tag);
            html.push_str("</a></li>");
        }
        html.push_str("</ul>");
    }

    html
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

// Usage:
// asset_path "styles.css"
//   -> "/assets/styles-20220925145205.css"
// asset_path "admin/ui.min.js"
//   -> "/assets/admin/ui.min-20220925145205.js"
handlebars_helper!(asset_path: |filename: String| {
    lazy_static! {
        static ref CONTENT_DIR: String = {
            let c = crate::config::Config::load().expect("Failed to load config");
            c.content_dir
        };
    }
    let real_path = PathBuf::from(CONTENT_DIR.as_str())
        .join("assets")
        .join(&filename);
    let base_path = String::from("/assets/");

    let mut new_filename = filename.clone();

    if let Ok(metadata) = fs::metadata(real_path) {
        if let Ok(date) = metadata.modified() {
            let dt: DateTime<Utc> = date.into();
            let timestamp = dt.format("%Y%m%d%H%M%S");
            if let Some((pre, suf)) = filename.rsplit_once('.') {
                new_filename = format!("{}-{}.{}", pre, timestamp, suf);
            }
        }
    }
    base_path + &new_filename
});

pub fn register_helpers(mut hb: Handlebars) -> Handlebars {
    // Not sure there's a way to automate this bit
    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));
    hb.register_helper("age_from_timestamp", Box::new(age_from_timestamp));
    hb.register_helper("rfc822_date", Box::new(rfc822_date));
    hb.register_helper("is_current_tag", Box::new(is_current_tag));
    hb.register_helper("return_text", Box::new(return_text));
    hb.register_helper("asset_path", Box::new(asset_path));
    hb.register_helper("render_tags", Box::new(render_tags));

    hb
}
