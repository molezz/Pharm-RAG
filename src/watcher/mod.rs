use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use crate::storage::Database;
use crate::parser::parse_file;

pub fn watch_directory<P: AsRef<Path>>(dir_path: P, db_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(dir_path.as_ref(), RecursiveMode::Recursive)?;

    println!("👀 Watching directory '{}' for regulatory document updates...", dir_path.as_ref().display());

    let mut db = Database::open(&db_path)?;

    for res in rx {
        match res {
            Ok(event) => {
                for path in event.paths {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if ["pdf", "docx", "md", "txt"].contains(&ext_lower.as_str()) && path.exists() {
                            println!("📄 Detected file change: {:?}", path);
                            // Wait briefly for file write to complete
                            std::thread::sleep(Duration::from_millis(300));
                            match parse_file(&path) {
                                Ok(clauses) => {
                                    let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("法规");
                                    let p_str = path.to_string_lossy();
                                    match db.save_document(title, &p_str, "hash_auto", &clauses) {
                                        Ok(_) => println!("✅ Successfully auto-ingested {} clauses from {}", clauses.len(), title),
                                        Err(e) => eprintln!("❌ Database error saving {}: {}", title, e),
                                    }
                                }
                                Err(e) => eprintln!("❌ Parse error on {:?}: {}", path, e),
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("❌ Watcher error: {:?}", e),
        }
    }

    Ok(())
}
