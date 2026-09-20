# Pharm-RAG 🧬⚖️

> **High-Fidelity Regulatory & SOP Precision Retrieval Engine for Biopharma CMC QA**  
> 专为生物制药（CGT、抗体、疫苗等）与药监法规、GMP/SOP 打造的高保真、零幻觉精确检索增强引擎。

[![CI](https://github.com/your-username/Pharm-RAG/actions/workflows/ci.yml/badge.svg)](https://github.com/your-username/Pharm-RAG/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![SQLite](https://img.shields.io/badge/sqlite-FTS5%20Trigram-green.svg)](https://www.sqlite.org/fts5.html)

---

## 🌟 为什么需要 Pharm-RAG？（Why Pharm-RAG?）

在生物医药质量保证（QA）与药学研究（CMC）审查场景下，通用开源 RAG（如 Dify、RagFlow、AnythingLLM）存在以下致命缺陷：

| 评估维度 | 通用商业 / 开源 RAG | **Pharm-RAG (本引擎)** | 药企审核实际价值 |
| :--- | :--- | :--- | :--- |
| **切块逻辑** | 机械字符切块（如每 500 字一刀，法条经常被拦腰截断） | **按《章-节-条-款-项》法律语法树（AST）切分** | 完整保留整条法规语义与从属约束 |
| **证据追溯** | 返回孤立文本块，常遗漏具体规程出处 | **强挂载面包屑层级（Breadcrumbs）+ 页码** | 明确标明如 `《指南》> 第3章 > 第2节 > 第4条 [P.18]` |
| **医药专有名词召回** | 通用分词器切碎词汇（如 `CAR-T`、`慢病毒`、`支原体`） | **SQLite FTS5 Trigram + 短词降级兜底** | 100% 逐字精确匹配，零检索盲区 |
| **响应速度与资源** | 需常驻庞大 Python 环境与外置向量库（数秒延迟，显存占用大） | **单二进制纯 Rust 编写，< 1ms 响应，内存 < 15MB** | AI Agent 批量调用数十次审查对比时毫无延迟感知 |
| **数据隐私** | 需配置繁琐数据库集群或依赖公有云 | **单文件 SQLite 本地存储（Local-first）** | 敏感未公开 SOP、偏差审计记录绝不外泄 |

---

## 🏗️ 核心架构（Architecture）

```mermaid
flowchart TD
    subgraph Sources["1. 多源规程接入 (Ingestion)"]
        S1["CDE / NMPA 指导原则 (PDF)"]
        S2["FDA / EMA / ICH 指南 (EN PDF)"]
        S3["企业内控 SMP / SOP (Word DOCX)"]
        S4["质控报告 / 校验规程 (Markdown/Text)"]
    end

    subgraph Parser["2. 结构化 AST 语法树切块器"]
        P1["章 - 节 - 条 - 款 树状层级识别"]
        P2["面包屑路径注入 (Breadcrumb Injection)"]
        P3["Word / PDF 结构化提取"]
    end

    subgraph Storage["3. 本地嵌入式存储 (SQLite FTS5)"]
        DB[("pharm.db (单文件数据库)")]
        FTS["FTS5 Trigram 全文索引 + BM25"]
        META["法规版本 / 生效状态 / 页码元数据"]
    end

    subgraph Interfaces["4. 交付与 Agent 接入"]
        CLI["CLI 命令行 (pharm-rag search)"]
        REST["本地极速 HTTP REST API (&lt;1ms)"]
        MCP["Model Context Protocol (MCP Server)"]
        WATCH["后台文件监听器 (Folder Watcher)"]
    end

    Sources --> Parser
    Parser --> Storage
    Storage --> Interfaces
```

---

## ✨ 核心特性（Features）

- **单二进制零依赖（Single Binary）**：基于 Rust 编写，开箱即用，无需安装 Python、Node.js 或 Docker。
- **逐字保真、零幻觉追溯**：每个返回的法规条目都带有完整的大纲层级面包屑和页码，作为法规审计判定的坚实底册。
- **生物医药专有 Trigram 检索**：完美适配中英文专有名词（如 `CAR-T`、`AAV`、`无菌检查`、`质粒`），长短词无缝降级兜底。
- **实时文件夹监控（Folder Watcher）**：一键开启守护，新放入的 PDF 或 DOCX 法规文档自动增量切块入库。
- **Agent 原生适配（MCP & REST API）**：内置标准 Model Context Protocol（MCP）Server，可直接作为工具挂载至 Hermes、Antigravity、Claude Code 等 AI 助手。
- **GitHub Actions 全平台预编译**：支持一键下载 Linux (x86_64/ARM64)、macOS (Apple Silicon/Intel) 及 Windows 原生可执行文件。

---

## 🚀 快速上手（Quick Start）

### 1. 安装方式

#### 方式 A：直接下载预编译二进制包（推荐）
从 [GitHub Releases](https://github.com/your-username/Pharm-RAG/releases) 页面下载对应操作系统的压缩包，解压后即可直接运行：
```bash
# 解压即可使用
tar -zxvf pharm-rag-linux-x86_64.tar.gz
chmod +x pharm-rag
./pharm-rag --help
```

#### 方式 B：从源码编译（需安装 Rust）
```bash
git clone https://github.com/your-username/Pharm-RAG.git
cd Pharm-RAG
cargo build --release
# 二进制文件位于 target/release/pharm-rag
```

---

### 2. 命令行常用操作

#### 📥 导入单篇或批量导入法规文档
```bash
# 导入单篇指导原则
pharm-rag ingest ./体内基因治疗产品药学研究与评价技术指导原则.pdf

# 导入企业内部 SOP 规程 (Word)
pharm-rag ingest ./SOP-QC-2026-无菌检查操作规程.docx

# 批量扫描导入整个法规目录
pharm-rag ingest ./cgt_regulations/
```

#### 🔍 精确检索法条
```bash
# 精确查找相关条款
pharm-rag search "CAR-T 无菌检查"

# 指定返回最多 3 条，并输出 JSON 供下游程序解析
pharm-rag search "药学变更 控制" --limit 3 --json
```

#### 👀 开启法规文件夹自动同步监听
```bash
# 只要将新法规 PDF/DOCX 丢进该文件夹，后台立即自动完成解析入库
pharm-rag watch ./incoming_regulations/
```

#### 🌐 启动本地极速 HTTP API & MCP Server
```bash
pharm-rag serve --port 8080
```
- **REST 检索接口**：`GET http://localhost:8080/api/v1/search?q=慢病毒滴度`
- **状态统计接口**：`GET http://localhost:8080/api/v1/stats`
- **MCP 服务端点**：`POST http://localhost:8080/mcp`（供 AI Agent 调用 `search_regulations` 工具）

---

## 🤖 接入 AI Agent（MCP 配置示例）

在你的 Agent 客户端（例如 Claude Desktop、Hermes 或 Antigravity）的 MCP 配置文件中添加：

```json
{
  "mcpServers": {
    "pharm-rag": {
      "command": "/path/to/pharm-rag",
      "args": ["serve", "--port", "8080"]
    }
  }
}
```
配置完成后，AI Agent 在执行 CMC 方案审查或变更定性时，会自动调用 `search_regulations` 获取 100% 准确的法条原文与条款编号！

---

## 🗺️ 路线图（Roadmap）

- [x] 基于 Rust + SQLite FTS5 Trigram 的高保真检索核心
- [x] 章-节-条-款 面包屑 AST 树状切分器
- [x] Word (.docx) XML 原生高速解析
- [x] PDF 文本与分页信息提取
- [x] 文件夹增量自动监控 (`notify`)
- [x] 本地 HTTP REST API 与 Model Context Protocol (MCP) Server
- [x] GitHub Actions 多平台自动化矩阵编译构建与 Release 发布
- [ ] 复杂质控表格（Table-aware）Markdown 高保真对齐重构
- [ ] FDA / EMA 英文指南专有词根与中英双语检索支持
- [ ] 本地 ONNX 嵌入轻量 Hybrid 语义重排支持

---

## 📄 开源许可证（License）

本项目基于 [Apache-2.0 License](LICENSE) 协议开源。
欢迎生物制药领域的研发、CMC、QA、注册及 AI 开发者共同贡献与完善！
