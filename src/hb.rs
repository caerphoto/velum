use std::path::{Path, PathBuf};
use chrono::prelude::*;
use chrono::Duration;
use ordinal::Ordinal;
use handlebars::{Handlebars, handlebars_helper};
use crate::config::Config;

fn tmpl_path(tmpl_name: &str, config: &Config) -> PathBuf {
    let filename = [tmpl_name, "html.hbs"].join(".");
    let path = Path::new(&config.content_dir).join("templates");
    path.join(filename)
}

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

pub fn create_handlebars(config: &Config) -> Handlebars<'static> {
    let mut hb = Handlebars::new();
    //TODO: put this stuff in config, and loop over it here.
    let index_tmpl_path = tmpl_path("index", config);
    let article_tmpl_path = tmpl_path("article", config);
    let login_tmpl_path = tmpl_path("login", config);
    let admin_tmpl_path = tmpl_path("admin", config);
    let tag_list_tmpl_path = tmpl_path("_tag_list", config);
    let comments_tmpl_path = tmpl_path("_comments", config);
    let comment_tmpl_path = tmpl_path("_comment", config);
    let header_tmpl_path = tmpl_path("_header", config);
    let footer_tmpl_path = tmpl_path("_footer", config);

    #[cfg(debug_assertions)]
    hb.set_dev_mode(true);

    hb.register_template_file("main", &index_tmpl_path)
        .expect("Failed to register index template file");
    hb.register_template_file("article", &article_tmpl_path)
        .expect("Failed to register article template file");
    hb.register_template_file("login", &login_tmpl_path)
        .expect("Failed to register login template file");
    hb.register_template_file("admin", &admin_tmpl_path)
        .expect("Failed to register admin template file");
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
