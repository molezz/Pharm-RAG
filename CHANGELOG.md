# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.2.3] - 2026-09-21

### Fixed
- **P0: 修复 TOC 过滤导致尾条以数字结尾或含中文省略号条款被误删的缺陷**：
  - 严格限制 TOC 过滤仅在 `in_toc == true` 目录块内生效，移除正文区域的 standalone is_toc 过滤；
  - 修复 `re_toc_dots` 对中文省略号 `……` 的误判，要求至少 4 组点导线；
  - 修复 `re_toc_entry` 对纯尾部数字（如 `0.5`、`0.25`、`pH 6.5`）的误判，强制要求真实点导线；
  - `flush_clause` 移除 `is_toc` 对合法条款的误杀，增加结构化日志记录。
- **P3: 修复 `compute_file_hash` 失败时回退常量导致不可读文件被误判为重复的缺陷**：
  - 将 `compute_file_hash` 签名改为返回 `std::io::Result<String>`，遇到不可读或不存在文件时直接抛出错误并跳过，杜绝固定字符串哈希碰撞。

### Security
- **P1: `serve --host 0.0.0.0` 外网监听强制 API Key 守卫**：当监听非回环地址（如 `0.0.0.0`）且未设置 API Key 时，直接拒绝启动并安全报错退出，杜绝未鉴权接口暴露。
- **P2: Bearer Token 常量时间比较防范计时侧信道**：实现 XOR 折叠常量时间字节比较（`constant_time_eq`），防止短路字符串比较泄露密钥长度与前缀信息。

### Changed & Performance
- **P3: 规范向量检索复杂度说明与 ANN 索引演进路线**：明确 `search_vector` 当前在内存中执行 O(N) 全局余弦相似度精准排序，保证 100% 召回率，并规划未来数据规模超 10 万条款时平滑迁移至 `sqlite-vec` / `usearch`。

---

## [v0.2.2] - 2026-09-20

### Fixed
- **Linux C23 符号链接兼容性修复**：增加内置 `build.rs` 与 `c_src/c23_compat.c`，自动解决低版本 glibc (< 2.38，如 Ubuntu 20.04/22.04) 编译时 `ort-sys` 缺失 `__isoc23_strtoull` 导致的链接失败问题，实现开箱即用无障碍克隆编译。
- **SQLite LIKE 通配符转义安全防护**：对短关键词回退检索参数转义 `%`、`_`、`\` 并声明 `ESCAPE '\\'`，防止通配符注入造成结果溢出。

### Security & Performance
- **HTTP 服务安全绑定与 Bearer 鉴权**：`serve` 服务默认绑定 `127.0.0.1` 保护企业内网数据，新增 `--api-key` / `PHARM_RAG_API_KEY` 访问令牌鉴权机制。
- **异步非阻塞查询保护**：将后台 HTTP 与 MCP 的 SQLite 查询统一迁移至 `tokio::task::spawn_blocking`，防止并发查询阻塞异步执行器工作线程。
- **向量检索防御上限**：`search_vector` 增加空向量拦截与最大查询数安全边界约束。

### Changed
- **AI Agent 接入指引全面升级**：README 补充完整的 Agent-Ready Protocol，包含 CLI `--json` 输出结构规范、REST API 认证示例及标准 Agent System Prompt 模板。
- **文风中立化与指标客观化**：检索性能描述由硬性指标调整为更客观的“毫秒级响应（通常 < 5ms）”，架构描述调整为“嵌入式本地运行（无需外置数据库）”。

### Added
- **集成测试**：新增 `test_like_wildcard_escaping` 单元测试，覆盖通配符转义与精确回退匹配。

---

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
