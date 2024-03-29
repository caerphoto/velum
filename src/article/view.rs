use crate::{
    article::storage::{fetch_by_slug, PaginatedArticles},
    comments::{Comment, Comments},
    CommonData,
};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use super::builder::ParsedArticle;

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
    articles: Vec<&'a ParsedArticle>,
    comment_counts: Vec<usize>,
    body_class: &'a str,
    content_dir: &'a str,
    theme: String,
    home_page_info: Option<&'a str>,
}

impl<'a> IndexRenderView<'a> {
    pub fn new(
        article_list: &'a PaginatedArticles,
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
            articles: article_list.articles.clone(), // is a vec of refs, so clone is cheap
            comment_counts: Self::get_comment_counts(&article_list.articles, &data.comments),
            content_dir: &data.config.content_dir,
            theme,
            home_page_info,
        }
    }

    fn get_comment_counts(articles: &[&ParsedArticle], comments: &Comments) -> Vec<usize> {
        articles
            .iter()
            .map(|a| comments.count_for(&a.slug))
            .collect()
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

#[derive(Serialize)]
pub struct RssArticleView<'a> {
    title: &'a str,
    slug: &'a str,
    content: String,
    timestamp: i64,
}

impl<'a> RssArticleView<'a> {
    pub fn from_parsed_article<'b>(
        article: &'b ParsedArticle,
        blog_url: &'b str,
    ) -> RssArticleView<'b> {
        lazy_static! {
            static ref RELATIVE_IMG_URL: Regex =
                Regex::new(r#"<(img|a)( .*)* (src|href)="/([^"]+)""#).unwrap();
        }

        let trimmed_url = blog_url.trim_end_matches('/');
        let modified_content = RELATIVE_IMG_URL.replace_all(
            &article.parsed_content,
            format!(r#"<$1$2 $3="{trimmed_url}/$4""#),
        );
        RssArticleView {
            title: article.title.as_ref(),
            slug: article.slug.as_ref(),
            content: String::from(modified_content),
            timestamp: article.timestamp,
        }
    }
}

#[derive(Serialize)]
pub struct RssIndexView<'a> {
    pub blog_title: &'a str,
    pub blog_url: &'a str,
    pub blog_description: &'a str,
    pub articles: Vec<RssArticleView<'a>>,
}

const MAX_RELATED_ARTICLES: usize = 5;

#[derive(Serialize)]
pub struct ArticleRenderView<'a> {
    title: &'a str,
    //blog_title: String,
    blog_title: &'a str,
    article: &'a ParsedArticle,
    related_articles: Vec<&'a ParsedArticle>,
    comments: Option<&'a Vec<Comment>>,
    return_path: &'a str,
    body_class: &'a str,
    content_dir: &'a str,
    theme: &'a str,
}

impl<'a> ArticleRenderView<'a> {
    pub fn new(
        article: &'a ParsedArticle,
        all_articles: &'a [ParsedArticle],
        return_path: &'a str,
        theme: &'a str,
        data: &'a CommonData,
    ) -> Self {
        Self {
            title: &article.title,
            blog_title: &data.config.blog_title,
            comments: data.comments.get_for(&article.slug),
            article,
            related_articles: related_articles(article, all_articles),
            return_path,
            body_class: "article",
            content_dir: &data.config.content_dir,
            theme,
        }
    }
}

fn related_articles<'a>(
    article: &'a ParsedArticle,
    all_articles: &'a [ParsedArticle],
) -> Vec<&'a ParsedArticle> {
    let mut related_slugs: HashMap<&String, usize> = HashMap::new();
    let a_tags: HashSet<&String> = HashSet::from_iter(article.tags.iter());

    for b in all_articles {
        let common_tag_count = HashSet::from_iter(b.tags.iter())
            .intersection(&a_tags)
            .count();
        if common_tag_count > 0 && article.slug != b.slug {
            related_slugs.insert(&b.slug, common_tag_count);
        }
    }

    let mut related_slugs: Vec<(String, usize)> = related_slugs
        .iter()
        .map(|kv| {
            let key = String::from(&**kv.0);
            (key, *kv.1)
        })
        .collect();

    related_slugs.sort_by(|a, b| b.1.cmp(&a.1));
    related_slugs
        .iter()
        .map(|kv| fetch_by_slug(&kv.0, all_articles).unwrap()) // unwrap is fine here; we know the
        // article exists
        .take(MAX_RELATED_ARTICLES)
        .collect()
}
