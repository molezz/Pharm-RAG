use rusqlite::{params, Connection, Result};
use std::path::Path;
use crate::parser::ast::{Clause, SearchResult};

#[derive(Debug, Clone)]
pub enum SaveOutcome {
    Saved { doc_id: i64, clause_count: usize },
    DuplicateSkipped { existing_title: String, existing_path: String },
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        // High performance pragmas
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;"
        )?;

        // Documents table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                hash TEXT,
                status TEXT NOT NULL DEFAULT 'effective',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
            [],
        )?;

        // Clauses table (structured AST records)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS clauses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                doc_id INTEGER NOT NULL,
                doc_title TEXT NOT NULL,
                chapter TEXT,
                section TEXT,
                article TEXT,
                breadcrumb TEXT NOT NULL,
                page_num INTEGER,
                content TEXT NOT NULL,
                table_data TEXT,
                status TEXT NOT NULL DEFAULT 'effective',
                FOREIGN KEY(doc_id) REFERENCES documents(id) ON DELETE CASCADE
            );",
            [],
        )?;

        // Clause embeddings table for dense semantic vector search
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS clause_embeddings (
                clause_id INTEGER PRIMARY KEY,
                embedding BLOB NOT NULL,
                FOREIGN KEY(clause_id) REFERENCES clauses(id) ON DELETE CASCADE
            );",
            [],
        )?;

        // Migrate older databases if status column is missing
        let _ = self.conn.execute("ALTER TABLE documents ADD COLUMN status TEXT DEFAULT 'effective'", []);
        let _ = self.conn.execute("ALTER TABLE clauses ADD COLUMN status TEXT DEFAULT 'effective'", []);

        // FTS5 Trigram virtual table for full-text search
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS clauses_fts USING fts5(
                content,
                breadcrumb,
                doc_title,
                content=clauses,
                content_rowid=id,
                tokenize='trigram'
            );",
            [],
        )?;

        // FTS5 Triggers to keep FTS in sync with clauses table
        self.conn.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS clauses_ai AFTER INSERT ON clauses BEGIN
                INSERT INTO clauses_fts(rowid, content, breadcrumb, doc_title)
                VALUES (new.id, new.content, new.breadcrumb, new.doc_title);
            END;
            CREATE TRIGGER IF NOT EXISTS clauses_ad AFTER DELETE ON clauses BEGIN
                INSERT INTO clauses_fts(clauses_fts, rowid, content, breadcrumb, doc_title)
                VALUES('delete', old.id, old.content, old.breadcrumb, old.doc_title);
            END;
            CREATE TRIGGER IF NOT EXISTS clauses_au AFTER UPDATE ON clauses BEGIN
                INSERT INTO clauses_fts(clauses_fts, rowid, content, breadcrumb, doc_title)
                VALUES('delete', old.id, old.content, old.breadcrumb, old.doc_title);
                INSERT INTO clauses_fts(rowid, content, breadcrumb, doc_title)
                VALUES (new.id, new.content, new.breadcrumb, new.doc_title);
            END;"
        )?;

        Ok(())
    }

    /// Insert or update a document and its clauses
    /// If an identical content hash already exists under a different path, skip and return DuplicateSkipped.
    pub fn save_document(&mut self, title: &str, path: &str, hash: &str, status: &str, clauses: &[Clause]) -> Result<SaveOutcome> {
        // Check if an identical file hash already exists under another path/filename
        if !hash.is_empty() && hash != "hash_placeholder" {
            let mut check_stmt = self.conn.prepare(
                "SELECT id, title, path FROM documents WHERE hash = ? AND path != ?"
            )?;
            let mut dup_rows = check_stmt.query(params![hash, path])?;
            if let Some(row) = dup_rows.next()? {
                let existing_title: String = row.get(1)?;
                let existing_path: String = row.get(2)?;
                return Ok(SaveOutcome::DuplicateSkipped {
                    existing_title,
                    existing_path,
                });
            }
        }

        let tx = self.conn.transaction()?;

        // Delete existing doc if re-ingesting same path
        tx.execute("DELETE FROM documents WHERE path = ?", params![path])?;

        tx.execute(
            "INSERT INTO documents (title, path, hash, status) VALUES (?, ?, ?, ?)",
            params![title, path, hash, status],
        )?;
        let doc_id = tx.last_insert_rowid();

        for clause in clauses {
            tx.execute(
                "INSERT INTO clauses (
                    doc_id, doc_title, chapter, section, article,
                    breadcrumb, page_num, content, table_data, status
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    doc_id,
                    clause.doc_title,
                    clause.chapter,
                    clause.section,
                    clause.article,
                    clause.breadcrumb,
                    clause.page_num,
                    clause.content,
                    clause.table_data,
                    clause.status,
                ],
            )?;
        }

        tx.commit()?;
        Ok(SaveOutcome::Saved { doc_id, clause_count: clauses.len() })
    }

    /// Search clauses with Trigram FTS5 + Fallback for short keywords (<3 chars)
    /// Sort priority: effective/trial (现行/试行) > draft (征求意见稿) > superseded (已废止)
    pub fn search(&self, query: &str, limit: usize, status_filter: Option<&str>) -> Result<Vec<SearchResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let char_count = trimmed.chars().count();
        let mut results = Vec::new();

        // Strategy 1: FTS5 Trigram MATCH if 3 or more characters
        if char_count >= 3 {
            let clean_query = trimmed.replace('"', "\"\"");
            let match_expr = format!("\"{}\"", clean_query);

            let sql = match status_filter {
                Some(_) => {
                    "SELECT c.id, c.doc_id, c.doc_title, c.chapter, c.section, c.article,
                            c.breadcrumb, c.page_num, c.content, c.table_data, c.status, bm25(clauses_fts) as score
                     FROM clauses_fts
                     JOIN clauses c ON c.id = clauses_fts.rowid
                     WHERE clauses_fts MATCH ? AND c.status = ?
                     ORDER BY CASE c.status WHEN 'effective' THEN 1 WHEN 'trial' THEN 2 WHEN 'draft' THEN 3 ELSE 4 END ASC, score ASC
                     LIMIT ?"
                }
                None => {
                    "SELECT c.id, c.doc_id, c.doc_title, c.chapter, c.section, c.article,
                            c.breadcrumb, c.page_num, c.content, c.table_data, c.status, bm25(clauses_fts) as score
                     FROM clauses_fts
                     JOIN clauses c ON c.id = clauses_fts.rowid
                     WHERE clauses_fts MATCH ?
                     ORDER BY CASE c.status WHEN 'effective' THEN 1 WHEN 'trial' THEN 2 WHEN 'draft' THEN 3 ELSE 4 END ASC, score ASC
                     LIMIT ?"
                }
            };

            let mut stmt = self.conn.prepare(sql)?;
            let map_fn = |row: &rusqlite::Row| {
                Ok((
                    Clause {
                        id: row.get(0)?,
                        doc_id: row.get(1)?,
                        doc_title: row.get(2)?,
                        chapter: row.get(3)?,
                        section: row.get(4)?,
                        article: row.get(5)?,
                        breadcrumb: row.get(6)?,
                        page_num: row.get(7)?,
                        content: row.get(8)?,
                        table_data: row.get(9)?,
                        status: row.get(10)?,
                    },
                    row.get::<_, f64>(11)?,
                ))
            };

            let rows = if let Some(sf) = status_filter {
                stmt.query_map(params![match_expr, sf, limit as i64], map_fn)
            } else {
                stmt.query_map(params![match_expr, limit as i64], map_fn)
            };

            if let Ok(iter) = rows {
                for item in iter.flatten() {
                    results.push(SearchResult {
                        clause: item.0,
                        score: item.1,
                        match_strategy: "FTS5_Trigram".to_string(),
                    });
                }
            }
        }

        // Strategy 2: Fallback to exact LIKE if <3 chars OR Trigram returned 0 results
        if results.is_empty() {
            let escaped = escape_like(trimmed);
            let like_pattern = format!("%{}%", escaped);
            let sql = match status_filter {
                Some(_) => {
                    "SELECT id, doc_id, doc_title, chapter, section, article,
                            breadcrumb, page_num, content, table_data, status
                     FROM clauses
                     WHERE (content LIKE ? ESCAPE '\\' OR breadcrumb LIKE ? ESCAPE '\\') AND status = ?
                     ORDER BY CASE status WHEN 'effective' THEN 1 WHEN 'trial' THEN 2 WHEN 'draft' THEN 3 ELSE 4 END ASC, id ASC
                     LIMIT ?"
                }
                None => {
                    "SELECT id, doc_id, doc_title, chapter, section, article,
                            breadcrumb, page_num, content, table_data, status
                     FROM clauses
                     WHERE content LIKE ? ESCAPE '\\' OR breadcrumb LIKE ? ESCAPE '\\'
                     ORDER BY CASE status WHEN 'effective' THEN 1 WHEN 'trial' THEN 2 WHEN 'draft' THEN 3 ELSE 4 END ASC, id ASC
                     LIMIT ?"
                }
            };

            let mut stmt = self.conn.prepare(sql)?;
            let map_fn = |row: &rusqlite::Row| {
                Ok(Clause {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    doc_title: row.get(2)?,
                    chapter: row.get(3)?,
                    section: row.get(4)?,
                    article: row.get(5)?,
                    breadcrumb: row.get(6)?,
                    page_num: row.get(7)?,
                    content: row.get(8)?,
                    table_data: row.get(9)?,
                    status: row.get(10)?,
                })
            };

            let rows = if let Some(sf) = status_filter {
                stmt.query_map(params![like_pattern, like_pattern, sf, limit as i64], map_fn)?
            } else {
                stmt.query_map(params![like_pattern, like_pattern, limit as i64], map_fn)?
            };

            for item in rows.flatten() {
                results.push(SearchResult {
                    clause: item,
                    score: 1.0,
                    match_strategy: "Substring_LIKE_Fallback".to_string(),
                });
            }
        }

        Ok(results)
    }

    /// Save dense vector embedding for a specific clause
    pub fn save_clause_embedding(&self, clause_id: i64, embedding: &[f32]) -> Result<()> {
        let bytes = crate::storage::semantic::vector_to_bytes(embedding);
        self.conn.execute(
            "INSERT OR REPLACE INTO clause_embeddings (clause_id, embedding) VALUES (?, ?)",
            params![clause_id, bytes],
        )?;
        Ok(())
    }

    /// Retrieve clauses that do not yet have vector embeddings
    pub fn get_unembedded_clauses(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.breadcrumb || '\n' || c.content
             FROM clauses c
             LEFT JOIN clause_embeddings e ON c.id = e.clause_id
             WHERE e.clause_id IS NULL"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut res = Vec::new();
        for item in rows.flatten() {
            res.push(item);
        }
        Ok(res)
    }

    /// Count how many clauses have embeddings
    pub fn get_embedding_stats(&self) -> Result<(usize, usize)> {
        let total_clauses: usize = self.conn.query_row("SELECT COUNT(*) FROM clauses", [], |r| r.get(0))?;
        let embedded_clauses: usize = self.conn.query_row("SELECT COUNT(*) FROM clause_embeddings", [], |r| r.get(0))?;
        Ok((embedded_clauses, total_clauses))
    }

    /// Dense semantic vector search using cosine similarity
    pub fn search_vector(&self, query_vector: &[f32], limit: usize, status_filter: Option<&str>) -> Result<Vec<SearchResult>> {
        if limit == 0 || query_vector.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.min(1000);

        let sql = match status_filter {
            Some(_) => {
                "SELECT c.id, c.doc_id, c.doc_title, c.chapter, c.section, c.article,
                        c.breadcrumb, c.page_num, c.content, c.table_data, c.status, e.embedding
                 FROM clause_embeddings e
                 JOIN clauses c ON c.id = e.clause_id
                 WHERE c.status = ?"
            }
            None => {
                "SELECT c.id, c.doc_id, c.doc_title, c.chapter, c.section, c.article,
                        c.breadcrumb, c.page_num, c.content, c.table_data, c.status, e.embedding
                 FROM clause_embeddings e
                 JOIN clauses c ON c.id = e.clause_id"
            }
        };

        let mut stmt = self.conn.prepare(sql)?;
        let map_fn = |row: &rusqlite::Row| {
            let embedding_bytes: Vec<u8> = row.get(11)?;
            let emb = crate::storage::semantic::bytes_to_vector(&embedding_bytes);
            let sim = crate::storage::semantic::cosine_similarity(query_vector, &emb);

            Ok((
                Clause {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    doc_title: row.get(2)?,
                    chapter: row.get(3)?,
                    section: row.get(4)?,
                    article: row.get(5)?,
                    breadcrumb: row.get(6)?,
                    page_num: row.get(7)?,
                    content: row.get(8)?,
                    table_data: row.get(9)?,
                    status: row.get(10)?,
                },
                sim as f64,
            ))
        };

        let mut scored_clauses = Vec::new();
        if let Some(sf) = status_filter {
            let rows = stmt.query_map(params![sf], map_fn)?;
            for item in rows.flatten() {
                scored_clauses.push(item);
            }
        } else {
            let rows = stmt.query_map([], map_fn)?;
            for item in rows.flatten() {
                scored_clauses.push(item);
            }
        }

        // Sort by status priority first, then cosine similarity descending
        scored_clauses.sort_by(|a, b| {
            let status_rank = |s: &str| match s {
                "effective" => 1,
                "trial" => 2,
                "draft" => 3,
                _ => 4,
            };
            let rank_a = status_rank(&a.0.status);
            let rank_b = status_rank(&b.0.status);
            if rank_a != rank_b {
                rank_a.cmp(&rank_b)
            } else {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        scored_clauses.truncate(limit);

        let results = scored_clauses.into_iter().map(|(clause, sim)| SearchResult {
            clause,
            score: sim,
            match_strategy: "BGE_M3_Semantic".to_string(),
        }).collect();

        Ok(results)
    }

    /// Hybrid Search: Combining SQLite FTS5 (Trigram) and BGE-M3 (Dense Vector) with Reciprocal Rank Fusion (RRF)
    pub fn search_hybrid(&self, query: &str, query_vector: &[f32], limit: usize, status_filter: Option<&str>) -> Result<Vec<SearchResult>> {
        let candidate_limit = (limit * 3).max(20);
        let fts_results = self.search(query, candidate_limit, status_filter)?;
        let vec_results = self.search_vector(query_vector, candidate_limit, status_filter)?;

        // RRF Constant k = 60
        const K: f64 = 60.0;
        let mut rrf_scores: std::collections::HashMap<i64, (Clause, f64, bool, bool)> = std::collections::HashMap::new();

        for (rank, item) in fts_results.iter().enumerate() {
            if let Some(id) = item.clause.id {
                let rrf = 1.0 / (K + rank as f64 + 1.0);
                rrf_scores.insert(id, (item.clause.clone(), rrf, true, false));
            }
        }

        for (rank, item) in vec_results.iter().enumerate() {
            if let Some(id) = item.clause.id {
                let rrf = 1.0 / (K + rank as f64 + 1.0);
                if let Some(entry) = rrf_scores.get_mut(&id) {
                    entry.1 += rrf;
                    entry.3 = true; // Both FTS and Vector matched!
                } else {
                    rrf_scores.insert(id, (item.clause.clone(), rrf, false, true));
                }
            }
        }

        let mut combined: Vec<(Clause, f64, String)> = rrf_scores.into_values().map(|(clause, score, in_fts, in_vec)| {
            let strategy = if in_fts && in_vec {
                "Hybrid_RRF (FTS5 + BGE-M3)".to_string()
            } else if in_fts {
                "FTS5_Trigram".to_string()
            } else {
                "BGE_M3_Semantic".to_string()
            };
            (clause, score, strategy)
        }).collect();

        combined.sort_by(|a, b| {
            let status_rank = |s: &str| match s {
                "effective" => 1,
                "trial" => 2,
                "draft" => 3,
                _ => 4,
            };
            let rank_a = status_rank(&a.0.status);
            let rank_b = status_rank(&b.0.status);
            if rank_a != rank_b {
                rank_a.cmp(&rank_b)
            } else {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        combined.truncate(limit);

        let results = combined.into_iter().map(|(clause, score, strategy)| SearchResult {
            clause,
            score,
            match_strategy: strategy,
        }).collect();

        Ok(results)
    }

    /// Retrieve summary statistics with status breakdown
    pub fn get_stats(&self) -> Result<(usize, usize, usize, usize)> {
        let doc_count: usize = self.conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
        let clause_count: usize = self.conn.query_row("SELECT COUNT(*) FROM clauses", [], |r| r.get(0))?;
        let draft_count: usize = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE status = 'draft'", [], |r| r.get(0))?;
        let effective_count: usize = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE status IN ('effective', 'trial')", [], |r| r.get(0))?;
        Ok((doc_count, clause_count, effective_count, draft_count))
    }
}

/// Compute SHA-256 hash of a file for content-based deduplication
pub fn compute_file_hash(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    if let Ok(mut file) = std::fs::File::open(path) {
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        while let Ok(n) = file.read(&mut buffer) {
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let result = hasher.finalize();
        return result.iter().map(|b| format!("{:02x}", b)).collect();
    }
    "hash_fallback".to_string()
}

/// Escape SQLite LIKE wildcard characters (%, _, \)
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('%', "\\%")
     .replace('_', "\\_")
}


