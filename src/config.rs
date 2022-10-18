use crate::io::paths_with_ext_in_dir;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::{fs, path::PathBuf};

const CONFIG_FILE: &str = "./Settings.toml";
const SECRETS_FILE: &str = "./Secrets.toml";
const BCRYPT_HASH_COST: u32 = 8;

#[derive(Serialize, Clone)]
pub struct Theme {
    filename: String,
    name: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub listen_ip: String,
    pub listen_port: u16,
    pub content_dir: String,
    pub page_size: usize,
    pub blog_title: String,
    pub blog_host: String,
    pub max_preview_length: usize,
    pub info_html: String,

    #[serde(skip)]
    pub secrets: Secrets,
    #[serde(skip)]
    pub theme_list: Vec<Theme>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Secrets {
    pub admin_password_hash: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, std::io::Error> {
        let s = fs::read_to_string(CONFIG_FILE)?;
        let mut config: Self = toml::from_str(&s)?;
        if config.content_dir.starts_with("./") {
            config.content_dir = config.content_dir.strip_prefix("./").unwrap().to_string();
        }
        if let Ok(s) = fs::read_to_string(SECRETS_FILE) {
            let secrets: Secrets = toml::from_str(&s)?;
            config.secrets = secrets;
        } else {
            config.prompt_for_password()
        }

        config.theme_list = Config::find_themes(&config.content_dir);

        Ok(config)
    }

    fn extract_theme_name(path: &PathBuf) -> std::io::Result<String> {
        let content = fs::read_to_string(path)?;
        let first_line = content
            .lines()
            .next()
            .map(|l| {
                l.trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim()
                    .to_string()
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Unable to extract theme name from file {:?}", path),
                )
            });

        first_line
    }

    fn find_themes(content_dir: &str) -> Vec<Theme> {
        let dir = PathBuf::from(content_dir).join("assets").join("themes");
        let mut themes = Vec::new();
        paths_with_ext_in_dir("css", &dir, |path| {
            let filename = path.file_name();
            if filename.is_none() {
                return;
            }
            let filename = String::from(filename.unwrap().to_string_lossy());
            let name = Config::extract_theme_name(&path.to_path_buf());
            if name.is_err() {
                return;
            }
            themes.push(Theme {
                name: name.unwrap(),
                filename,
            })
        });
        themes.sort_by(|t1, t2| t1.name.cmp(&t2.name));
        themes
    }

    pub fn save_secrets(&self) -> Result<(), std::io::Error> {
        let secrets = toml::to_string(&self.secrets).map_err(|e| {
            log::error!("Failed to serialize secrets: {:?}", e);
            let ek = std::io::ErrorKind::InvalidInput;
            std::io::Error::from(ek)
        })?;

        fs::write(SECRETS_FILE, secrets)
    }

    pub fn prompt_for_password(&mut self) {
        let pw = rpassword::prompt_password("Enter an admin password: ")
            .expect("Failed to fetch password from prompt.");
        if pw.is_empty() {
            println!("Password cannot be blank.");
            std::process::exit(1);
        }
        let pw_conf = rpassword::prompt_password("Confirm admin password: ")
            .expect("Faile to fetch password confirmation from prompt.");
        if pw != pw_conf {
            println!("Passwords do not match.");
            std::process::exit(1);
        }
        self.secrets.admin_password_hash =
            Some(bcrypt::hash(pw, BCRYPT_HASH_COST).expect("Failed to hash password"));

        if let Err(e) = self.save_secrets() {
            panic!("Config save failed: {:?}", e);
        }
    }
}
