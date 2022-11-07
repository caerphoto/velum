use serde::Serialize;
use crate::{
    CommonData,
    comments::Comment,
    article::storage::LinkList,

};

#[derive(Serialize, Clone, Debug)]
pub struct IndexView {
    pub title: String,
    pub slug: String,
    pub preview: String,
    pub timestamp: i64,
    pub tags: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PrevNextView {
    pub title: String,
    pub slug: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ContentView {
    pub title: String,
    pub parsed_content: String,
    pub base_content: String,
    pub preview: String,
    pub slug: String,
    pub source_filename: std::path::PathBuf,
    pub timestamp: i64,
    pub tags: Vec<String>,
    pub prev: Option<PrevNextView>,
    pub next: Option<PrevNextView>,
}

impl ContentView {
    pub fn to_prev_next_view(&self) -> PrevNextView {
        PrevNextView { title: self.title.clone(), slug: self.slug.clone() }
    }

    pub fn to_index_view(&self) -> IndexView {
        IndexView {
            title: self.title.clone(),
            preview: self.preview.clone(),
            slug: self.slug.clone(),
            timestamp: self.timestamp,
            tags: self.tags.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct IndexRenderView<'a> {
    blog_title: &'a str,
    title: String,
    prev_page: usize,
    current_page: usize,
    next_page: usize,
    last_page: usize,
    search_tag: Option<&'a str>,
    article_count: usize,
    articles: Vec<IndexView>,
    body_class: &'a str,
    content_dir: &'a str,
    theme: String,
    home_page_info: Option<&'a str>,
}

impl<'a> IndexRenderView<'a> {
    pub fn new(
        article_list: LinkList,
        tag: Option<&'a str>,
        page: usize,
        page_size: usize,
        theme: String,
        data: &'a CommonData,
    ) -> Self {
        let last_page = div_ceil(article_list.total_articles, page_size);

        let title = if let Some(tag) = tag {
            String::from("Tag: ") + tag
        } else {
            String::from("Article Index")
        };

        // Page '0' is the home page: shows the same article list as the first index
        // page, but has the additional home page info box.
        let home_page_info = if page == 0 {
            Some(data.config.info_html.as_ref())
        } else {
            None
        };
        let page = std::cmp::max(page, 1);

        Self {
            blog_title: &data.config.blog_title,
            title,
            prev_page: if page > 1 { page - 1 } else { 0 },
            current_page: page,
            next_page: if page < last_page { page + 1 } else { 0 },
            last_page,
            body_class: if tag.is_some() { "tag-index" } else { "index" },
            search_tag: tag,
            article_count: article_list.total_articles,
            articles: article_list.index_views,
            content_dir: &data.config.content_dir,
            theme,
            home_page_info,
        }
    }
}

#[derive(Serialize)]
pub struct ArticleRenderView<'a> {
    title: &'a str,
    //blog_title: String,
    blog_title: &'a str,
    article: &'a ContentView,
    comments: Option<&'a Vec<Comment>>,
    return_path: String,
    body_class: &'a str,
    content_dir: &'a str,
    theme: String,
}

impl<'a> ArticleRenderView<'a> {
    pub fn new(
        article: &'a ContentView,
        return_path: String,
        theme: String,
        data: &'a CommonData,
    ) -> Self {
        Self {
            title: &article.title,
            blog_title: &data.config.blog_title,
            comments: data.comments.get(&article.slug),
            article,
            return_path,
            body_class: "article",
            content_dir: &data.config.content_dir,
            theme,
        }
    }
}

// Integer division rounding up, for calculating page count
fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 && rhs > 0 {
        d + 1
    } else {
        d
    }
}

