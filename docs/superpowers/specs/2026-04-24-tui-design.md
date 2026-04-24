# TUI 界面设计文档

> 为 ficode 添加基于 ratatui 的终端用户界面，直接运行 `ficode` 时默认启动 TUI 并与后台 Server 通信。

---

## 1. 背景与目标

### 1.1 现状

ficode 目前支持：
- CLI REPL 模式（`ficode -i`）
- 单命令模式（`ficode -c "..."`）
- Web 服务模式（`ficode server`）

但默认运行 `ficode`（无参数）时仅显示错误提示，缺乏友好的终端交互界面。

### 1.2 目标

1. 使用 **ratatui** 构建终端用户界面（TUI）
2. 直接运行 `ficode` 时，**后台自动启动 Server**，TUI 通过 HTTP 与 Server 通信
3. TUI 界面包含：状态栏（含动态进度条）、消息区、底部居中输入框
4. 输入 `/` 时弹出指令下拉框，选择后通过 JSON-RPC 执行
5. 普通文本输入通过 SSE 与 Agent 进行流式对话

---

## 2. 架构设计

### 2.1 模块划分

```
src/tui/
├── mod.rs          # 模块入口，导出 run_tui()
├── app.rs          # TuiApp 状态、事件循环、消息处理
├── ui.rs           # 渲染函数
└── client.rs       # HTTP 客户端封装
```

### 2.2 文件职责

#### `src/tui/app.rs`

`TuiApp` 结构体管理全部 UI 状态：
- `input: String` — 当前输入内容
- `messages: Vec<Message>` — 消息历史
- `current_model: String` — 当前模型名称
- `waiting: bool` — 是否等待响应
- `show_dropdown: bool` — 是否显示指令下拉框
- `dropdown_selected: usize` — 下拉框选中项索引
- `session_id: Option<String>` — 当前会话 ID
- `spinner_frame: usize` — 进度条当前帧

事件循环：
1. `crossterm::event::poll(timeout)` 检测按键
2. 根据按键更新状态
3. `terminal.draw(|f| ui::draw(f, &mut self))` 渲染
4. 如果输入以 `/` 开头，显示下拉框

#### `src/tui/ui.rs`

`draw()` 主渲染函数，将屏幕分为三个区域：
- **状态栏**（顶部 1 行）：`FiCode <spinner> | model: <current_model>`
- **消息区**（中间剩余高度）：滚动显示消息历史
- **输入框**（底部 3 行）：居中输入框 + 指令下拉框

#### `src/tui/client.rs`

`TuiClient` 封装 HTTP 通信：
- `execute(command: &str) -> Result<String>` — JSON-RPC 调用 `/rpc`
- `chat(session_id, message, tx) -> Result<String>` — SSE 连接 `/chat`

### 2.3 修改文件

| 文件 | 变更 |
|------|------|
| `src/entry.rs` | `run()` 中无参数时调用 `run_tui_mode()` |
| `src/utils/cli.rs` | 无参数不再报错，移除 `println!("Please provide an option...")` |
| `Cargo.toml` | 新增 `ratatui`、`crossterm` 依赖 |

---

## 3. TUI 布局与行为

### 3.1 界面布局

```
┌─────────────────────────────────────────────────┐
│ FiCode ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ | model: gpt-4o         │  ← 状态栏
├─────────────────────────────────────────────────┤
│                                                 │
│  🤖 我来帮你写 Rust Hello World                 │  ← 消息区
│                                                 │
│  ✅ 文件已写入 src/main.rs                      │
│                                                 │
│  🤖 还有什么可以帮你的吗？                      │
│                                                 │
├─────────────────────────────────────────────────┤
│  > /model_                                      │  ← 输入框
│    ┌─────────────────────────┐                  │
│    │ /model  —  切换模型     │                  │  ← 指令下拉框
│    │ /init   —  生成AGENTS.md│                  │
│    │ /help   —  显示帮助     │                  │
│    └─────────────────────────┘                  │
└─────────────────────────────────────────────────┘
```

### 3.2 状态栏

- 格式：`FiCode <spinner> | model: <current_model>`
- `<spinner>`：等待响应时显示动态 braille 进度条（`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` 循环）
- 空闲时显示空格或静态分隔符

### 3.3 消息区

- 支持 `↑`/`↓` 或 `PgUp`/`PgDn` 滚动
- 消息类型：
  - 用户消息：`> 用户输入内容`
  - Assistant 消息：`🤖 回复内容`
  - 系统消息：`ℹ️ 指令执行结果`（灰色）
  - 错误消息：`❌ 错误内容`（红色）

### 3.4 输入框

- 底部居中，前缀 `>`
- 高度固定 1-3 行（自动换行）
- 支持：光标左右移动、Home/End、Backspace、Delete、Ctrl+A/E

### 3.5 指令下拉框

- 当输入以 `/` 开头时，在输入框上方弹出
- 显示可用指令列表（从 `commands::slash` 解析器获取）
- 支持 `↑`/`↓` 选择，`Enter` 确认，`Esc` 关闭
- 选择后自动补全到输入框

---

## 4. 客户端-服务器通信

### 4.1 指令通信（JSON-RPC）

**流程：**
```
用户输入 /model gpt-4o
        │
        ▼
TUI 检测以 / 开头 → 识别为指令
        │
        ▼
TuiClient::execute("/model gpt-4o")
POST /rpc
{ "jsonrpc": "2.0", "method": "execute",
  "params": { "command": "/model gpt-4o" }, "id": 1 }
        │
        ▼
Server 处理 → 返回响应
        │
        ▼
TUI 在消息区显示结果：✅ 已切换模型: gpt-4o
```

### 4.2 对话通信（SSE）

**流程：**
```
用户输入 "帮我写Hello World"
        │
        ▼
TUI 检测非 / 开头 → 识别为对话
        │
        ▼
TuiClient::chat(session_id, "帮我写Hello World", tx)
POST /chat
{ "session_id": "...", "message": "帮我写Hello World" }
        │
        ▼
Server 返回 SSE 流
        │
        ▼
TUI 实时解析并显示：
  event: message → 追加文本到消息区
  event: tool_use → 显示工具调用信息
  event: tool_result → 显示工具结果
  event: done → 停止进度条，保存 session_id
```

### 4.3 进度条驱动

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(80));
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    loop {
        interval.tick().await;
        if !waiting.load(Ordering::Relaxed) {
            break;
        }
        frame = (frame + 1) % frames.len();
        let _ = spinner_tx.send(frames[frame]).await;
    }
});
```

---

## 5. 入口点变更

### 5.1 启动流程

```
ficode (无参数)
        │
        ▼
┌─────────────────┐
│ 加载 Config      │
│ 创建 Provider    │
├─────────────────┤
│ 启动 Server      │ → tokio::spawn(server.run())
│ 等待 500ms       │
├─────────────────┤
│ 启动 TUI         │ → tui::run_tui()
│                  │
│ TUI 退出后       │
│ └─ 关闭 Server   │ → handle.abort()
└─────────────────┘
```

### 5.2 `entry::run()` 修改

```rust
pub async fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Commands::Server { port }) => {
            // server 子命令
            let config = Arc::new(RwLock::new(Config::load()?));
            let provider = Arc::new(RwLock::new(Provider::new(Arc::clone(&config))?));
            crate::server::Server::new(provider, config, port).run().await;
            return Ok(());
        }
        None => {
            if args.interactive || args.cmd.is_some() 
                || args.session.is_some() || args.models {
                // 继续原有 CLI 逻辑
            } else {
                // 默认启动 TUI 模式
                return run_tui_mode().await;
            }
        }
    }
    
    // 原有 CLI 逻辑...
}

async fn run_tui_mode() -> Result<()> {
    let config = Arc::new(RwLock::new(Config::load()?));
    let provider = Arc::new(RwLock::new(Provider::new(Arc::clone(&config))?));
    
    let server = crate::server::Server::new(
        Arc::clone(&provider),
        Arc::clone(&config),
        None,
    );
    let server_handle = tokio::spawn(async move {
        server.run().await;
    });
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let result = crate::tui::run_tui().await;
    server_handle.abort();
    
    result
}
```

### 5.3 向后兼容

- `ficode -i`、`-c`、`-s`、`-m` 完全保留原有行为
- `ficode server` 子命令不受影响
- 仅**无参数且无 flag**时行为变更（从报错改为启动 TUI）

---

## 6. 实现顺序

1. **修改 `Cargo.toml`** — 添加 `ratatui`、`crossterm`
2. **修改 `src/utils/cli.rs`** — 移除无参数错误提示
3. **创建 `src/tui/client.rs`** — HTTP 客户端（JSON-RPC + SSE）
4. **创建 `src/tui/app.rs`** — TuiApp 状态和事件循环
5. **创建 `src/tui/ui.rs`** — 渲染函数
6. **创建 `src/tui/mod.rs`** — 模块入口 `run_tui()`
7. **修改 `src/entry.rs`** — `run_tui_mode()` 和入口分支
8. **运行测试** — `cargo test`、`cargo clippy`

---

## 7. 依赖

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
```

---

## 8. 兼容性说明

- **CLI 行为变更**：无参数时从报错改为启动 TUI，这是唯一的破坏性变更
- **Server 模块不受影响**：TUI 只是新增客户端
- **配置文件不受影响**：无需新增配置项
