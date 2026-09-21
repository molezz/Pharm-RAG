use regex::Regex;
use super::ast::Clause;

pub struct RegulatoryParser {
    re_chapter: Regex,
    re_section: Regex,
    re_article: Regex,
    re_noise: Regex,
    re_toc_dots: Regex,
    re_toc_dots_page: Regex,
    re_toc_entry: Regex,
    re_toc_title: Regex,
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

            // 过滤页眉页脚固定文本噪点（注意：页码如 1 / 49 交由 re_page 处理，不在此丢弃）
            re_noise: Regex::new(r"(?i)^(FDA CBER OTP Town Hall Series|Contains Nonbinding Recommendations|Guidance for Industry|Food and Drug Administration|April 25, 2023|June 8, 2023)$").unwrap(),

            // 目录过滤特征：真正的点导线（连续4个及以上的半角点、全角点 U+FF0E、下划线、中间点，或3个以上的省略号字符）
            re_toc_dots: Regex::new(r"(\.{4,}|．{4,}|…{3,}|(?:\.\s*){4,}|(?:．\s*){4,}|·{4,}|_{4,})").unwrap(),
            // 点导线结尾接页码（至少3个导线点紧跟页码，排除小数如 0.5）
            re_toc_dots_page: Regex::new(r"(\.{3,}|．{3,}|…{2,}|(?:\.\s*){3,}|(?:．\s*){3,}|·{3,})\s*(\d+|[ivxldcm]+)\s*$").unwrap(),
            // 目录条目特征：标题紧接点导线与页码（必须包含真正的导线点，裸尾部数字不算证据）
            re_toc_entry: Regex::new(r"(?is)^(第[一二三四五六七八九十百]+[章节条]|[一二三四五六七八九十]+、|Q\d+[\.:\s]|Question\s*\d+|[IVXLCDM]+\.\s+|[A-Z]\.\s+|\d+[\.、]).+?(\.{3,}|．{3,}|…{2,}|(?:\.\s*){3,}|(?:．\s*){3,}|·{3,})\s*(\d+|[ivxldcm]+)\s*$").unwrap(),
            // 目录标题标记
            re_toc_title: Regex::new(r"(?i)^(-{3,}\s*)?(目\s*录|table of contents|contents)\s*(-{3,})?$").unwrap(),
        }
    }

    /// Check if a text chunk is a Table of Contents entry or catalogue title
    pub fn is_toc(&self, text: &str) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        if self.re_toc_title.is_match(trimmed) {
            return true;
        }
        if let Some(last_line) = trimmed.lines().last() {
            if self.re_toc_title.is_match(last_line.trim()) {
                return true;
            }
        }
        if self.re_toc_dots.is_match(trimmed) || self.re_toc_dots_page.is_match(trimmed) {
            return true;
        }
        if trimmed.len() < 300 && self.re_toc_entry.is_match(trimmed) {
            return true;
        }
        false
    }

    /// Clean trailing dot leaders and page numbers from heading titles
    pub fn clean_heading(text: &str) -> String {
        let re_trailing = Regex::new(r"([\.·…_]{2,}|(?:\.\s*){2,})\s*(\d+|[ivxldcm]+)?\s*$").unwrap();
        re_trailing.replace(text.trim(), "").trim().to_string()
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
        let mut clause_page: Option<i32> = None;
        let mut in_toc = false;

        let re_page = Regex::new(r"(?i)\[Page\s*(\d+)\]|---\s*第\s*(\d+)\s*页\s*---|第\s*(\d+)\s*页(?:\s*[/共]|\s*$)|(?:^|\b)Page\s+(\d+)\b|^\s*(\d{1,4})\s*/\s*\d+\s*$|(?m)^\s*(\d{1,4})\s*$").unwrap();

        let flush_clause = |clauses: &mut Vec<Clause>,
                            chapter: &str,
                            section: &str,
                            article: &str,
                            lines: &mut Vec<String>,
                            page: Option<i32>| {
            let content = lines.join("\n").trim().to_string();
            if !content.is_empty() {
                // Secondary safeguard: Discard if it is an empty header duplicate with no body content
                let is_empty_header_dup = lines.len() <= 2 && content.chars().count() < 120 && (content == article || content == chapter);

                if !is_empty_header_dup {
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
                } else {
                    tracing::debug!("Document '{}': discarding empty header duplicate: {}", doc_title, content);
                }
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

            // Check for page marker (must be checked before TOC checks so page numbers update accurately)
            if let Some(caps) = re_page.captures(trimmed) {
                let page_match = caps.get(1)
                    .or_else(|| caps.get(2))
                    .or_else(|| caps.get(3))
                    .or_else(|| caps.get(4))
                    .or_else(|| caps.get(5))
                    .or_else(|| caps.get(6));
                if let Some(p) = page_match {
                    if let Ok(num) = p.as_str().parse::<i32>() {
                        current_page = Some(num);
                        continue;
                    }
                }
            }

            // Check if this line starts a Table of Contents block
            if self.re_toc_title.is_match(trimmed) {
                if current_chapter.is_empty() && current_article.is_empty() {
                    current_lines.clear();
                } else {
                    flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, current_page);
                }
                current_chapter.clear();
                current_section.clear();
                current_article.clear();
                in_toc = true;
                tracing::info!("Document '{}': entering Table of Contents block", doc_title);
                continue;
            }

            // If inside TOC block, determine if document body has started
            if in_toc {
                if self.is_toc(trimmed) {
                    tracing::debug!("Document '{}': filtered TOC line: {}", doc_title, trimmed);
                    continue;
                }

                // Check for start of substantive document body or clean chapter/section heading without dots
                let is_clean_chapter = (self.re_chapter.is_match(trimmed) || self.re_section.is_match(trimmed)) && !self.re_toc_dots.is_match(trimmed) && !self.re_toc_dots_page.is_match(trimmed);
                let is_substantive = (trimmed.len() > 30 && !trimmed.ends_with("...") && (trimmed.ends_with('.') || trimmed.ends_with('。') || trimmed.ends_with(';') || trimmed.ends_with('；'))) || is_clean_chapter;

                if is_substantive {
                    in_toc = false;
                    tracing::info!("Document '{}': TOC block ended, document body started at: {}", doc_title, trimmed);
                } else {
                    // Still part of TOC preamble/title lines inside TOC
                    tracing::debug!("Document '{}': skipped TOC preamble line: {}", doc_title, trimmed);
                    continue;
                }
            }

            // Check Chapter
            if let Some(caps) = self.re_chapter.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, clause_page.or(current_page));
                let raw_ch = caps.get(0).map_or("", |m| m.as_str());
                current_chapter = Self::clean_heading(raw_ch);
                current_section.clear();
                current_article.clear();
                clause_page = current_page;
                continue;
            }

            // Check Section
            if let Some(caps) = self.re_section.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, clause_page.or(current_page));
                let raw_sec = caps.get(0).map_or("", |m| m.as_str());
                current_section = Self::clean_heading(raw_sec);
                current_article.clear();
                clause_page = current_page;
                continue;
            }

            // Check Article / FAQ Question
            if let Some(caps) = self.re_article.captures(trimmed) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, clause_page.or(current_page));
                let raw_art = caps.get(0).map_or("", |m| m.as_str());
                current_article = Self::clean_heading(raw_art);
                current_lines.push(current_article.clone());
                clause_page = current_page;
                continue;
            }

            // Town hall / transcript special: Italicized or bold questions ending with ?
            if trimmed.ends_with('?') && (trimmed.starts_with("What") || trimmed.starts_with("How") || trimmed.starts_with("Can") || trimmed.starts_with("Is") || trimmed.starts_with("Could")) {
                flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, clause_page.or(current_page));
                current_article = Self::clean_heading(trimmed);
                current_lines.push(current_article.clone());
                clause_page = current_page;
                continue;
            }

            // Regular body text
            if clause_page.is_none() {
                clause_page = current_page;
            }
            current_lines.push(trimmed.to_string());
        }

        // Flush any remaining text
        flush_clause(&mut clauses, &current_chapter, &current_section, &current_article, &mut current_lines, clause_page.or(current_page));

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

        // Post-filter: Guarantee that any residual TOC / catalogue entries are excluded at ingest time
        clauses.retain(|c| {
            let is_toc_content = self.is_toc(&c.content);
            let is_toc_article = self.is_toc(&c.article);
            let is_toc_chapter = self.is_toc(&c.chapter);
            !(is_toc_content || is_toc_article || is_toc_chapter)
        });

        clauses
    }
}
