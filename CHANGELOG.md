# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.2.1] - 2026-09-20

### Fixed
- **Watcher 误判去重缺陷修复**：修复 `pharm-rag watch` 目录监听录入时由于使用固定占位哈希字符串，导致除首个文件外的不同法规文件被错误识别为重复件（False Duplicate Skipping）而静默跳过的问题。统一接入物理文件真实 SHA-256 计算，并在检测到重复时输出明确日志。
- **公共工具函数解耦**：将 `compute_file_hash` 提升至 `pharm_rag::storage` 公共模块，消除代码冗余并确保 CLI 与后台监听服务的一致性。

### Changed
- **项目定位与文风中立化**：精简标题为中立扁平风格（*Pharmaceutical Regulatory and GxP SOP Retrieval Engine / 医药行业药监法规与 GxP 规程检索工具*），并将适用范围精准归纳为涵盖化学药、中药与生物制品的“医药行业”。

### Added
- **集成测试**：新增 `test_distinct_files_hash_saved_and_compute_file_hash` 自动化测试，覆盖不同文件哈希校验与去重逻辑。

---

## [v0.2.0] - 2026-09-20

### Added
- **BGE-M3 本地 ONNX 语义嵌入引擎**：集成多语言模型，支持纯 Rust 单二进制离线计算 1024 维向量。
- **中英双语 Hybrid 混合检索**：基于 Reciprocal Rank Fusion (RRF) 算法将 SQLite FTS5 Trigram 与密集向量搜索融合，支持中文语义直接检索英文法规。
- **法规生命周期管理**：新增现行正式版、试行版、征求意见稿和已废止的效力标注，支持 `--only-effective` 过滤。
- **SHA-256 内容指纹去重**：防止同内容文件改名重复入库。
- **Hermes Agent 原生支持**：完善 Model Context Protocol (MCP) Server 端点。

---

## [v0.1.0] - 2026-09-20

### Added
- 初始版本发布：法规 AST 切块器、SQLite FTS5 Trigram 全文检索与本地 HTTP/MCP 服务。
