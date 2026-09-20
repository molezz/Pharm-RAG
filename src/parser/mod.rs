pub mod ast;
pub mod regulatory;
pub mod docx;

use std::fs;
use std::path::Path;
use ast::Clause;
use regulatory::RegulatoryParser;

pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Clause>, Box<dyn std::error::Error>> {
    let p = path.as_ref();
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let doc_title = p.file_stem().and_then(|s| s.to_str()).unwrap_or("文档");

    match ext.as_str() {
        "docx" => docx::parse_docx(p),
        "txt" | "md" | "markdown" => {
            let content = fs::read_to_string(p)?;
            let parser = RegulatoryParser::new();
            Ok(parser.parse(doc_title, &content))
        }
        "pdf" => {
            let content = match pdf_extract::extract_text(p) {
                Ok(text) => text,
                Err(_) => fs::read_to_string(p).unwrap_or_default(),
            };
            let parser = RegulatoryParser::new();
            Ok(parser.parse(doc_title, &content))
        }
        _ => {
            let content = fs::read_to_string(p)?;
            let parser = RegulatoryParser::new();
            Ok(parser.parse(doc_title, &content))
        }
    }
}
