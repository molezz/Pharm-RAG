use pharmrag::parser::regulatory::RegulatoryParser;
use pharmrag::storage::Database;

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
    assert!(matches!(outcome1, pharmrag::storage::SaveOutcome::Saved { .. }));

    // Second save with different name and path, but identical hash: should be skipped!
    let outcome2 = db.save_document("法规A2026改名", "path/to/a_renamed.pdf", "sha256_identical", "effective", &clauses).unwrap();
    assert!(matches!(outcome2, pharmrag::storage::SaveOutcome::DuplicateSkipped { .. }));
}

#[test]
fn test_distinct_files_hash_saved_and_compute_file_hash() {
    use std::io::Write;
    use pharmrag::storage::compute_file_hash;

    let dir = std::env::temp_dir().join(format!("test_pharmrag_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let file1 = dir.join("doc1.txt");
    let file2 = dir.join("doc2.txt");

    std::fs::File::create(&file1).unwrap().write_all(b"regulation A content").unwrap();
    std::fs::File::create(&file2).unwrap().write_all(b"regulation B completely different content").unwrap();

    let hash1 = compute_file_hash(&file1).unwrap();
    let hash2 = compute_file_hash(&file2).unwrap();

    assert_ne!(hash1, hash2);

    let mut db = Database::open_in_memory().unwrap();
    let parser = RegulatoryParser::new();
    let clauses = parser.parse("法规", "第一章 质量\n第一条 保证");

    let res1 = db.save_document("法规A", file1.to_str().unwrap(), &hash1, "effective", &clauses).unwrap();
    let res2 = db.save_document("法规B", file2.to_str().unwrap(), &hash2, "effective", &clauses).unwrap();

    // Both distinct files must be successfully saved
    assert!(matches!(res1, pharmrag::storage::SaveOutcome::Saved { .. }));
    assert!(matches!(res2, pharmrag::storage::SaveOutcome::Saved { .. }));
}

#[test]
fn test_like_wildcard_escaping() {
    use pharmrag::storage::escape_like;

    assert_eq!(escape_like("normal text"), "normal text");
    assert_eq!(escape_like("100% pure"), "100\\% pure");
    assert_eq!(escape_like("sample_name"), "sample\\_name");
    assert_eq!(escape_like("c:\\path"), "c:\\\\path");

    let mut db = Database::open_in_memory().unwrap();
    let parser = RegulatoryParser::new();
    let clauses = parser.parse("法规", "纯度必须达到99.9%以上，且无残留_物质。");
    db.save_document("指标", "/path/test.txt", "hash_like", "effective", &clauses).unwrap();

    // Searching "%" directly should match literal "%" via LIKE fallback
    let res_pct = db.search("%", 10, None).unwrap();
    assert_eq!(res_pct.len(), 1);

    // Searching "_" directly should match literal "_" via LIKE fallback
    let res_underscore = db.search("_", 10, None).unwrap();
    assert_eq!(res_underscore.len(), 1);

    // Searching something not present with wildcard shouldn't match everything
    let res_none = db.search("!%", 10, None).unwrap();
    assert_eq!(res_none.len(), 0);
}

#[test]
fn test_toc_filtering() {
    let parser = RegulatoryParser::new();

    // Chinese document with TOC
    let zh_text = r#"
CAR-T 2023 11
目录
一、前言 .......................................................................................................... 4
二、常见问题与技术要求 .......................................................................... 4
1、药学变更一般原则 .......................................................................... 4
2、变更研究的特殊性 .......................................................................... 6
3、变更可比性研究方案 ...................................................................... 8
4、生产场地、生产线（生产模块）、生产频次的变更（工艺不变）
............................................................................................................. 10
5、生产场地、生产线的变更（工艺改变） ..................................... 12

一、前言
近年来，中国自体CAR-T细胞治疗产品研发迅速。本指导原则旨在规范药学变更研究。

二、常见问题与技术要求
1、药学变更一般原则
药学变更研究应遵循基于风险的原则。在产品生命周期中，应建立有效的产品质量控制策略。
"#;

    let zh_clauses = parser.parse("自体CAR-T药学变更", zh_text);
    // TOC lines should be filtered out; only real clauses should be present
    assert_eq!(zh_clauses.len(), 2, "Should only have 2 real clauses, not TOC entries");
    assert_eq!(zh_clauses[0].chapter, "一、前言");
    assert!(zh_clauses[0].content.contains("中国自体CAR-T细胞治疗产品"));
    assert_eq!(zh_clauses[1].chapter, "二、常见问题与技术要求");
    assert_eq!(zh_clauses[1].article, "1、药学变更一般原则");
    assert_eq!(zh_clauses[1].breadcrumb, "自体CAR-T药学变更 > 二、常见问题与技术要求 > 1、药学变更一般原则");

    // English document with TOC
    let en_text = r#"
Table of Contents
I.  INTRODUCTION ......................................................................................... 1
II.  BACKGROUND ........................................................................................... 1
III.  SUBMISSIONS AND FDA REVIEW ............................................................. 2
A.  IND Submission and Quality ............................................................................... 2
Q1.  What should sponsors know about electronic submission of an Investigational
New Drug application? ..................................................................................... 2
Q2.  What is important for inclusion in an original IND submission? ..................... 4
Q3.  What regulatory forms are included in original INDs and IND amendments?  8

I.  INTRODUCTION
This guidance is intended to provide industry with answers to frequently asked questions.

III.  SUBMISSIONS AND FDA REVIEW
A.  IND Submission and Quality
Q1.  What should sponsors know about electronic submission of an Investigational New Drug application?
Commercial INDs must be submitted consistent with the eCTD requirements.
"#;

    let en_clauses = parser.parse("FDA-FAQs", en_text);
    assert_eq!(en_clauses.len(), 2, "Should only parse real substantive clauses");
    assert_eq!(en_clauses[0].chapter, "I.  INTRODUCTION");
    assert_eq!(en_clauses[1].chapter, "III.  SUBMISSIONS AND FDA REVIEW");
    assert_eq!(en_clauses[1].section, "A.  IND Submission and Quality");
    assert_eq!(en_clauses[1].article, "Q1.  What should sponsors know about electronic submission of an Investigational New Drug application?");
    assert!(!en_clauses[1].breadcrumb.contains("..."), "Breadcrumb must not contain dot leaders");
}

#[test]
fn test_tail_clause_ending_with_number_and_ellipsis_preserved() {
    let parser = RegulatoryParser::new();

    // Document where the final clause ends with numbers (e.g. 0.5, 0.25, 6.5)
    // and clauses contain Chinese ellipsis ……
    let text = r#"
第一章 质量要求
第一条 蛋白质纯度
终产品中宿主蛋白质残留量应低于 0.5

第二条 溶剂残留
有机溶剂残留检测应符合规定，如乙醇残留量应控制在 0.25

第三条 综合质控指标
结合生产工艺参数优化情况……终产品的微粒数应小于 6000 粒，且pH值应维持在 6.5
"#;

    let clauses = parser.parse("质量控制标准", text);
    assert_eq!(clauses.len(), 3, "All 3 clauses must be preserved, none dropped by false-positive TOC filters");
    assert!(clauses[0].content.ends_with("0.5"));
    assert!(clauses[1].content.ends_with("0.25"));
    assert!(clauses[2].content.contains("……"));
    assert!(clauses[2].content.ends_with("6.5"));
}

#[test]
fn test_compute_file_hash_error_on_missing_file() {
    use pharmrag::storage::compute_file_hash;
    let nonexistent = std::path::Path::new("/path/to/definitely/nonexistent_file_12345.xyz");
    let res = compute_file_hash(nonexistent);
    assert!(res.is_err(), "compute_file_hash must return Err for nonexistent file instead of a constant fallback string");
}



