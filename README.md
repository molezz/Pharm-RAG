# PharmRAG 🧬⚖️

> **Pharmaceutical Regulatory and GxP SOP Retrieval Engine**  
> 医药行业药监法规与 GxP 规程检索工具。

[![CI](https://github.com/molezz/PharmRAG/actions/workflows/ci.yml/badge.svg)](https://github.com/molezz/PharmRAG/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![SQLite](https://img.shields.io/badge/sqlite-FTS5%20Trigram-green.svg)](https://www.sqlite.org/fts5.html)

---

## 🌟 为什么需要 PharmRAG？（Why PharmRAG?）

在医药行业药学研究（CMC）与 GMP/GxP 规程检索场景下，通用 RAG 工具通常存在以下挑战：

| 评估维度 | 通用开源 / 商业 RAG | **PharmRAG (本引擎)** |
| :--- | :--- | :--- |
| **切块粒度** | 机械字符硬切（常截断法条） | **法律语法树（AST）切分**（章-节-条-款完整闭合） |
| **溯源能力** | 孤立片段，出处模糊 | **挂载大纲层级面包屑 + 原文页码** |
| **法规效力** | 草案与正式版混为一谈 | **四级生命周期管理**（现行/试行/征求意见稿清晰标识，可过滤草案） |
| **文件去重** | 重命名文件重复入库致结果冗余 | **SHA-256 内容指纹去重**（改名不重录） |
| **术语召回** | 分词截断专业缩写（如 CAR-T、RCL） | **FTS5 Trigram 逐字精准匹配**（毫秒级响应，通常 < 5ms） |
| **语义与双语** | 依赖外置向量库 / 仅支持单语 | **内置 BGE-M3 + RRF 混合检索**（中文语义直接检索 FDA 英文指南） |
| **数据安全** | 依赖云端或微服务组件 | **嵌入式 SQLite 单文件本地运行**（内控规程数据完全本地化） |

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
        P4["SHA-256 自动指纹计算 & 内容去重"]
        P5["法规效力状态判定 (Draft/Trial/Effective)"]
    end

    subgraph Storage["3. 本地嵌入式存储 (SQLite FTS5 + 密集向量)"]
        DB[("pharmrag.db (单文件数据库)")]
        FTS["FTS5 Trigram 全文索引 + BM25"]
        VEC["BGE-M3 语义向量表 (ONNX BLOB)"]
        META["法规效力状态 / 篇章结构 / 页码元数据"]
    end

    subgraph Interfaces["4. 交付与 Agent 接入"]
        CLI["CLI 命令行 (精确 / Hybrid 混合检索)"]
        REST["本地 HTTP REST API (毫秒级响应)"]
        MCP["Model Context Protocol (MCP Server)"]
        WATCH["后台文件监听器 (Folder Watcher)"]
    end

    Sources --> Parser
    Parser --> Storage
    Storage --> Interfaces
```

---

## ✨ 核心特性（Features）

- **嵌入式本地运行（无需外置数据库）**：基于 Rust 与 SQLite 编写，开箱即用，无需配置复杂的外部服务或数据库。
- **本地 ONNX 跨语言 Hybrid 混合检索（BGE-M3 + RRF）**：内置多语言模型 BGE-M3，结合 Reciprocal Rank Fusion 算法将 Trigram 精确匹配与语义向量融合，支持中文语义检索 FDA/EMA 英文指南。
- **法规效力全生命周期管理（Regulatory Lifecycle）**：
  - 🟢 **`[现行正式版]`**：药监部门正式发布，法定生效标准；
  - 🟡 **`[试行版]`**：现行有效监管技术指导原则；
  - 🔴 **`[征求意见稿]`**：供审评趋势参考，检索时标注醒目标识，支持 `--only-effective` 过滤；
  - ⚪ **`[已废止/历史版本]`**：提供历史追溯。
- **SHA-256 智能内容去重（Deduplication）**：自动计算文件内容哈希。即使文件名被修改，也能识别为同一文件并跳过重复录入。
- **明确的篇章层级与来源追溯**：每个返回的法规条目均附带大纲层级面包屑和页码，便于核对原文。
- **医药专有名词 Trigram 检索**：适配中英文专有名词（如 CAR-T、AAV、无菌检查、质粒、RCL 等），长短词无缝降级兜底。
- **实时文件夹监控（Folder Watcher）**：新放入的 PDF 或 DOCX 法规文档自动增量切块入库。
- **Agent 原生适配（MCP & REST API）**：内置标准 Model Context Protocol（MCP）Server，可直接作为工具挂载至 Hermes、Antigravity、Claude 等 AI 助手。
- **GitHub Actions 全平台预编译**：支持一键下载 Linux (x86_64/ARM64)、macOS (Apple Silicon/Intel) 及 Windows 原生可执行文件。

---

## 🚀 快速上手（Quick Start）

### 1. 安装方式
 
#### 方式 A：直接下载预编译二进制包（推荐，无需 Rust 环境）
从 [GitHub Releases](https://github.com/molezz/PharmRAG/releases) 页面下载对应操作系统的压缩包，或直接在终端一键安装：

```bash
# Linux x86_64 极速安装：
curl -sL "https://github.com/molezz/PharmRAG/releases/latest/download/pharmrag-linux-x86_64.tar.gz" | tar -xz -C /tmp/ && sudo mv /tmp/pharmrag /usr/local/bin/pharmrag && chmod +x /usr/local/bin/pharmrag

# Linux ARM64 (aarch64) 极速安装（如各类 ARM 云服务器）：
curl -sL "https://github.com/molezz/PharmRAG/releases/latest/download/pharmrag-linux-aarch64.tar.gz" | tar -xz -C /tmp/ && sudo mv /tmp/pharmrag /usr/local/bin/pharmrag && chmod +x /usr/local/bin/pharmrag

# 验证安装
pharmrag --help
```


#### 方式 B：从源码编译（需安装 Rust）
```bash
git clone https://github.com/molezz/PharmRAG.git
cd PharmRAG
cargo build --release
# 二进制文件位于 target/release/pharmrag
# 可安装至全局命令路径：
cp target/release/pharmrag /usr/local/bin/pharmrag
ln -sf /usr/local/bin/pharmrag /usr/local/bin/pharmRAG
```

---

### 2. 命令行常用操作

> 💡 **命令提示**：可执行文件支持 `pharmRAG` 或 `pharmrag`。默认使用当前目录下的 `pharmrag.db`，亦可通过环境变量 `export PHARMRAG_DB="/path/to/pharmrag.db"` 或 `--db` 参数灵活指定数据库路径。

#### 📥 导入单篇或批量导入法规文档（支持自动去重）
```bash
# 导入单篇指导原则（自动识别征求意见稿/试行版/正式版）
pharmRAG ingest ./CDE-体外基因修饰系统药学研究与评价技术指导原则-202205.pdf

# 导入企业内部 SOP 规程 (Word)
pharmRAG ingest ./SOP-QC-2026-无菌检查操作规程.docx

# 批量扫描导入整个法规目录（自动识别重复文件并跳过）
pharmRAG ingest ./cgt_regulations/
```

#### 🧠 生成与更新本地语义向量（BGE-M3 / ONNX Runtime）
```bash
# 自动下载并初始化多语言旗舰模型 BGE-M3，为库内法规条目生成密集向量
pharmRAG embed

# 可选：使用轻量版模型 BGE-Small-ZH（适合极低资源环境）
pharmRAG embed --small

# 可选：指定模型存放目录（默认优先使用 PHARMRAG_MODELS 环境变量，或当前 ./models/，无则回退至标准缓存 ~/.cache/pharmrag/models）
pharmRAG embed --model-dir ./my_models/
```

#### 🔍 法规检索（支持 FTS5 Trigram 与 BGE-M3 混合检索）
```bash
# 1. 默认精确检索（Trigram 匹配医药专有名词）
pharmRAG search "CAR-T 无菌检查"

# 2. 跨语言语义混合检索（RRF 倒数排名融合，支持中文语义检索英文指南）
pharmRAG search "AAV 宿主细胞DNA残留限度" --hybrid --limit 3

# 3. 现行法规模式：仅检索现行正式版与试行版，过滤征求意见稿
pharmRAG search "RCL 检测" --only-effective

# 4. 指定仅检索征求意见稿，了解药监审评最新风向
pharmRAG search "复制型病毒" --status draft

# 5. 设置相关性分数阈值与目录排除开关
# - 混合检索内置相关性截断（FTS 0命中时默认阈值为 0.40），无关查询（如"如何在火星种土豆"）自动返回 []
# - 不传 --min-score 走内置默认阈值 (0.40)；显式传 --min-score 0.0 则关闭阈值过滤
# - 文档入库（ingest）期已默认剔除目录页；--exclude-toc 是检索期的二次兜底过滤
pharmRAG search "药学变更" --hybrid --min-score 0.45 --exclude-toc

# 6. 指定返回最多 3 条，并输出格式化 JSON 供下游代码或 Agent 解析
pharmRAG search "药学变更 控制" --limit 3 --json

```

#### 📊 查看法规库统计（含效力状态与向量覆盖率）
```bash
pharmRAG stats
```
输出示例：
```text
📊 PharmRAG 状态统计
─────────────────────────────
  数据库文件:     pharmrag.db
  已索引文档总数: 8
    ├─ 现行/试行版: 6
    └─ 征求意见稿: 2
  已切分法规条款: 303
  语义向量覆盖率: 303 / 303 (100.0%)
─────────────────────────────
```

#### 👀 开启法规文件夹自动同步监听
```bash
# 将新法规 PDF/DOCX 放入该文件夹，后台自动完成解析入库
pharmRAG watch ./incoming_regulations/
```

#### 🌐 启动本地 HTTP API & MCP Server（支持安全鉴权与本地绑定）
```bash
# 默认仅绑定本地 127.0.0.1，保障企业规程内网安全
pharmRAG serve --host 127.0.0.1 --port 8080

# 可选：开启 Bearer Token 访问控制（亦可通过环境变量 export PHARMRAG_API_KEY=your-secret）
pharmRAG serve --host 127.0.0.1 --port 8080 --api-key "my-secure-token"
```
- **REST 检索接口**：`GET http://127.0.0.1:8080/api/v1/search?q=慢病毒滴度&limit=3`
- **状态统计接口**：`GET http://127.0.0.1:8080/api/v1/stats`
- **MCP 服务端点**：`POST http://127.0.0.1:8080/mcp`（供 AI Agent 调用 `search_regulations` 工具）

---

## 🤖 AI Agent 接入指引 (Agent-Ready Protocol)

PharmRAG 专为 AI Agent（Hermes、Antigravity、Pi、Claude、Cursor 等）设计，支持三种灵活接入方式：

### 方式 1：CLI JSON 模式（推荐用于具身/命令行 Agent）
若 Agent 拥有 Shell/Bash 执行环境（如 Hermes、AGY、Pi），直接调用命令行并追加 `--json` 参数即可获得结构化数据：

```bash
pharmRAG search "无菌检查 规程" --limit 3 --json
```

**返回的 JSON 结构规范：**
```json
[
  {
    "clause": {
      "id": 42,
      "doc_id": 2,
      "doc_title": "体外基因修饰系统药学研究与评价技术指导原则",
      "chapter": "第三章 生产工艺与过程控制",
      "section": "第二节 原液与制剂生产",
      "article": "第十五条",
      "breadcrumb": "体外基因修饰系统药学研究与评价技术指导原则 > 第三章 生产工艺与过程控制 > 第二节 原液与制剂生产 > 第十五条",
      "page_num": 14,
      "content": "终产品无菌检查应符合中国药典现行版通则要求，对于批量较小或具有时效限制的细胞治疗产品...",
      "table_data": null,
      "status": "effective"
    },
    "score": 0.0325,
    "match_strategy": "Hybrid_RRF (FTS5 + BGE-M3)",
    "semantic_score": 0.5842,
    "fts_score": -4.21,
    "rerank_score": null
  }
]
```
> **评分与排序字段口径说明**：
> - `score`（统一排序得分）：
>   - 在纯精确检索下为 SQLite FTS5 BM25 得分；
>   - 在 `--hybrid` 模式下为 **RRF（Reciprocal Rank Fusion 倒数排名融合得分）**（公式：`1/(60 + rank)`），用于跨检索源多模融合的相对优先级排序，**不可直接视为 0~1 的绝对相似度**。
> - `semantic_score`（原始语义余弦相似度）：
>   - 透出 BGE-M3 密集向量与查询文本的真实 Cosine 相似度（取值范围 `0.0 ~ 1.0`）。下游应用或 Agent 判断条款语义相关性置信度时，**应优先以此字段作为绝对阈值参考**（通常建议 `0.40 ~ 0.60`）。
> - `fts_score`（BM25 词频匹配分）：
>   - 当条款命中了全文索引关键字时透出该分值，否则为 `null`。
> - `clause.status`（法规效力状态）：
>   - 包含四种枚举：`effective`（现行）、`trial`（试行）、`draft`（征求意见稿）、`superseded`（已废止）。Agent 可通过 `--only-effective` 在检索时过滤草案。


---

### 方式 2：REST API 模式（适用于 Web 应用与微服务 Agent）
```bash
curl -s "http://127.0.0.1:8080/api/v1/search?q=无菌检查&limit=2" \
  -H "Authorization: Bearer my-secure-token"
```
**HTTP 响应：**
```json
{
  "query": "无菌检查",
  "count": 2,
  "results": [ ... ]
}
```

---

### 方式 3：Model Context Protocol (MCP Server)
PharmRAG 原生支持标准 MCP 协议，暴露 `search_regulations` 工具。

#### 接入 Hermes Agent / Open-WebUI
在 Hermes 的 `hermes_config.json` 中配置：
```json
{
  "mcp_servers": {
    "pharmrag": {
      "url": "http://127.0.0.1:8080/mcp",
      "description": "Pharmaceutical Regulatory and GxP SOP Retrieval Engine"
    }
  }
}
```

#### 接入 Claude Desktop / Antigravity / Cursor
在客户端的 `mcp_settings.json` 中配置：
```json
{
  "mcpServers": {
    "pharmrag": {
      "command": "/usr/local/bin/pharmrag",
      "args": ["serve", "--host", "127.0.0.1", "--port", "8080"]
    }
  }
}
```

---

### 💡 推荐 Agent System Prompt 模板
将以下提示词加入 Agent 的系统指令（System Prompt）中，可引导大模型精确引用条款溯源：

```text
你是一个精通医药 CMC 与 GMP/GxP 合规审评的专业专家助手。
在回答医药法规、CMC 变更、分析方法验证或无菌保证相关问题时，请遵循以下原则：
1. 严谨溯源：引述法条时必须指明完整出处面包屑（breadcrumb）及页码（page_num），不可泛泛而谈。
2. 效力核查：检查检索结果中的 status 字段。若引用了「draft (征求意见稿)」，必须在回答中显式提示用户：“⚠️ 注意：该条款来自征求意见稿，仅反映最新审评趋势，正式 GMP 申报须以现行正式版本为准”。
3. 冲突比对：若出现新旧指导原则并存，优先以现行版本为法定依据。
```

---

## 🗺️ 路线图（Roadmap）

- [x] 基于 Rust + SQLite FTS5 Trigram 的全文检索核心
- [x] 章-节-条-款 面包屑 AST 树状切分器
- [x] 法规生命周期管理（现行/试行/征求意见稿区分与 `--only-effective` 过滤）
- [x] 基于 SHA-256 指纹的文档内容去重
- [x] Word (.docx) XML 原生高速解析
- [x] PDF 文本与分页信息提取
- [x] 文件夹增量自动监控 (`notify`)
- [x] 本地 HTTP REST API 与 Model Context Protocol (MCP) Server
- [x] GitHub Actions 多平台自动化矩阵编译构建与 Release 发布
- [x] 本地 ONNX 嵌入轻量 Hybrid 语义重排支持（BGE-M3 + RRF 融合）
- [x] FDA / EMA 英文指南专有词根与中英双语检索支持
- [ ] 复杂质控表格（Table-aware）Markdown 对齐重构

---

## 📄 开源许可证（License）

本项目基于 [Apache-2.0 License](LICENSE) 协议开源。
欢迎医药领域的研发、CMC、质量管理与合规、注册及 AI 开发者共同贡献与完善！
