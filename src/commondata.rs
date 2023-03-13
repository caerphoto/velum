use std::collections::HashSet;
use std::path::PathBuf;
use handlebars::Handlebars;
use crate::hb::create_handlebars;
use crate::article::gather_fs_articles;
use crate::errors::ParseError;
use crate::article::builder::ParsedArticle;
use crate::comments::Comments;
use crate::config::Config;

pub struct CommonData {
    pub hbs: Handlebars<'static>,
    pub articles: Vec<ParsedArticle>,
    pub comments: Comments,
    pub config: Config,
    pub session_id: Option<String>,
    pub thumb_progress: HashSet<PathBuf>,
    pub initial_remaining_thumbs: usize,
}

impl CommonData {
    pub fn new() -> Self {
        let config = Config::load().expect("Failed to load config");
        let articles = gather_fs_articles(&config).expect("gather FS articles");
        let comments = Comments::load(&config);
        Self {
            hbs: create_handlebars(&config),
            articles,
            comments,
            config,
            session_id: None,
            thumb_progress: HashSet::new(),
            initial_remaining_thumbs: 0,
        }
    }

    pub fn rebuild(&mut self) -> Result<(), ParseError> {
        gather_fs_articles(&self.config)
            .map(|articles| {
                self.articles = articles;
            })
    }
}

impl Default for CommonData {
    fn default() -> Self {
        Self::new()
    }
}
