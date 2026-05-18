<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.zh_TW.md">繁體中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.pt.md">Português</a>
</p>

# fi-code

一個基於 Rust 構建的終端 AI Coding Agent，透過 REPL、TUI、HTTP Server 或 Desktop 與使用者互動，支援多輪對話、工具呼叫、會話持久化以及 MCP 協議擴展。

## 特性

- **🤖 多模型支援**：統一封裝 OpenAI 相容介面與 Anthropic 介面，支援流式 SSE 響應。內建重試機制（指數退避 + Full Jitter）。
- **🔧 工具呼叫**：內建 20 個工具，包括 `bash`、`read`、`write`、`edit`、`web_fetch`、`grep`、`glob` 以及完整的 Git 工具套件。Agent 可根據模型返回自動執行並回傳結果。
- **💬 會話持久化**：採用 JSON Lines 格式將會話增量寫入本地磁碟，支援中斷後恢復。
- **🖥️ 多模式互動**：
  - **CLI REPL**：傳統命令列互動模式（`fi-code-cli -i`）
  - **TUI**：基於 `ratatui` 的全終端介面（`fi-code-tui`）
  - **HTTP Server**：REST API + SSE 流式響應（`fi-code-server` 或 `fi-code-cli server`）
  - **Desktop**：基於 Tauri v2 的桌面應用，採用「Tauri 殼 + 嵌入式 Sidecar」架構（`fi-code-desktop`）
- **🛡️ 權限校驗**：對 Bash 等高風險操作進行風險分級（Allow / Ask / Deny），攔截 `sudo`、`rm -rf` 及常見注入攻擊。
- **⚙️ 靈活配置**：支援 `~/.config/fi-code/config.json` 或 `config.jsonc`，支援註釋、環境變數佔位符（`{env:VAR_NAME}`）以及熱重載（500ms 防抖）。
- **🔗 MCP 支援**：完整實現 Model Context Protocol，支援多伺服器管理（stdio / HTTP 傳輸），自動重連（最多 3 次，指數退避）。
- **📦 Skills 系統**：可擴展的 Skill 註冊與載入機制，Agent 可透過 `use_skill` 工具按需載入專案內的 Skill 指令。

## 快速開始

### 環境要求

- [Rust](https://rustup.rs/) 1.70+（推薦最新穩定版）
- Node.js 18+（僅構建 Desktop 前端時需要）
- 對應的 AI Provider API Key

### 安裝

```bash
# 克隆倉庫
git clone <repository-url>
cd fi-code

# 編譯全部二進位檔案
cargo build --release

# 執行（見下方使用說明）
cargo run --bin fi-code-cli -- --help
```

### 配置

#### 方式一：環境變數（最高優先級）

**OpenAI 相容：**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic：**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

其他預設 Provider 也支援環境變數字首：`GLM_*`、`KIMI_*`、`DEEPSEEK_*`、`QWEN_*` / `DASHSCOPE_*`。

#### 方式二：配置檔案

配置檔案路徑（按優先級查詢）：
- Linux/macOS: `~/.config/fi-code/config.jsonc` 或 `~/.config/fi-code/config.json`

範例：
```json
{
  "model": "openai/kimi-k2.5",
  "provider": {
    "openai": {
      "provider_type": "openai_compatible",
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1",
        "timeout": 300000,
        "chunkTimeout": 10000
      },
      "models": {
        "kimi-k2.5": {
          "name": "Kimi K2.5",
          "maxTokens": 128000,
          "modalities": {
            "input": ["text", "image"],
            "output": ["text"]
          }
        }
      }
    }
  },
  "mcp": {
    "filesystem": {
      "type": "local",
      "enabled": true,
      "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/path"]
    }
  },
  "server": {
    "port": 4040,
    "api_token": null,
    "allowed_origins": null
  }
}
```

支援 `//` 和 `/* */` 註釋，`apiKey` 支援 `{env:VAR_NAME}` 佔位符語法。預設 Provider（openai、anthropic、glm、kimi、qwen、deepseek）會自動合併到配置中。

### 使用

```bash
# CLI REPL 互動模式
cargo run --bin fi-code-cli -- -i

# TUI 全終端介面模式
cargo run --bin fi-code-tui

# 執行單條命令並退出
cargo run --bin fi-code-cli -- -c "幫我寫一個 Rust Hello World"

# 查看已配置的 Provider 和模型
cargo run --bin fi-code-cli -- --models

# 查看會話列表
cargo run --bin fi-code-cli -- -s

# 指定工作目錄
cargo run --bin fi-code-cli -- -i -w /path/to/project

# 啟動 HTTP Server
cargo run --bin fi-code-server
# 或
cargo run --bin fi-code-cli -- server --port 4040
```

**注意**：直接執行 `fi-code-cli` 不攜帶任何參數時，將自動啟動 TUI 模式。

### Desktop 開發

Desktop 應用採用 **「Tauri 殼 + 嵌入式 Sidecar」** 架構：

```bash
# 先編譯 Sidecar 二進位檔案
cargo build

# 安裝前端依賴並啟動 Desktop 開發模式
cd frontend && npm install
cargo tauri dev

# 生產構建
cargo tauri build
```

## 專案結構

本專案採用 **Cargo Workspace** 結構：

```
.
├── Cargo.toml              # Workspace 定義
├── crates/
│   ├── core/               # 核心庫（fi-code-core）：所有業務邏輯
│   ├── cli/                # CLI 二進位入口（fi-code-cli）
│   ├── tui/                # TUI 二進位入口（fi-code-tui）
│   ├── server/             # Server 二進位入口（fi-code-server）
│   ├── shared/             # 共享 DTO 與常量（fi-code-shared）
│   └── utils/              # 測試工具庫（fi-code-utils）
├── src-tauri/              # Tauri Desktop 應用（fi-code-desktop）
├── frontend/               # Tauri 前端（React + Vite + Tailwind）
└── tests/                  # E2E 與 BDD 測試（fi-code-tests）
```

## 內建工具

| 工具 | 說明 | 風險等級 |
|------|------|----------|
| `bash` | 執行 shell 命令 | Ask（危險命令 Deny） |
| `read` / `read_file` | 讀取檔案內容 | Allow |
| `write` | 寫入檔案 | Ask |
| `edit` | 編輯檔案 | Ask |
| `web_fetch` | 抓取網頁並轉為 Markdown | Ask |
| `grep` | 正則搜尋檔案內容 | Allow |
| `glob` | 檔案路徑匹配 | Allow |
| `git` | 執行 git 命令 | Ask |
| `git_status` | Git 狀態 | Allow |
| `git_diff` | Git diff | Allow |
| `git_add` | Git add | Ask |
| `git_commit` | Git commit | Ask |
| `git_log` | Git 日誌 | Allow |
| `git_worktree` | Git worktree 操作 | Ask |
| `create_task_plan` | 建立任務計劃 | Ask |
| `handle_task_plan` | 執行任務計劃 | Ask |
| `ask_for_question` | 向使用者提問 | Ask |
| `use_skill` | 載入並使用 Skill | Ask |
| `mcp:*` | MCP 工具（動態載入） | 取決於 MCP 配置 |

## 安全機制

- **路徑逃逸防護**：所有檔案操作透過 `safe_path` 檢查，確保不超出工作目錄。
- **Bash 沙箱**：清除繼承的環境變數，僅保留最小必要變數（`PATH=/usr/bin:/bin`、`HOME`），並設定 120 秒超時。
- **權限分級**：Deny（直接拒絕危險命令）、Ask（互動確認）、Allow（唯讀操作直接放行）。
- **輸出截斷**：工具返回內容限制在 50,000 字元以內，防止撐爆上下文。

## TUI 快捷鍵

在 TUI 模式下，可使用以下快捷鍵：

| 快捷鍵 | 功能 |
|--------|------|
| `Tab` / `Shift+Tab` | 切換焦點區域 |
| `Ctrl+C` | 停止生成 / 退出程式 |
| `Ctrl+B` | 開啟/關閉左側檔案抽屜 |
| `Ctrl+H` | 開啟/關閉右側會話歷史抽屜 |
| `Ctrl+M` | 開啟模型選擇下拉框 |
| `Ctrl+T` | 切換主題 |
| `Ctrl+N` | 新建會話 |
| `Enter` | 傳送訊息 |
| `Shift+Enter` | 輸入框內換行 |
| `Esc` | 關閉抽屜/下拉框/返回主區域 |
| `Ctrl+Up` / `PageUp` | 聊天區向上捲動 |
| `Ctrl+Down` / `PageDown` | 聊天區向下捲動 |

## 開發

```bash
# 執行全部測試（含單元測試）
cargo test

# 執行 E2E 測試
cargo test --test e2e_cli
cargo test --test e2e_tui
cargo test --test tui_flow_e2e

# 執行 BDD 測試
cargo test --test bdd

# 格式化程式碼
cargo fmt

# Clippy 靜態檢查
cargo clippy
```

## 技術棧

| 依賴 | 用途 |
|------|------|
| `tokio` | 非同步執行時 |
| `reqwest` | HTTP 客戶端，SSE 流式請求 |
| `serde` / `serde_json` | 序列化與反序列化 |
| `anyhow` | 錯誤處理 |
| `axum` / `tower-http` | HTTP Server 框架與 CORS |
| `ratatui` / `crossterm` | TUI 渲染與終端事件 |
| `colored` | 終端彩色輸出 |
| `clap` | 命令列參數解析 |
| `notify` | 配置檔案熱重載 |
| `regex` | 正則匹配 |
| `html2md` | 網頁 HTML 轉 Markdown |
| `jsonc-parser` | JSONC 配置檔案解析 |
| `tauri` | 桌面應用框架（v2） |

## 資料儲存

會話資料以 `.jsonl` 格式儲存在平台配置目錄下：
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

日誌檔案持久化到 `~/.config/fi-code/logs/`（由 `LogFileWriter` 非同步寫入）。

## 授權

本專案採用 [MIT License](./LICENSE) 開源授權。

Copyright (c) 2025 fi-code contributors.

---

> **提示**：本專案處於早期開發階段，API 和配置格式可能會發生變化。
