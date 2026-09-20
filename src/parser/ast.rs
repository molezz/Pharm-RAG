use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clause {
    pub id: Option<i64>,
    pub doc_id: Option<i64>,
    pub doc_title: String,
    pub chapter: String,
    pub section: String,
    pub article: String,
    pub breadcrumb: String,
    pub page_num: Option<i32>,
    pub content: String,
    pub table_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: Option<i64>,
    pub title: String,
    pub path: String,
    pub hash: String,
    pub clause_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub clause: Clause,
    pub score: f64,
    pub match_strategy: String,
}
