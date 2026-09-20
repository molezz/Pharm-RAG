use rusqlite::{params, Connection, Result};
use std::path::Path;
use crate::parser::ast::{Clause, SearchResult};

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
                FOREIGN KEY(doc_id) REFERENCES documents(id) ON DELETE CASCADE
            );",
            [],
        )?;

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
    pub fn save_document(&mut self, title: &str, path: &str, hash: &str, clauses: &[Clause]) -> Result<i64> {
        let tx = self.conn.transaction()?;

        // Delete existing doc if present
        tx.execute("DELETE FROM documents WHERE path = ?", params![path])?;

        tx.execute(
            "INSERT INTO documents (title, path, hash) VALUES (?, ?, ?)",
            params![title, path, hash],
        )?;
        let doc_id = tx.last_insert_rowid();

        for clause in clauses {
            tx.execute(
                "INSERT INTO clauses (
                    doc_id, doc_title, chapter, section, article,
                    breadcrumb, page_num, content, table_data
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                ],
            )?;
        }

        tx.commit()?;
        Ok(doc_id)
    }

    /// Search clauses with Trigram FTS5 + Fallback for short keywords (<3 chars)
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let char_count = trimmed.chars().count();
        let mut results = Vec::new();

        // Strategy 1: FTS5 Trigram MATCH if 3 or more characters
        if char_count >= 3 {
            // Escape double quotes for FTS5
            let clean_query = trimmed.replace('"', "\"\"");
            let match_expr = format!("\"{}\"", clean_query);

            let mut stmt = self.conn.prepare(
                "SELECT c.id, c.doc_id, c.doc_title, c.chapter, c.section, c.article,
                        c.breadcrumb, c.page_num, c.content, c.table_data, bm25(clauses_fts) as score
                 FROM clauses_fts
                 JOIN clauses c ON c.id = clauses_fts.rowid
                 WHERE clauses_fts MATCH ?
                 ORDER BY score ASC
                 LIMIT ?"
            )?;

            let rows = stmt.query_map(params![match_expr, limit as i64], |row| {
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
                    },
                    row.get::<_, f64>(10)?,
                ))
            });

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
            let like_pattern = format!("%{}%", trimmed);
            let mut stmt = self.conn.prepare(
                "SELECT id, doc_id, doc_title, chapter, section, article,
                        breadcrumb, page_num, content, table_data
                 FROM clauses
                 WHERE content LIKE ? OR breadcrumb LIKE ?
                 ORDER BY id ASC
                 LIMIT ?"
            )?;

            let rows = stmt.query_map(params![like_pattern, like_pattern, limit as i64], |row| {
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
                })
            })?;

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

    /// Retrieve summary statistics
    pub fn get_stats(&self) -> Result<(usize, usize)> {
        let doc_count: usize = self.conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
        let clause_count: usize = self.conn.query_row("SELECT COUNT(*) FROM clauses", [], |r| r.get(0))?;
        Ok((doc_count, clause_count))
    }
}
