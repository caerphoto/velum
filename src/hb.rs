pub mod helpers;

use std::path::{Path, PathBuf};
use handlebars::Handlebars;
use crate::config::Config;
use crate::io::paths_with_ext_in_dir;
use helpers::register_helpers;


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
        panic!("Template path {dir:?} is not a directory.");
    }

    #[cfg(debug_assertions)]
    hb.set_dev_mode(true);

    paths_with_ext_in_dir("hbs", &dir, |path| {
        let template_name = template_name(path);
        hb.register_template_file(&template_name, path)
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to register template {template_name} with path {path:?}. Error: {e}"
                )
            })
    });

    register_helpers(hb)
}
