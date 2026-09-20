use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use crate::storage::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchApiResponse {
    pub query: String,
    pub count: usize,
    pub results: Vec<crate::parser::ast::SearchResult>,
}

#[derive(Serialize)]
pub struct StatsApiResponse {
    pub total_documents: usize,
    pub total_clauses: usize,
    pub version: &'static str,
}

pub async fn run_server(db: Database, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
    };

    let app = Router::new()
        .route("/api/v1/search", get(search_handler))
        .route("/api/v1/stats", get(stats_handler))
        .route("/mcp", post(mcp_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("🚀 Pharm-RAG API & MCP Server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchApiResponse>, StatusCode> {
    let limit = params.limit.unwrap_or(10);
    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let results = db.search(&params.q, limit).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SearchApiResponse {
        query: params.q,
        count: results.len(),
        results,
    }))
}

async fn stats_handler(
    State(state): State<AppState>,
) -> Result<Json<StatsApiResponse>, StatusCode> {
    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (docs, clauses) = db.get_stats().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(StatsApiResponse {
        total_documents: docs,
        total_clauses: clauses,
        version: env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Deserialize)]
pub struct McpRequest {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

async fn mcp_handler(
    State(state): State<AppState>,
    Json(payload): Json<McpRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = payload.id.unwrap_or(serde_json::Value::Number(1.into()));

    match payload.method.as_str() {
        "tools/list" => Ok(Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "search_regulations",
                        "description": "Search biopharma regulations, CDE/FDA guidelines, and company SOPs with exact clause breadcrumbs",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search keyword or regulation query (e.g. '无菌检查', 'CAR-T 药学变更')" },
                                "limit": { "type": "integer", "description": "Max number of clauses to return (default 5)" }
                            },
                            "required": ["query"]
                        }
                    }
                ]
            }
        }))),
        "tools/call" => {
            let params = payload.params.unwrap_or_default();
            let query = params.get("arguments")
                .and_then(|a| a.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let limit = params.get("arguments")
                .and_then(|a| a.get("limit"))
                .and_then(|l| l.as_u64())
                .unwrap_or(5) as usize;

            let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let results = db.search(query, limit).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let formatted: Vec<String> = results.into_iter().map(|r| {
                format!(
                    "【法规出处】: {}\n【页码】: {}\n【条款内容】:\n{}\n---",
                    r.clause.breadcrumb,
                    r.clause.page_num.map_or("未知".to_string(), |p| p.to_string()),
                    r.clause.content
                )
            }).collect();

            Ok(Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [
                        {
                            "type": "text",
                            "text": formatted.join("\n\n")
                        }
                    ]
                }
            })))
        }
        _ => Ok(Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": "Method not found" }
        }))),
    }
}
