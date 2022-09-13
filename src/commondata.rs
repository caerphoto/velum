use handlebars::Handlebars;
use config::Config;
use crate::hb::create_handlebars;
use crate::article::gather_fs_articles;
use crate::errors::ParseError;
use crate::article::view::ContentView;
use crate::comments::Comments;

const CONFIG_FILE: &str = "Settings"; // .toml is implied
const DEFAULT_PAGE_SIZE: usize = 5;

pub fn load_config() -> Config {
    Config::builder()
        .add_source(config::File::with_name(CONFIG_FILE))
        .build()
        .expect("Failed to build config")
}

pub struct CommonData {
    pub hbs: Handlebars<'static>,
    pub articles: Vec<ContentView>,
    pub comments: Comments,
    pub config: Config,
    pub session_id: Option<String>,
}

impl CommonData {
    pub fn new() -> Self {
        let config = load_config();
        let articles = gather_fs_articles(&config).expect("gather FS articles");
        let comments = Comments::new(&config);
        Self {
            hbs: create_handlebars(&config),
            articles,
            comments,
            config,
            session_id: None,
        }
    }

    fn rebuild(&mut self) -> Result<(), ParseError> {
        gather_fs_articles(&self.config)
            .map(|articles| {
                self.articles = articles;
            })
    }

    pub fn page_size(&self) -> usize {
        self.config
            .get_int("page_size")
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize
    }
}


