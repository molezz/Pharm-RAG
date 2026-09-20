# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.2.1] - 2026-09-20

### Fixed
- **Watcher 误判去重缺陷修复**：修复 `pharm-rag watch` 目录监听录入时由于使用固定占位哈希字符串，导致除首个文件外的不同法规文件被错误识别为重复件（False Duplicate Skipping）而静默跳过的问题。统一接入物理文件真实 SHA-256 计算，并在检测到重复时输出明确日志。
- **Linux C23 符号链接兼容性修复**：增加内置 `build.rs` 与 `c_src/c23_compat.c`，自动解决低版本 glibc (< 2.38，如 Ubuntu 20.04/22.04) 编译时 `ort-sys` 缺失 `__isoc23_strtoull` 导致的链接失败问题，实现开箱即用无障碍克隆编译。
- **SQLite LIKE 通配符转义安全防护**：对短关键词回退检索参数转义 `%`、`_`、`\` 并声明 `ESCAPE '\\'`，防止通配符注入造成结果溢出。
- **公共工具函数解耦**：将 `compute_file_hash` 提升至 `pharm_rag::storage` 公共模块，消除代码冗余并确保 CLI 与后台监听服务的一致性。

### Security & Performance
- **HTTP 服务安全绑定与 Bearer 鉴权**：`serve` 服务默认绑定 `127.0.0.1` 保护企业内网数据，新增 `--api-key` / `PHARM_RAG_API_KEY` 访问令牌鉴权机制。
- **异步非阻塞查询保护**：将后台 HTTP 与 MCP 的 SQLite 查询统一迁移至 `tokio::task::spawn_blocking`，防止并发查询阻塞异步执行器工作线程。

### Changed
- **项目定位与文风中立化**：精简标题为中立扁平风格（*Pharmaceutical Regulatory and GxP SOP Retrieval Engine / 医药行业药监法规与 GxP 规程检索工具*），并将适用范围精准归纳为涵盖化学药、中药与生物制品的“医药行业”。
- **AI Agent 接入指引全面升级**：README 补充完整的 Agent-Ready Protocol，包含 CLI `--json` 输出结构规范、REST API 认证示例及标准 Agent System Prompt 模板。

### Added
- **集成测试**：新增 `test_distinct_files_hash_saved_and_compute_file_hash` 与 `test_like_wildcard_escaping` 自动化测试，覆盖文件哈希去重与通配符转义逻辑。

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
