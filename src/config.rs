use std::fs;
use serde::{Serialize, Deserialize};

const CONFIG_FILE: &str = "./Settings.toml";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub listen_ip: String,
    pub listen_port: u16,
    pub content_dir: String,
    pub page_size: usize,
    pub blog_title: String,
    pub max_preview_length: usize,
    pub admin_password_hash: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, std::io::Error> {
        let content = fs::read_to_string(CONFIG_FILE)?;
        let mut config: Self = toml::from_str(&content)?;
        if config.content_dir.starts_with("./") {
            config.content_dir = config.content_dir
                .strip_prefix("./")
                .unwrap()
                .to_string();
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let s = toml::to_string(&self)
            .map_err(|e| {
                log::error!("Failed to serialize config: {:?}", e);
                let ek = std::io::ErrorKind::InvalidInput;
                std::io::Error::from(ek)
            })?;

        fs::write(CONFIG_FILE, s)
    }
}
