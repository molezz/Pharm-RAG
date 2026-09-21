use axum::{
    extract::{Query, Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use crate::storage::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    pub limit: Option<usize>,
    pub status: Option<String>,
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
    pub effective_documents: usize,
    pub draft_documents: usize,
    pub version: &'static str,
}

pub async fn run_server(
    db: Database,
    host: &str,
    port: u16,
    api_key: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        api_key: api_key.clone(),
    };

    let app = Router::new()
        .route("/api/v1/search", get(search_handler))
        .route("/api/v1/stats", get(stats_handler))
        .route("/mcp", post(mcp_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    if api_key.is_some() {
        println!("🚀 PharmRAG API & MCP Server listening on http://{} (Bearer Auth Enabled)", addr);
    } else {
        println!("🚀 PharmRAG API & MCP Server listening on http://{}", addr);
    }

    axum::serve(listener, app).await?;
    Ok(())
}

/// Constant-time byte slice comparison to mitigate timing side-channel attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref required_key) = state.api_key {
        let auth_header = req.headers().get(AUTHORIZATION).and_then(|v| v.to_str().ok());
        let is_valid = match auth_header {
            Some(header) => {
                if let Some(token) = header.strip_prefix("Bearer ") {
                    constant_time_eq(token.trim().as_bytes(), required_key.as_bytes())
                } else {
                    false
                }
            }
            None => false,
        };

        if !is_valid {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(req).await)
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchApiResponse>, StatusCode> {
    let limit = params.limit.unwrap_or(10);
    let q = params.q.clone();
    let status_filter = params.status.clone();
    let db_arc = state.db.clone();

    let results = tokio::task::spawn_blocking(move || {
        let db = db_arc.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        db.search(&q, limit, status_filter.as_deref()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    Ok(Json(SearchApiResponse {
        query: params.q,
        count: results.len(),
        results,
    }))
}

async fn stats_handler(
    State(state): State<AppState>,
) -> Result<Json<StatsApiResponse>, StatusCode> {
    let db_arc = state.db.clone();
    let (docs, clauses, effective, draft) = tokio::task::spawn_blocking(move || {
        let db = db_arc.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        db.get_stats().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    Ok(Json(StatsApiResponse {
        total_documents: docs,
        total_clauses: clauses,
        effective_documents: effective,
        draft_documents: draft,
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
                .unwrap_or("")
                .to_string();
            let limit = params.get("arguments")
                .and_then(|a| a.get("limit"))
                .and_then(|l| l.as_u64())
                .unwrap_or(5) as usize;

            let db_arc = state.db.clone();
            let results = tokio::task::spawn_blocking(move || {
                let db = db_arc.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                db.search(&query, limit, None).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            })
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

            let formatted: Vec<String> = results.into_iter().map(|r| {
                let status_badge = match r.clause.status.as_str() {
                    "draft" => "⚠️【征求意见稿 - 仅供审评趋势参考，非正式生效版本】",
                    "trial" => "🟡【试行版】",
                    "superseded" => "⚪【已废止/已被新版替代】",
                    _ => "🟢【现行有效】",
                };

                format!(
                    "【法规效力】: {}\n【出处层级】: {}\n【页码】: {}\n【条款内容】:\n{}\n---",
                    status_badge,
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
