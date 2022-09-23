use std::fs;
use serde::{Serialize, Deserialize};

const CONFIG_FILE: &str = "./Settings.toml";
const SECRETS_FILE: &str = "./Secrets.toml";
const BCRYPT_HASH_COST: u32 = 8;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub listen_ip: String,
    pub listen_port: u16,
    pub content_dir: String,
    pub page_size: usize,
    pub blog_title: String,
    pub blog_host: String,
    pub max_preview_length: usize,

    #[serde(skip)]
    pub secrets: Secrets
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
            config.content_dir = config.content_dir
                .strip_prefix("./")
                .unwrap()
                .to_string();
        }
        if let Ok(s) = fs::read_to_string(SECRETS_FILE) {
            let secrets: Secrets = toml::from_str(&s)?;
            config.secrets = secrets;
        } else {
            config.prompt_for_password()
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let config = toml::to_string(&self)
            .map_err(|e| {
                log::error!("Failed to serialize config: {:?}", e);
                let ek = std::io::ErrorKind::InvalidInput;
                std::io::Error::from(ek)
            })?;

        let secrets = toml::to_string(&self.secrets)
            .map_err(|e| {
                log::error!("Failed to serialize secrets: {:?}", e);
                let ek = std::io::ErrorKind::InvalidInput;
                std::io::Error::from(ek)
            })?;

        fs::write(CONFIG_FILE, config)?;
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
        self.secrets.admin_password_hash = Some(
            bcrypt::hash(pw, BCRYPT_HASH_COST).expect("Failed to hash password")
        );

        if let Err(e) = self.save() {
            panic!("Config save failed: {:?}", e);
        }
    }
}
