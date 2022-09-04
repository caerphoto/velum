use std::path::{Path, PathBuf};
use crate::article::storage::DEFAULT_CONTENT_DIR;
use chrono::prelude::*;
use chrono::Duration;
use ordinal::Ordinal;
use handlebars::{Handlebars, handlebars_helper};

fn tmpl_path(tmpl_name: &str, config: &config::Config) -> PathBuf {
    let base_path = config
        .get_string("content_path")
        .unwrap_or_else(|_| DEFAULT_CONTENT_DIR.to_string());
    let filename = [tmpl_name, "html.hbs"].join(".");
    let path = Path::new(&base_path).join("templates");
    path.join(filename)
}

fn pluralize(word: &str, num: i64) -> String {
    if num == 1 {
        word.to_string()
    } else {
        word.to_string() + "s"
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
    let num: Option<i64>;
    if age.num_minutes() < 60 * 24 {
        if age.num_minutes() <= 90 {
            if age.num_minutes() == 0 {
                num = None;
                unit = "Just now".to_string();
            } else {
                num = Some(age.num_minutes());
                unit = pluralize("minute", num.unwrap());
            }
        } else {
            num = Some(age.num_hours());
            unit = pluralize("hour", num.unwrap());
        }
    } else {
        match age.num_days() {
            1..=14 => {
                num = Some(age.num_days());
                unit = pluralize("day", num.unwrap());
            },
            15..=31 => {
                num = Some(age.num_weeks());
                unit = pluralize("week", num.unwrap());
            },
            32..=365 => {
                // We'll pretend every month has 30 days, it's close enough
                num = Some(age.num_days() / 30);
                unit = pluralize("month", num.unwrap());
            },
            _ => {
                let years = age.num_days() / 365;
                let remainder = age - Duration::days(years * 365);
                let months = remainder.num_days() / 30;
                let yunit = pluralize("year", years);
                unit = format!("{} {} {}", years, yunit, months);
                num = Some(months);
            }
        }
    }
    if let Some(n) = num {
        format!("{} {} ago", n, unit)
    } else {
        unit
    }
});

handlebars_helper!(is_current_tag: |this_tag: String, search_tag: String| {
    this_tag == search_tag
});

pub fn create_handlebars(config: &config::Config) -> Handlebars<'static> {
    let mut hb = Handlebars::new();
    let index_tmpl_path = tmpl_path("index", config);
    let article_tmpl_path = tmpl_path("article", config);
    let tag_list_tmpl_path = tmpl_path("_tag_list", config);
    let comments_tmpl_path = tmpl_path("_comments", config);
    let comment_tmpl_path = tmpl_path("_comment", config);
    let header_tmpl_path = tmpl_path("_header", config);
    let footer_tmpl_path = tmpl_path("_footer", config);

    hb.set_dev_mode(true);

    hb.register_template_file("main", &index_tmpl_path)
        .expect("Failed to register index template file");
    hb.register_template_file("article", &article_tmpl_path)
        .expect("Failed to register article template file");
    hb.register_template_file("tag_list", &tag_list_tmpl_path)
        .expect("Failed to register tag_list template file");
    hb.register_template_file("header", &header_tmpl_path)
        .expect("Failed to register header template file");
    hb.register_template_file("comments", &comments_tmpl_path)
        .expect("Failed to register comments template file");
    hb.register_template_file("comment", &comment_tmpl_path)
        .expect("Failed to register comment template file");
    hb.register_template_file("footer", &footer_tmpl_path)
        .expect("Failed to register footer template file");

    hb.register_helper("date_from_timestamp", Box::new(date_from_timestamp));
    hb.register_helper("is_current_tag", Box::new(is_current_tag));
    hb.register_helper("age_from_timestamp", Box::new(age_from_timestamp));

    hb
}
