use pharm_rag::parser::regulatory::RegulatoryParser;
use pharm_rag::storage::Database;

#[test]
fn test_regulatory_ast_parser() {
    let parser = RegulatoryParser::new();
    let doc_title = "自体CAR-T细胞治疗产品药学变更研究指导原则";
    let text = r#"
第一章 概述
本指导原则适用于自体CAR-T细胞治疗产品的药学变更。

第二章 变更分类与评估
第一节 生产工艺与质控
第十条 无菌保证
针对慢病毒载体和终产品的无菌检查，应符合《中国药典》现行版要求。
"#;

    let clauses = parser.parse(doc_title, text);
    assert!(!clauses.is_empty());

    let has_sterile_clause = clauses.iter().any(|c| {
        c.breadcrumb.contains("第一章 概述") || c.breadcrumb.contains("第二章 变更分类与评估")
    });
    assert!(has_sterile_clause, "Should retain hierarchical breadcrumbs");
}

#[test]
fn test_database_fts5_and_fallback() {
    let mut db = Database::open_in_memory().expect("Failed to open memory db");
    let parser = RegulatoryParser::new();
    let sample = r#"
第一章 质量控制
第一条 慢病毒载体检测
慢病毒载体收获液应进行RVV（复制型病毒）检查和滴度测定。

第二条 终产品放行
终产品应完成支原体与无菌检查。
"#;
    let clauses = parser.parse("测试规程", sample);
    db.save_document("测试规程", "sample.md", "hash1", "effective", &clauses).expect("Save doc failed");

    // Test 1: Trigram search (>= 3 chars)
    let res1 = db.search("慢病毒", 5, None).expect("Search failed");
    assert!(!res1.is_empty(), "Trigram should match 慢病毒");
    assert_eq!(res1[0].match_strategy, "FTS5_Trigram");

    // Test 2: Short keyword fallback (< 3 chars)
    let res2 = db.search("滴度", 5, None).expect("Search failed");
    assert!(!res2.is_empty(), "Fallback should match 2-char query 滴度");
    assert_eq!(res2[0].match_strategy, "Substring_LIKE_Fallback");
}

#[test]
fn test_status_detection() {
    assert_eq!(RegulatoryParser::detect_status("慢病毒载体RCL检测问题与解答（征求意见稿）", ""), "draft");
    assert_eq!(RegulatoryParser::detect_status("细胞治疗药品药学变更研究技术指导原则（试行）", ""), "trial");
    assert_eq!(RegulatoryParser::detect_status("中国药典四部通则", ""), "effective");
}

#[test]
fn test_deduplication() {
    let mut db = Database::open_in_memory().expect("Failed to open memory db");
    let parser = RegulatoryParser::new();
    let clauses = parser.parse("法规A", "第一章 质量\n第一条 保证");

    // First save: should succeed
    let outcome1 = db.save_document("法规A", "path/to/a.pdf", "sha256_identical", "effective", &clauses).unwrap();
    assert!(matches!(outcome1, pharm_rag::storage::SaveOutcome::Saved { .. }));

    // Second save with different name and path, but identical hash: should be skipped!
    let outcome2 = db.save_document("法规A2026改名", "path/to/a_renamed.pdf", "sha256_identical", "effective", &clauses).unwrap();
    assert!(matches!(outcome2, pharm_rag::storage::SaveOutcome::DuplicateSkipped { .. }));
}
