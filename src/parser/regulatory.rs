use regex::Regex;
use super::ast::Clause;

pub struct RegulatoryParser {
    re_chapter: Regex,
    re_section: Regex,
    re_article: Regex,
}

impl Default for RegulatoryParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RegulatoryParser {
    pub fn new() -> Self {
        Self {
            // 第X章 或 一、概况
            re_chapter: Regex::new(r"^(第[一二三四五六七八九十百]+章|[一二三四五六七八九十]+、)\s*(.*)$").unwrap(),
            // 第X节 或 (一) 生产与质控
            re_section: Regex::new(r"^(第[一二三四五六七八九十]+节|（[一二三四五六七八九十]+）|\([一二三四五六七八九十]+\))\s*(.*)$").unwrap(),
            // 第X条 或 1. 原材料控制
            re_article: Regex::new(r"^(第[一二三四五六七八九十百]+条|\d+[\.、])\s*(.*)$").unwrap(),
        }
    }

    /// Parse document text into clauses with breadcrumb hierarchy
    pub fn parse(&self, doc_title: &str, raw_text: &str) -> Vec<Clause> {
        let mut clauses = Vec::new();
        let mut current_chapter = String::new();
        let mut current_section = String::new();
        let mut current_article = String::new();
        let mut current_lines = Vec::new();
        let mut current_page: Option<i32> = None;

        let re_page = Regex::new(r"\[Page\s*(\d+)\]|---\s*第\s*(\d+)\s*页\s*---").unwrap();

        let flush_clause = |clauses: &mut Vec<Clause>,
                            chapter: &str,
                            section: &str,
                            article: &str,
                            lines: &mut Vec<String>,
                            page: Option<i32>| {
            let content = lines.join("\n").trim().to_string();
            if !content.is_empty() {
                let mut parts = vec![doc_title.to_string()];
                if !chapter.is_empty() {
                    parts.push(chapter.to_string());
                }
                if !section.is_empty() {
                    parts.push(section.to_string());
                }
                if !article.is_empty() {
                    parts.push(article.to_string());
                }
                let breadcrumb = parts.join(" > ");

                clauses.push(Clause {
                    id: None,
                    doc_id: None,
                    doc_title: doc_title.to_string(),
                    chapter: chapter.to_string(),
                    section: section.to_string(),
                    article: article.to_string(),
                    breadcrumb,
                    page_num: page,
                    content,
                    table_data: None,
                });
            }
            lines.clear();
        };

        for line in raw_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for page marker
            if let Some(caps) = re_page.captures(trimmed) {
                if let Some(p) = caps.get(1).or_else(|| caps.get(2)) {
                    if let Ok(num) = p.as_str().parse::<i32>() {
                        current_page = Some(num);
                    }
                }
                continue;
            }

            // Check Chapter
            if let Some(caps) = self.re_chapter.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                current_chapter = caps.get(0).map_or("", |m| m.as_str()).to_string();
                current_section.clear();
                current_article.clear();
                continue;
            }

            // Check Section
            if let Some(caps) = self.re_section.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                current_section = caps.get(0).map_or("", |m| m.as_str()).to_string();
                current_article.clear();
                continue;
            }

            // Check Article
            if let Some(caps) = self.re_article.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                current_article = caps.get(0).map_or("", |m| m.as_str()).to_string();
                current_lines.push(trimmed.to_string());
                continue;
            }

            // Regular body text
            current_lines.push(trimmed.to_string());
        }

        // Flush any remaining text
        flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);

        // Fallback: If document didn't match standard headings, split by paragraphs
        if clauses.is_empty() && !raw_text.trim().is_empty() {
            for (idx, para) in raw_text.split("\n\n").enumerate() {
                let trimmed = para.trim();
                if !trimmed.is_empty() {
                    clauses.push(Clause {
                        id: None,
                        doc_id: None,
                        doc_title: doc_title.to_string(),
                        chapter: String::new(),
                        section: String::new(),
                        article: format!("段落 {}", idx + 1),
                        breadcrumb: format!("{} > 段落 {}", doc_title, idx + 1),
                        page_num: current_page,
                        content: trimmed.to_string(),
                        table_data: None,
                    });
                }
            }
        }

        clauses
    }
}
