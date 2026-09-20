use regex::Regex;
use super::ast::Clause;

pub struct RegulatoryParser {
    re_chapter: Regex,
    re_section: Regex,
    re_article: Regex,
    re_noise: Regex,
}

impl Default for RegulatoryParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RegulatoryParser {
    pub fn new() -> Self {
        Self {
            // 中文：第X章 或 一、概况
            // 英文：CHAPTER I, SECTION II, I. INTRODUCTION, II. BACKGROUND
            re_chapter: Regex::new(r"(?i)^(第[一二三四五六七八九十百]+章|[一二三四五六七八九十]+、|(CHAPTER|SECTION)\s+[IVXLCDM\d]+|[IVXLCDM]+\.\s+([A-Z\s]{3,}))\s*(.*)$").unwrap(),
            
            // 中文：第X节 或 （一）生产与质控
            // 英文：A. Personnel, B. QC Function
            re_section: Regex::new(r"^(第[一二三四五六七八九十]+节|（[一二三四五六七八九十]+）|\([一二三四五六七八九十]+\)|[A-Z]\.\s+[A-Za-z].*)$").unwrap(),
            
            // 中文：第X条 或 1. 原材料控制
            // 英文：Q1., Q2., Question 1., 1. Testing, 2. Stability
            re_article: Regex::new(r"(?i)^(第[一二三四五六七八九十百]+条|\d+[\.、]|Q\d+[\.:\s]|Question\s*\d+[\.:\s])\s*(.*)$").unwrap(),

            // 过滤页眉页脚噪点
            re_noise: Regex::new(r"(?i)^(FDA CBER OTP Town Hall Series|Contains Nonbinding Recommendations|Guidance for Industry|Food and Drug Administration|\d+\s*/\s*\d+|April 25, 2023|June 8, 2023)$").unwrap(),
        }
    }

    /// Automatically determine regulatory lifecycle status
    pub fn detect_status(doc_title: &str, raw_text: &str) -> String {
        let lower_title = doc_title.to_lowercase();
        let sample_str: String = raw_text.chars().take(800).collect();
        let sample = sample_str.to_lowercase();

        if lower_title.contains("征求意见稿") || sample.contains("征求意见稿") || lower_title.contains("draft") || sample.contains("draft guidance") {
            "draft".to_string()
        } else if lower_title.contains("试行") || sample.contains("（试行）") || sample.contains("(试行)") || lower_title.contains("interim") || lower_title.contains("trial") {
            "trial".to_string()
        } else {
            "effective".to_string()
        }
    }

    /// Parse document text into clauses with breadcrumb hierarchy
    pub fn parse(&self, doc_title: &str, raw_text: &str) -> Vec<Clause> {
        let status = Self::detect_status(doc_title, raw_text);
        let mut clauses = Vec::new();
        let mut current_chapter = String::new();
        let mut current_section = String::new();
        let mut current_article = String::new();
        let mut current_lines = Vec::new();
        let mut current_page: Option<i32> = None;

        let re_page = Regex::new(r"\[Page\s*(\d+)\]|---\s*第\s*(\d+)\s*页\s*---|(?m)^\s*(\d+)\s*$").unwrap();

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
                    status: status.clone(),
                });
            }
            lines.clear();
        };

        for line in raw_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Filter repeating header/footer noise
            if self.re_noise.is_match(trimmed) {
                continue;
            }

            // Check for page marker
            if let Some(caps) = re_page.captures(trimmed) {
                if let Some(p) = caps.get(1).or_else(|| caps.get(2)) {
                    if let Ok(num) = p.as_str().parse::<i32>() {
                        current_page = Some(num);
                        continue;
                    }
                }
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

            // Check Article / FAQ Question
            if let Some(caps) = self.re_article.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                current_article = caps.get(0).map_or("", |m| m.as_str()).to_string();
                current_lines.push(trimmed.to_string());
                continue;
            }

            // Town hall / transcript special: Italicized or bold questions ending with ?
            if trimmed.ends_with('?') && (trimmed.starts_with("What") || trimmed.starts_with("How") || trimmed.starts_with("Can") || trimmed.starts_with("Is") || trimmed.starts_with("Could")) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                current_article = trimmed.to_string();
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
                        status: status.clone(),
                    });
                }
            }
        }

        clauses
    }
}
