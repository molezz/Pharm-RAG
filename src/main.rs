use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};
use pharm_rag::parser::parse_file;
use pharm_rag::storage::Database;

#[derive(Parser)]
#[command(name = "pharm-rag")]
#[command(about = "High-Fidelity Regulatory & SOP Retrieval Engine for Biopharma CMC QA", long_about = None)]
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
    },
    /// Search regulatory clauses with FTS5 Trigram precision & breadcrumb tracing
    Search {
        /// Search keyword or question
        query: String,
        /// Maximum number of clauses to return
        #[arg(short, long, default_value = "5")]
        limit: usize,
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
        Commands::Ingest { path } => {
            let mut db = Database::open(&db_path)?;
            if path.is_dir() {
                println!("📂 Ingesting documents from directory: {}", path.display());
                ingest_dir(&mut db, &path)?;
            } else {
                ingest_single_file(&mut db, &path)?;
            }
        }
        Commands::Search { query, limit, json } => {
            let db = Database::open(&db_path)?;
            let start = std::time::Instant::now();
            let results = db.search(&query, limit)?;
            let elapsed = start.elapsed();

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                println!("\n🔍 检索关键词: {} (耗时: {:.2?}, 命中文档条目: {})\n", query.bold().cyan(), elapsed, results.len());
                for (i, res) in results.iter().enumerate() {
                    println!("─────────────────────────────────────────────────────────────────");
                    println!("【{}】 {}", i + 1, res.clause.breadcrumb.bold().green());
                    if let Some(p) = res.clause.page_num {
                        println!("📄 页码: 第 {} 页 | 召回策略: {}", p, res.match_strategy.yellow());
                    } else {
                        println!("📄 召回策略: {}", res.match_strategy.yellow());
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
            let (docs, clauses) = db.get_stats()?;
            println!("\n📊 {} 状态统计", "Pharm-RAG".bold().green());
            println!("─────────────────────────────");
            println!("  数据库文件:   {}", db_path.display());
            println!("  已索引文档数: {}", docs.to_string().cyan());
            println!("  已切分法规条款: {}", clauses.to_string().cyan());
            println!("─────────────────────────────\n");
        }
    }

    Ok(())
}

fn ingest_single_file(db: &mut Database, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");
    println!("⏳ Parsing {}...", path.display());
    let clauses = parse_file(path)?;
    let p_str = path.to_string_lossy();
    db.save_document(title, &p_str, "hash_placeholder", &clauses)?;
    println!("✅ 成功录入: {} (共切分出 {} 条法规条款)", title.bold().green(), clauses.len());
    Ok(())
}

fn ingest_dir(db: &mut Database, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(dir)?;
    let mut total_files = 0;
    let mut total_clauses = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ["pdf", "docx", "md", "txt"].contains(&ext_lower.as_str()) {
                let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");
                if let Ok(clauses) = parse_file(&path) {
                    let p_str = path.to_string_lossy();
                    if db.save_document(title, &p_str, "hash_placeholder", &clauses).is_ok() {
                        total_files += 1;
                        total_clauses += clauses.len();
                        println!("  ✓ 已索引: {} ({} 条目)", title, clauses.len());
                    }
                }
            }
        }
    }

    println!("\n🎉 目录导入完成! 共处理 {} 篇法规/SOP，入库 {} 个精确条款。", total_files, total_clauses);
    Ok(())
}
