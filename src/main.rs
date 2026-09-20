use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};
use pharm_rag::parser::parse_file;
use pharm_rag::storage::{Database, compute_file_hash};

#[derive(Parser)]
#[command(name = "pharm-rag")]
#[command(about = "Pharmaceutical Regulatory and GxP SOP Retrieval Engine", long_about = None)]
#[command(version)]
struct Cli {
    /// Custom path to SQLite database file
    #[arg(long, global = true, default_value = "pharm.db")]
    db: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest a regulatory document (PDF, DOCX, MD, TXT) or directory
    Ingest {
        /// File or folder path to ingest
        path: PathBuf,
        /// Explicitly override status: effective, trial, draft, superseded
        #[arg(long)]
        status: Option<String>,
    },
    /// Generate dense vector embeddings for indexed clauses using BGE-M3
    Embed {
        /// Custom model storage directory (defaults to ./models)
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// Use lightweight BGE-Small-ZH instead of BGE-M3
        #[arg(long)]
        small: bool,
    },
    /// Search regulatory clauses with FTS5 Trigram precision, BGE-M3 semantic, or Hybrid RRF
    Search {
        /// Search keyword or question
        query: String,
        /// Maximum number of clauses to return
        #[arg(short, long, default_value = "5")]
        limit: usize,
        /// Enable Hybrid Search (FTS5 Trigram + BGE-M3 Dense Vector)
        #[arg(long)]
        hybrid: bool,
        /// Custom model storage directory for hybrid search
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// Filter by status: effective, trial, draft, superseded
        #[arg(long)]
        status: Option<String>,
        /// Only show in-force regulations (effective and trial), filtering out drafts
        #[arg(long)]
        only_effective: bool,
        /// Output formatted JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },
    /// Watch a directory for auto-ingestion when files are added or updated
    Watch {
        /// Directory to monitor
        dir: PathBuf,
    },
    /// Start local HTTP REST API & MCP Server for Agent integration
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Show database statistics and indexed regulations
    Stats,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let db_path = cli.db;

    match cli.command {
        Commands::Ingest { path, status } => {
            let mut db = Database::open(&db_path)?;
            if path.is_dir() {
                println!("📂 Ingesting documents from directory: {}", path.display());
                ingest_dir(&mut db, &path, status.as_deref())?;
            } else {
                ingest_single_file(&mut db, &path, status.as_deref())?;
            }
        }
        Commands::Embed { model_dir, small } => {
            let db = Database::open(&db_path)?;
            let engine = pharm_rag::storage::SemanticEngine::new(model_dir, !small)?;
            let unembedded = db.get_unembedded_clauses()?;
            if unembedded.is_empty() {
                println!("🎉 所有法规条款均已生成向量索引！无需重复计算。");
            } else {
                println!("⏳ 开始生成语义向量嵌入: 共 {} 个条款待处理 (模型: {})...", unembedded.len(), engine.model_name());
                let total = unembedded.len();
                let mut processed = 0;
                for chunk in unembedded.chunks(16) {
                    let texts: Vec<String> = chunk.iter().map(|c| c.1.clone()).collect();
                    let embeddings = engine.embed_batch(&texts)?;
                    for (item, emb) in chunk.iter().zip(embeddings.iter()) {
                        db.save_clause_embedding(item.0, emb)?;
                    }
                    processed += chunk.len();
                    print!("\r  ⚡ 进度: {} / {} ({:.1}%)", processed, total, (processed as f64 / total as f64) * 100.0);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
                println!("\n✅ 成功完成 {} 个条款的语义向量生成！已持久化至 SQLite 数据库。", total);
            }
        }
        Commands::Search { query, limit, hybrid, model_dir, status, only_effective, json } => {
            let db = Database::open(&db_path)?;
            let status_filter = if only_effective {
                Some("effective")
            } else {
                status.as_deref()
            };

            let start = std::time::Instant::now();
            let results = if hybrid {
                let engine = pharm_rag::storage::SemanticEngine::new(model_dir, true)?;
                let q_vec = engine.embed_query(&query)?;
                db.search_hybrid(&query, &q_vec, limit, status_filter)?
            } else {
                db.search(&query, limit, status_filter)?
            };
            let elapsed = start.elapsed();

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                let mode_label = if hybrid { "Hybrid 混合检索 (FTS5 + BGE-M3)" } else { "精确检索 (FTS5 Trigram)" };
                println!("\n🔍 检索关键词: {} [{}] (耗时: {:.2?}, 命中文档条目: {})\n", query.bold().cyan(), mode_label.green(), elapsed, results.len());
                for (i, res) in results.iter().enumerate() {
                    let badge = match res.clause.status.as_str() {
                        "draft" => "🔴 [征求意见稿 (非现行/仅供审评趋势参考)]".red().bold(),
                        "trial" => "🟡 [试行版 (现行有效监管)]".yellow().bold(),
                        "superseded" => "⚪ [已废止/历史版本]".white().dimmed(),
                        _ => "🟢 [现行正式版]".green().bold(),
                    };

                    println!("─────────────────────────────────────────────────────────────────");
                    println!("【{}】 {}  {}", i + 1, res.clause.breadcrumb.bold().cyan(), badge);
                    if let Some(p) = res.clause.page_num {
                        println!("📄 页码: 第 {} 页 | 召回策略: {}", p, res.match_strategy.yellow());
                    } else {
                        println!("📄 召回策略: {}", res.match_strategy.yellow());
                    }
                    
                    if res.clause.status == "draft" {
                        println!("{}", "⚠️ 提示：该条款来自征求意见稿，正式申报与GMP合规请核对现行版指导原则。".red());
                    }

                    println!("\n{}", res.clause.content);
                    println!();
                }
            }
        }
        Commands::Watch { dir } => {
            pharm_rag::watcher::watch_directory(dir, db_path)?;
        }
        Commands::Serve { port } => {
            let db = Database::open(&db_path)?;
            pharm_rag::server::run_server(db, port).await?;
        }
        Commands::Stats => {
            let db = Database::open(&db_path)?;
            let (docs, clauses, effective, draft) = db.get_stats()?;
            let (embedded, total_clauses) = db.get_embedding_stats().unwrap_or((0, clauses));
            let pct = if total_clauses > 0 { (embedded as f64 / total_clauses as f64) * 100.0 } else { 0.0 };

            println!("\n📊 {} 状态统计", "Pharm-RAG".bold().green());
            println!("─────────────────────────────");
            println!("  数据库文件:     {}", db_path.display());
            println!("  已索引文档总数: {}", docs.to_string().cyan());
            println!("    ├─ 现行/试行版: {}", effective.to_string().green());
            println!("    └─ 征求意见稿: {}", draft.to_string().yellow());
            println!("  已切分法规条款: {}", clauses.to_string().cyan());
            println!("  语义向量覆盖率: {} / {} ({:.1}%)", embedded.to_string().green(), total_clauses, pct);
            println!("─────────────────────────────\n");
        }
    }

    Ok(())
}

fn ingest_single_file(db: &mut Database, path: &Path, override_status: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");
    println!("⏳ Parsing {}...", path.display());
    let mut clauses = parse_file(path)?;
    let p_str = path.to_string_lossy();
    let hash = compute_file_hash(path);
    
    let status = if let Some(s) = override_status {
        for c in &mut clauses {
            c.status = s.to_string();
        }
        s
    } else {
        clauses.first().map(|c| c.status.as_str()).unwrap_or("effective")
    };

    match db.save_document(title, &p_str, &hash, status, &clauses)? {
        pharm_rag::storage::SaveOutcome::Saved { clause_count, .. } => {
            println!("✅ 成功录入: {} [{}] (共切分出 {} 条法规条款)", title.bold().green(), status.yellow(), clause_count);
        }
        pharm_rag::storage::SaveOutcome::DuplicateSkipped { existing_title, existing_path } => {
            println!("⏭️  {} 检测到文件内容与已收录的《{}》（{}）完全一致（SHA-256指纹相同），已自动去重跳过，避免法规条目冗余污染。",
                "跳过重复文件:".bold().yellow(),
                existing_title.cyan(),
                existing_path
            );
        }
    }

    Ok(())
}

fn ingest_dir(db: &mut Database, dir: &Path, override_status: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(dir)?;
    let mut total_files = 0;
    let mut total_clauses = 0;
    let mut skipped_duplicates = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ["pdf", "docx", "md", "txt"].contains(&ext_lower.as_str()) {
                let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");
                if let Ok(mut clauses) = parse_file(&path) {
                    let p_str = path.to_string_lossy();
                    let hash = compute_file_hash(&path);
                    let status = if let Some(s) = override_status {
                        for c in &mut clauses {
                            c.status = s.to_string();
                        }
                        s
                    } else {
                        clauses.first().map(|c| c.status.as_str()).unwrap_or("effective")
                    };

                    match db.save_document(title, &p_str, &hash, status, &clauses) {
                        Ok(pharm_rag::storage::SaveOutcome::Saved { clause_count, .. }) => {
                            total_files += 1;
                            total_clauses += clause_count;
                            println!("  ✓ 已索引: {} [{}] ({} 条目)", title, status.yellow(), clause_count);
                        }
                        Ok(pharm_rag::storage::SaveOutcome::DuplicateSkipped { existing_title, .. }) => {
                            skipped_duplicates += 1;
                            println!("  ⏭️  已去重跳过: {} (与《{}》内容完全一致)", title.yellow(), existing_title.cyan());
                        }
                        Err(e) => eprintln!("  ❌ 录入失败 {}: {}", title, e),
                    }
                }
            }
        }
    }

    println!("\n🎉 目录导入完成! 成功录入 {} 篇法规/SOP (共 {} 个条款)，自动去重跳过 {} 个重复文件。",
        total_files.to_string().green(),
        total_clauses.to_string().cyan(),
        skipped_duplicates.to_string().yellow()
    );
    Ok(())
}
