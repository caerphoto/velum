use serde::Serialize;
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
    pub content: String,
    pub preview: String,
    pub slug: String,
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
