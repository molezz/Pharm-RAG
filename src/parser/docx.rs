use std::fs::File;
use std::io::Read;
use std::path::Path;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use crate::parser::ast::Clause;
use crate::parser::regulatory::RegulatoryParser;

pub fn parse_docx<P: AsRef<Path>>(path: P) -> Result<Vec<Clause>, Box<dyn std::error::Error>> {
    let file = File::open(path.as_ref())?;
    let mut archive = zip::ZipArchive::new(file)?;
    
    let mut doc_xml = String::new();
    let mut entry = archive.by_name("word/document.xml")?;
    entry.read_to_string(&mut doc_xml)?;

    let mut reader = Reader::from_str(&doc_xml);
    reader.config_mut().trim_text(true);

    let mut text_buf = String::new();
    let mut in_t = false;
    let mut current_p = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"w:p" => {
                        current_p.clear();
                    }
                    b"w:t" => {
                        in_t = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    let text = e.unescape()?;
                    current_p.push_str(&text);
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"w:t" => in_t = false,
                    b"w:p" => {
                        if !current_p.trim().is_empty() {
                            text_buf.push_str(current_p.trim());
                            text_buf.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Box::new(e)),
            _ => {}
        }
    }

    let doc_title = path.as_ref()
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名文档");

    let parser = RegulatoryParser::new();
    Ok(parser.parse(doc_title, &text_buf))
}
