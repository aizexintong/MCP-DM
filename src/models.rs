//! 数据模型
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Entry {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub cause: String,
    pub effect: String,
    pub cat_path: String,
    pub tags: Vec<String>,
    pub source: Vec<String>,
    pub state: String,
    pub state_reason: String,
    pub secrecy: String,
    pub inferred: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_fav: bool,
    pub is_pick: bool,
    pub likes: i32,
    pub hits: i64,
    pub mark_level: i32,
    pub backlinks: i64,
}

#[derive(Debug, Serialize)]
pub struct EntryDetail {
    #[serde(flatten)]
    pub entry: Entry,
    pub html: String,
    pub links_out: Vec<LinkRef>,
    pub links_in: Vec<LinkRef>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkRef {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchHit {
    pub id: i64,
    pub title: String,
    pub cat_path: String,
    pub snippet: String,
    pub cause: String,
    pub effect: String,
    pub state: String,
    pub mark_level: i32,
    pub score: f32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub status: String,
    pub query: String,
    pub total: i64,
    pub results: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// 可为空——只按分类筛选时不需要关键词
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub cat: Option<String>,
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    #[serde(default)]
    pub cat: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntry {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub cause: String,
    #[serde(default)]
    pub effect: String,
    pub cat_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntry {
    pub title: Option<String>,
    pub body: Option<String>,
    pub cause: Option<String>,
    pub effect: Option<String>,
    pub cat_path: Option<String>,
    pub state: Option<String>,
    pub state_reason: Option<String>,
    /// 传了就整体替换，传 null 不动
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CategoryNode {
    pub path: String,
    pub label: String,
    pub depth: i32,
    pub count: i64,
}


#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimelineItem {
    pub id: i64,
    pub title: String,
    pub cat_path: String,
    pub updated_at: DateTime<Utc>,
    pub cause: String,
    pub effect: String,
    pub mark_level: i32,
}
