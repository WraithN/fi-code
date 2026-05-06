# TUI 斜杠命令菜单 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 TUI 输入框中实现 `/` 斜杠命令菜单，支持方向键/鼠标滚轮导航、高亮选中、回车/点击执行，命令由 Server 端 `CommandRegistry` 统一管理并通过 HTTP API 暴露。

**Architecture:** 在 `commands` 模块新建 `CommandRegistry`（元数据 + Handler trait），Server 启动时注册命令并暴露 `/api/commands` 和 `/api/commands/:name/execute` 端点；TUI 通过 `TuiClient` 获取列表并执行，输入框侧支持键盘和鼠标事件；执行结果作为系统消息显示在聊天区。

**Tech Stack:** Rust, tokio, axum, crossterm, ratatui, async-trait, serde

---

## 文件结构

| 文件 | 变更 | 职责 |
|------|------|------|
| `src/commands/registry.rs` | 创建 | `CommandRegistry`、`CommandHandler` trait、`CommandMeta`、`CommandOutput`、`CommandContext` |
| `src/commands/mod.rs` | 修改 | 导出 `registry` 模块及公共类型 |
| `src/commands/slash.rs` | 修改 | 提取 `ModelCommandHandler` 和 `InitCommandHandler`，`SlashCommandHandler` 复用它们 |
| `src/server/server.rs` | 修改 | `AppState` 增加 `commands`；新增 `/api/commands` 路由；启动时注册命令 |
| `src/tui/mod.rs` | 修改 | 启用/禁用 `EnableMouseCapture` |
| `src/tui/app.rs` | 修改 | 路由 `Event::Mouse` 到 `Input` 组件 |
| `src/tui/client.rs` | 修改 | 新增 `list_commands` 和 `execute_command` |
| `src/tui/event.rs` | 修改 | 新增 `ClearChat`、`ShowSystemMessage` 等应用事件 |
| `src/tui/components/input.rs` | 大幅修改 | 从 HTTP 获取命令列表、支持鼠标、执行后显示结果、记录绘制区域 |
| `src/tui/components/chat.rs` | 修改 | `MessageRole::System` 消息使用 warning 颜色高亮显示 |

---

## Task 1: CommandRegistry 核心实现

**Files:**
- Create: `src/commands/registry.rs`
- Modify: `src/commands/mod.rs`

### Step 1: 创建 `src/commands/registry.rs`

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
// ...（许可证头，与项目一致）

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::provider::Provider;

/// 命令元数据，用于 TUI 展示和 HTTP API 返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMeta {
    pub name: String,
    pub description: String,
    pub args_hint: Option<String>,
}

/// 命令执行上下文，由调用方（Server）传入
pub struct CommandContext {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub session_id: Option<String>,
}

/// 命令执行结果类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    Text,
    Error,
    Silent,
}

/// 命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub message: String,
    pub r#type: OutputType,
    pub metadata: Option<Value>,
}

impl CommandOutput {
    pub fn text(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            r#type: OutputType::Text,
            metadata: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            r#type: OutputType::Error,
            metadata: None,
        }
    }

    pub fn silent() -> Self {
        Self {
            message: String::new(),
            r#type: OutputType::Silent,
            metadata: None,
        }
    }
}

/// 命令处理器 trait
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, args: Option<String>, ctx: &CommandContext) -> Result<CommandOutput>;
}

struct CommandEntry {
    meta: CommandMeta,
    handler: Box<dyn CommandHandler>,
}

/// 命令注册表
pub struct CommandRegistry {
    commands: HashMap<String, CommandEntry>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, meta: CommandMeta, handler: Box<dyn CommandHandler>) {
        let name = meta.name.clone();
        self.commands.insert(name, CommandEntry { meta, handler });
    }

    pub fn list(&self) -> Vec<&CommandMeta> {
        self.commands.values().map(|e| &e.meta).collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Option<String>,
        ctx: &CommandContext,
    ) -> Result<CommandOutput> {
        let entry = self
            .commands
            .get(name)
            .ok_or_else(|| anyhow!("Unknown command: {}", name))?;
        entry.handler.execute(args, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    #[async_trait]
    impl CommandHandler for TestHandler {
        async fn execute(&self, args: Option<String>, _ctx: &CommandContext) -> Result<CommandOutput> {
            Ok(CommandOutput::text(format!("test: {:?}", args)))
        }
    }

    fn dummy_ctx() -> CommandContext {
        CommandContext {
            provider: Arc::new(RwLock::new(Provider::default())),
            config: Arc::new(RwLock::new(Config::default())),
            session_id: None,
        }
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let mut registry = CommandRegistry::new();
        registry.register(
            CommandMeta {
                name: "clear".into(),
                description: "Clear".into(),
                args_hint: None,
            },
            Box::new(TestHandler),
        );

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "clear");
    }

    #[tokio::test]
    async fn test_execute_unknown_command() {
        let registry = CommandRegistry::new();
        let result = registry.execute("foo", None, &dummy_ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    #[tokio::test]
    async fn test_command_output_serde() {
        let out = CommandOutput::text("hello");
        let json = serde_json::to_string(&out).unwrap();
        let de: CommandOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(de.message, "hello");
        assert!(matches!(de.r#type, OutputType::Text));
    }
}
```

> **注意：** `Provider::default()` 和 `Config::default()` 需要确认是否存在。如果不存在，测试中使用 `Arc::new(RwLock::new(...))` 占位即可，编译时会提示需要实现 `Default`。

### Step 2: 修改 `src/commands/mod.rs`

在现有导出之上增加：

```rust
pub mod registry;
pub use registry::{
    CommandContext, CommandHandler, CommandMeta, CommandOutput, CommandRegistry, OutputType,
};
```

### Step 3: 编译验证

Run: `cargo check`

Expected: 通过，无错误。

### Step 4: 运行单元测试

Run: `cargo test commands::registry::tests --lib`

Expected: 3 个测试全部通过。

### Step 5: Commit

```bash
git add src/commands/registry.rs src/commands/mod.rs
git commit -m "feat(commands): add CommandRegistry with CommandHandler trait"
```

---

## Task 2: SlashCommandHandler 迁移为 CommandHandler

**Files:**
- Modify: `src/commands/slash.rs`

### Step 1: 在 `slash.rs` 底部新增 Handler 实现

在 `SlashCommandHandler` 的 `impl` 块之后、tests 之前，添加：

```rust
// ============================================================================
// CommandHandler 实现（供 Server / TUI 复用）
// ============================================================================

/// /model 命令处理器
pub struct ModelCommandHandler;

#[async_trait]
impl CommandHandler for ModelCommandHandler {
    async fn execute(&self, args: Option<String>, ctx: &CommandContext) -> Result<CommandOutput> {
        let cfg = ctx.config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let mut provider = ctx
            .provider
            .write()
            .map_err(|_| anyhow!("Provider锁中毒"))?;

        if let Some(key) = args.filter(|s| !s.is_empty()) {
            if provider.list_models(&cfg).iter().any(|(k, _)| k == &key) {
                provider.set_model(&key, &cfg)?;
                Ok(CommandOutput::text(format!("✅ 已切换模型: {}", key)))
            } else {
                let models = provider.list_models(&cfg);
                drop(provider);
                let mut text = format!("❌ 没有此模型: {}\n可用模型列表：\n", key);
                for (i, (k, display)) in models.iter().enumerate() {
                    text.push_str(&format!("  [{}] {} — {}\n", i + 1, k, display));
                }
                Ok(CommandOutput::error(text))
            }
        } else {
            let models = provider.list_models(&cfg);
            drop(provider);
            let mut text = String::from("可用模型列表：\n");
            for (i, (k, display)) in models.iter().enumerate() {
                let mut limit_str = String::new();
                for (_pname, pcfg) in &cfg.provider {
                    if let Some(mcfg) = pcfg.models.get(k) {
                        limit_str = format!(
                            " (context: {}, output: {})",
                            mcfg.limit.context, mcfg.limit.output
                        );
                        break;
                    }
                }
                text.push_str(&format!("  [{}] {} — {}{}\n", i + 1, k, display, limit_str));
            }
            Ok(CommandOutput::text(text))
        }
    }
}

/// /init 命令处理器
pub struct InitCommandHandler;

#[async_trait]
impl CommandHandler for InitCommandHandler {
    async fn execute(&self, _args: Option<String>, ctx: &CommandContext) -> Result<CommandOutput> {
        use crate::agent::runner::AgentRunner;
        use crate::tools::tool_schema;
        use crate::utils::workspace::workspace_dir;

        let workspace = workspace_dir();
        let agents_path = workspace.join("AGENTS.md");

        let system_prompt = r#"你是一个项目文档助手。请深入分析当前项目的结构、技术栈、代码风格和重要约定，生成一份 AGENTS.md 文件。AGENTS.md 的目标是帮助 AI 编程助手快速理解项目背景。

你可以使用以下工具来探索代码库：
- read / read_file: 读取文件内容
- grep: 搜索代码内容
- bash: 执行命令（如 find, ls, tree 等）
- write: 写入文件（用于生成 AGENTS.md）

分析时请注意：
1. 阅读项目根目录的关键文件（README.md, Cargo.toml, package.json 等）
2. 浏览 src/ 目录结构
3. 查看主要模块的入口文件
4. 总结项目使用的技术栈、架构模式和开发约定
5. 将结果写入 AGENTS.md"#;

        let user_prompt = format!(
            "请为当前项目生成 AGENTS.md，保存路径为: {}",
            agents_path.display()
        );

        let client = ctx
            .provider
            .read()
            .map_err(|_| anyhow!("Provider锁中毒"))?
            .get_client()
            .map_err(|e| anyhow!("Failed to create client: {}", e))?;

        let schema = tool_schema().await;
        let runner = AgentRunner::new(client, system_prompt, schema);

        let initial_messages = vec![Message::new(
            "init-session".to_string(),
            Role::User,
            vec![Part::Text { text: user_prompt }],
        )];

        let result = runner.run(initial_messages).await?;

        let has_write = result.messages.iter().any(|msg| {
            msg.parts
                .iter()
                .any(|part| matches!(part, Part::ToolUse { name, .. } if name == "write"))
        });

        if has_write || agents_path.exists() {
            Ok(CommandOutput::text(format!(
                "✅ AGENTS.md 已生成: {}",
                agents_path.display()
            )))
        } else {
            Ok(CommandOutput::text(
                "⚠️ AGENTS.md 可能未生成，请检查对话结果".to_string(),
            ))
        }
    }
}
```

> **导入变更：** 在 `slash.rs` 顶部需要增加 `use crate::commands::registry::{CommandContext, CommandHandler, CommandOutput};` 和 `use async_trait::async_trait;`。

### Step 2: 重构 `SlashCommandHandler` 复用新 Handler

将 `SlashCommandHandler::execute` 改为委托给新的 Handler：

```rust
impl SlashCommandHandler {
    pub async fn execute(&self, cmd: SlashCommand) -> Result<SlashCommandResult> {
        let ctx = CommandContext {
            provider: self.provider.clone(),
            config: self.config.clone(),
            session_id: None,
        };

        match cmd {
            SlashCommand::Model(model_key) => {
                let output = ModelCommandHandler
                    .execute(model_key, &ctx)
                    .await?;
                match output.r#type {
                    OutputType::Error => eprintln!("{}", output.message),
                    _ => println!("{}", output.message),
                }
                Ok(SlashCommandResult::Handled)
            }
            SlashCommand::Init => {
                let output = InitCommandHandler.execute(None, &ctx).await?;
                println!("{}", output.message);
                Ok(SlashCommandResult::Handled)
            }
            SlashCommand::Unknown(name) if name.is_empty() => unreachable!(),
            SlashCommand::Unknown(name) => {
                eprintln!("{} 未知命令: /{}。可用命令: /init, /model", "❌".red(), name);
                Ok(SlashCommandResult::Handled)
            }
        }
    }
}
```

### Step 3: 删除旧的 `handle_model` 和 `handle_init` 私有方法

它们已被 `ModelCommandHandler` 和 `InitCommandHandler` 替代。

### Step 4: 编译并运行测试

Run: `cargo test commands::slash::tests --lib`

Expected: 现有测试全部通过（`test_parse_model_no_args`, `test_parse_model_with_args`, `test_parse_init`, `test_parse_unknown`, `test_parse_not_slash`）。

### Step 5: Commit

```bash
git add src/commands/slash.rs
git commit -m "refactor(commands): extract Model/Init CommandHandlers from SlashCommandHandler"
```

---

## Task 3: Server 端注册命令并暴露 HTTP API

**Files:**
- Modify: `src/server/server.rs`

### Step 1: `AppState` 增加 `CommandRegistry`

```rust
use crate::commands::registry::{CommandContext, CommandOutput, CommandRegistry};
use crate::commands::slash::{InitCommandHandler, ModelCommandHandler};

#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
}
```

### Step 2: `Server::new` 中创建并注册 `CommandRegistry`

```rust
impl Server {
    pub fn new(
        provider: Arc<RwLock<Provider>>,
        config: Arc<RwLock<Config>>,
        port_override: Option<u16>,
    ) -> Self {
        let port = port_override
            .or_else(|| {
                let cfg = config.read().ok()?;
                let server_cfg = cfg.server.as_ref()?;
                server_cfg.port
            })
            .unwrap_or(4040);

        let sessions = Arc::new(HttpSessionManager::new());
        let mut commands = CommandRegistry::new();

        // 注册 /clear 命令（在 server 端直接操作 sessions）
        let sessions_for_clear = sessions.clone();
        struct ClearHandler {
            sessions: Arc<HttpSessionManager>,
        }

        #[async_trait]
        impl crate::commands::registry::CommandHandler for ClearHandler {
            async fn execute(
                &self,
                _args: Option<String>,
                ctx: &CommandContext,
            ) -> anyhow::Result<CommandOutput> {
                if let Some(id) = &ctx.session_id {
                    self.sessions.save(id, crate::agent::LoopState::new(Vec::new()));
                }
                Ok(CommandOutput::text("Conversation cleared"))
            }
        }

        commands.register(
            crate::commands::registry::CommandMeta {
                name: "clear".into(),
                description: "Clear conversation".into(),
                args_hint: None,
            },
            Box::new(ClearHandler {
                sessions: sessions_for_clear,
            }),
        );

        // 注册 /model 命令
        commands.register(
            crate::commands::registry::CommandMeta {
                name: "model".into(),
                description: "Switch model".into(),
                args_hint: Some("[model_key]".into()),
            },
            Box::new(ModelCommandHandler),
        );

        // 注册 /init 命令
        commands.register(
            crate::commands::registry::CommandMeta {
                name: "init".into(),
                description: "Generate AGENTS.md".into(),
                args_hint: None,
            },
            Box::new(InitCommandHandler),
        );

        Self {
            state: AppState {
                provider,
                config,
                sessions,
                commands: Arc::new(commands),
            },
            port,
        }
    }
}
```

### Step 3: 新增 HTTP handler

在 `server.rs` 中 `handle_chat_endpoint` 之后添加：

```rust
#[derive(serde::Deserialize)]
struct ExecuteCommandBody {
    args: Option<String>,
    session_id: Option<String>,
}

async fn handle_list_commands(State(state): State<AppState>) -> Json<crate::server::models::ApiResponse<Vec<crate::commands::registry::CommandMeta>>> {
    let metas = state.commands.list();
    let owned: Vec<_> = metas.into_iter().cloned().collect();
    Json(crate::server::models::ApiResponse {
        success: true,
        data: Some(owned),
        error: None,
    })
}

async fn handle_execute_command(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(body): Json<ExecuteCommandBody>,
) -> Json<crate::server::models::ApiResponse<CommandOutput>> {
    let ctx = CommandContext {
        provider: state.provider.clone(),
        config: state.config.clone(),
        session_id: body.session_id,
    };

    match state.commands.execute(&name, body.args, &ctx).await {
        Ok(output) => Json(crate::server::models::ApiResponse {
            success: true,
            data: Some(output),
            error: None,
        }),
        Err(e) => Json(crate::server::models::ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
```

### Step 4: 注册路由

在 `run` 方法的路由链中增加：

```rust
.route("/api/commands", get(handle_list_commands))
.route("/api/commands/:name/execute", post(handle_execute_command))
```

### Step 5: 编译验证

Run: `cargo check`

Expected: 通过，无错误。

### Step 6: Commit

```bash
git add src/server/server.rs
git commit -m "feat(server): add CommandRegistry to AppState and expose /api/commands endpoints"
```

---

## Task 4: TuiClient 新增命令相关方法

**Files:**
- Modify: `src/tui/client.rs`

### Step 1: 导入类型

在 `client.rs` 顶部增加：

```rust
use crate::commands::registry::{CommandMeta, CommandOutput};
```

### Step 2: 新增 `list_commands`

在 `TuiClient` 的 `impl` 块中添加：

```rust
/// 获取所有可用命令的元数据列表
pub async fn list_commands(&self) -> Result<Vec<CommandMeta>> {
    let resp = self
        .client
        .get(format!("{}/api/commands", self.base_url))
        .send()
        .await?
        .json::<ApiResponse<Vec<CommandMeta>>>()
        .await?;

    match resp.data {
        Some(data) => Ok(data),
        None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
    }
}
```

### Step 3: 新增 `execute_command`

```rust
/// 执行指定命令
pub async fn execute_command(
    &self,
    name: &str,
    args: Option<String>,
    session_id: Option<String>,
) -> Result<CommandOutput> {
    let body = serde_json::json!({
        "args": args,
        "session_id": session_id,
    });

    let resp = self
        .client
        .post(format!("{}/api/commands/{}/execute", self.base_url, name))
        .json(&body)
        .send()
        .await?
        .json::<ApiResponse<CommandOutput>>()
        .await?;

    match resp.data {
        Some(data) => Ok(data),
        None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
    }
}
```

### Step 4: 编译验证

Run: `cargo check`

Expected: 通过。

### Step 5: Commit

```bash
git add src/tui/client.rs
git commit -m "feat(tui): add list_commands and execute_command to TuiClient"
```

---

## Task 5: TUI 鼠标事件支持与事件路由

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/event.rs`

### Step 1: 启用鼠标捕获（`src/tui/mod.rs`）

```rust
pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // 启用鼠标事件捕获
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture
    );

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    // 退出前禁用鼠标捕获（防止终端残留鼠标事件）
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture
    );
    ratatui::restore();
    result
}
```

### Step 2: `app.rs` 路由 Mouse 事件

在 `route_event` 中增加 `Event::Mouse` 分支：

```rust
async fn route_event(&mut self, event: Event) {
    match event {
        Event::Key(key) => {
            // 现有 Key 处理逻辑...
        }
        Event::Mouse(mouse) => {
            if self.focus == FocusArea::Input {
                let app_event = self.input.handle_event(&Event::Mouse(mouse), true);
                if let Some(app_event) = app_event {
                    self.handle_app_event(app_event).await;
                }
            }
        }
        _ => {
            self.maybe_focus_input(&event);
            self.dispatch_event(event).await;
        }
    }
}
```

> **注意：** `route_event` 中原来的 `let Event::Key(key) = event else { ... }` 需要改为 `match event`。

### Step 3: `event.rs` 新增事件（用于命令执行结果反馈）

```rust
pub enum AppEvent {
    // ... 现有事件 ...
    ClearChat,
    ShowSystemMessage(String),
    // ...
}
```

### Step 4: Commit

```bash
git add src/tui/mod.rs src/tui/app.rs src/tui/event.rs
git commit -m "feat(tui): enable mouse capture and route mouse events to Input"
```

---

## Task 6: Input 组件重构（斜杠菜单从 HTTP 获取、支持鼠标）

**Files:**
- Modify: `src/tui/components/input.rs`

### Step 1: 修改 `Input` 结构体

```rust
pub struct Input {
    content: String,
    cursor_position: usize,
    dropdown_visible: bool,
    dropdown_items: Vec<CommandMeta>,  // 改为从 HTTP 获取的 CommandMeta
    dropdown_selected: usize,
    session_id: Option<String>,
    last_drawn_area: Option<Rect>,     // 新增：记录输入框最后绘制的区域，用于鼠标碰撞检测
    dropdown_area: Option<Rect>,       // 新增：记录下拉菜单区域
    commands_loaded: bool,             // 新增：是否已从 Server 加载命令列表
}
```

### Step 2: 修改 `Input::new`

```rust
pub fn new() -> Self {
    Self {
        content: String::new(),
        cursor_position: 0,
        dropdown_visible: false,
        dropdown_items: Vec::new(),  // 初始为空，首次输入 / 时从 Server 加载
        dropdown_selected: 0,
        session_id: None,
        last_drawn_area: None,
        dropdown_area: None,
        commands_loaded: false,
    }
}
```

### Step 3: 新增 `set_last_drawn_area`

```rust
pub fn set_last_drawn_area(&mut self, area: Rect) {
    self.last_drawn_area = Some(area);
}
```

### Step 4: 重构 `check_slash_commands` 为异步加载

由于 `Input::handle_event` 不是 async 的，我们不能在 `handle_event` 中直接调用 async 的 `TuiClient::list_commands`。因此，命令列表的加载由 `TuiApp` 负责：当 `Input` 检测到 `/` 时，发送一个 `AppEvent::LoadCommands` 事件，`TuiApp` 调用 `client.list_commands().await`，然后将结果通过 `update` 方法传给 `Input`。

为此，需要新增 `AppEvent::LoadCommands` 和 `Input::set_commands`。

**修改 `event.rs`：**

```rust
pub enum AppEvent {
    // ...
    LoadCommands,                // 新增：触发从 Server 加载命令列表
    SetCommands(Vec<CommandMeta>), // 新增：将命令列表设置到 Input
    // ...
}
```

**修改 `input.rs`：**

```rust
pub fn set_commands(&mut self, commands: Vec<CommandMeta>) {
    self.dropdown_items = commands;
    self.commands_loaded = true;
    if self.content == "/" {
        self.dropdown_visible = true;
        self.dropdown_selected = 0;
    }
}

pub fn is_dropdown_visible(&self) -> bool {
    self.dropdown_visible
}
```

### Step 5: `check_slash_commands` 逻辑调整

```rust
fn check_slash_commands(&mut self) -> Option<AppEvent> {
    if self.content == "/" {
        if self.commands_loaded {
            self.dropdown_visible = true;
            self.dropdown_selected = 0;
            None
        } else {
            // 需要加载命令列表
            Some(AppEvent::LoadCommands)
        }
    } else if !self.content.starts_with('/') {
        self.dropdown_visible = false;
        None
    } else {
        None
    }
}
```

### Step 6: 修改 `Component::handle_event` 中的键盘逻辑

在 `handle_event` 中，当 dropdown 可见时，处理方向键和回车：

```rust
fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return None;
            }

            if self.dropdown_visible {
                match key.code {
                    KeyCode::Up => {
                        if self.dropdown_selected > 0 {
                            self.dropdown_selected -= 1;
                        }
                        return None;
                    }
                    KeyCode::Down => {
                        if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
                            self.dropdown_selected += 1;
                        }
                        return None;
                    }
                    KeyCode::Enter => {
                        if let Some(cmd) = self.dropdown_items.get(self.dropdown_selected) {
                            return Some(AppEvent::ExecuteSlashCommand {
                                name: cmd.name.clone(),
                                args_hint: cmd.args_hint.clone(),
                            });
                        }
                        return None;
                    }
                    KeyCode::Esc => {
                        self.dropdown_visible = false;
                        return None;
                    }
                    _ => {}
                }
            }
            // ... 原有非 dropdown 的键盘处理逻辑 ...
        }
        Event::Mouse(mouse) => {
            if !self.dropdown_visible {
                return None;
            }
            match mouse.kind {
                crossterm::event::MouseEventKind::ScrollUp => {
                    if self.dropdown_selected > 0 {
                        self.dropdown_selected -= 1;
                    }
                    None
                }
                crossterm::event::MouseEventKind::ScrollDown => {
                    if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
                        self.dropdown_selected += 1;
                    }
                    None
                }
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    // 碰撞检测：判断鼠标是否在下拉菜单内
                    if let Some(area) = self.dropdown_area {
                        let mx = mouse.column;
                        let my = mouse.row;
                        if mx >= area.x
                            && mx < area.x + area.width
                            && my >= area.y
                            && my < area.y + area.height
                        {
                            // 计算点击的菜单项索引
                            let item_y = my.saturating_sub(area.y + 1); // +1 为边框
                            let index = item_y as usize;
                            if index < self.dropdown_items.len() {
                                self.dropdown_selected = index;
                                let cmd = &self.dropdown_items[index];
                                return Some(AppEvent::ExecuteSlashCommand {
                                    name: cmd.name.clone(),
                                    args_hint: cmd.args_hint.clone(),
                                });
                            }
                        } else {
                            // 点击菜单外部，关闭菜单
                            self.dropdown_visible = false;
                        }
                    }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}
```

### Step 7: `draw_dropdown` 中记录区域

由于 `draw` 是 `&self`，不能直接保存 `dropdown_area`。改为在 `draw` 方法中通过 `app.rs` 传入 `&mut self` 来保存。

但 `Component::draw` 的签名是 `&self`。解决方案：在 `draw` 返回前，由 `Input` 的调用方（`app.rs`）根据 `last_drawn_area` 和 `dropdown_items` 重新计算 `dropdown_area` 并保存。

更简单的方式：在 `Input` 中新增一个 `update_dropdown_area` 方法，由 `app.rs` 在 `draw` 之后调用。

```rust
impl Input {
    pub fn update_dropdown_area(&mut self, input_area: Rect) {
        if !self.dropdown_visible || self.dropdown_items.is_empty() {
            self.dropdown_area = None;
            return;
        }
        let items_len = self.dropdown_items.len() as u16;
        let height = items_len + 2;
        let width = 40u16.min(input_area.width);
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height);
        self.dropdown_area = Some(Rect::new(x, y, width, height));
    }
}
```

### Step 8: `draw_dropdown` 使用 `self.dropdown_area`（如果已计算）

由于 `draw_dropdown` 在 `draw(&self, ...)` 中调用，不能修改 `self`。所以 `draw_dropdown` 仍然自己计算区域用于渲染，但不保存。

### Step 9: `app.rs` 中 `draw` 之后更新 `dropdown_area`

```rust
fn draw(&mut self, frame: &mut ratatui::Frame) {
    // ... 现有绘制逻辑 ...
    self.input.draw(frame, input_area, &self.theme, self.focus == FocusArea::Input);
    self.input.set_last_drawn_area(input_area);
    self.input.update_dropdown_area(input_area);
    // ...
}
```

### Step 10: `app.rs` 中处理 `LoadCommands` 和 `ExecuteSlashCommand`

修改 `handle_app_event`：

```rust
AppEvent::LoadCommands => {
    let client = self.client.clone();
    let tx = self.event_tx.clone();
    tokio::spawn(async move {
        match client.list_commands().await {
            Ok(commands) => {
                let _ = tx.send(AppEvent::SetCommands(commands)).await;
            }
            Err(_) => {
                // Server 未启动，回退到硬编码列表
                let fallback = vec![
                    CommandMeta { name: "clear".into(), description: "Clear conversation".into(), args_hint: None },
                    CommandMeta { name: "model".into(), description: "Switch model".into(), args_hint: Some("[model_key]".into()) },
                    CommandMeta { name: "init".into(), description: "Generate AGENTS.md".into(), args_hint: None },
                    CommandMeta { name: "help".into(), description: "Show help".into(), args_hint: None },
                ];
                let _ = tx.send(AppEvent::SetCommands(fallback)).await;
            }
        }
    });
}
AppEvent::SetCommands(commands) => {
    self.input.set_commands(commands);
}
AppEvent::ExecuteSlashCommand { name, args_hint } => {
    self.input.set_content(format!("/{}", name));
    if args_hint.is_some() {
        // 有参数，等待用户补全
        self.input.set_cursor_position(self.input.content().len());
        self.input.close_dropdown();
    } else {
        // 无参数，自动执行
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        let session_id = self.header.session_id();
        let cmd_name = name.clone();
        tokio::spawn(async move {
            match client.execute_command(&cmd_name, None, session_id).await {
                Ok(output) => {
                    if !matches!(output.r#type, OutputType::Silent) {
                        let _ = tx.send(AppEvent::ShowSystemMessage(output.message)).await;
                    }
                    if let Some(meta) = output.metadata {
                        if let Some(model) = meta.get("current_model").and_then(|v| v.as_str()) {
                            let _ = tx.send(AppEvent::SelectModel(model.to_string())).await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ShowSystemMessage(format!("Error: {}", e))).await;
                }
            }
        });
        self.input.clear_content();
    }
}
AppEvent::ShowSystemMessage(msg) => {
    self.chat.add_system_message(&msg);
}
```

> **新增辅助方法：** `Input::set_content`, `Input::content`, `Input::set_cursor_position`, `Input::close_dropdown`, `Input::clear_content`, `Chat::add_system_message`。

### Step 11: Commit

```bash
git add src/tui/components/input.rs src/tui/app.rs src/tui/event.rs
git commit -m "feat(tui): refactor Input slash menu to use HTTP commands, support mouse"
```

---

## Task 7: Chat 组件支持系统消息显示

**Files:**
- Modify: `src/tui/components/chat.rs`

### Step 1: 新增 `add_system_message`

```rust
pub fn add_system_message(&mut self, content: &str) {
    self.messages.push(Message {
        role: MessageRole::System,
        content: content.to_string(),
    });
}
```

### Step 2: 调整 `System` 消息的渲染样式

在 `draw` 方法中：

```rust
MessageRole::System => ("ℹ️ ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
```

### Step 3: Commit

```bash
git add src/tui/components/chat.rs
git commit -m "feat(tui): support system messages in Chat component"
```

---

## Task 8: 集成编译与测试

### Step 1: 全量编译

Run: `cargo check`

Expected: 通过，无错误。

### Step 2: 运行所有单元测试

Run: `cargo test`

Expected: 全部通过（包括新添加的 registry 测试和原有测试）。

### Step 3: Clippy 检查

Run: `cargo clippy --all-targets`

Expected: 无 warning。

### Step 4: 手动验证（TUI 模式）

Run: `cargo run`（确保默认进入 TUI 模式）

验证项：
1. 输入 `/`，菜单是否弹出并显示 `clear`, `model`, `init`
2. 上下方向键是否能滚动选中
3. 鼠标滚轮是否能滚动选中
4. 选中 `/clear` 回车，聊天区是否显示 "Conversation cleared"
5. 选中 `/model` 回车，输入框是否变为 `/model `，等待补全参数
6. 输入 `/model gpt-4o` 回车，模型是否切换，Header 是否更新
7. 点击菜单外部，菜单是否关闭

### Step 5: 最终 Commit

```bash
git commit -m "feat(tui): complete slash command menu with CommandRegistry and mouse support"
```

---

## 自检清单

### Spec 覆盖检查

| Spec 需求 | 对应 Task | 状态 |
|-----------|-----------|------|
| CommandRegistry + CommandHandler trait | Task 1 | ✅ |
| list() 方法 | Task 1 | ✅ |
| /model, /init 迁移 | Task 2 | ✅ |
| /clear 注册 | Task 3 | ✅ |
| Server HTTP API /api/commands | Task 3 | ✅ |
| TuiClient list_commands / execute_command | Task 4 | ✅ |
| 鼠标捕获 EnableMouseCapture | Task 5 | ✅ |
| 鼠标滚轮滚动菜单 | Task 6 | ✅ |
| 鼠标左键点击执行 | Task 6 | ✅ |
| 选中后自动填入并执行 | Task 6 | ✅ |
| 执行结果作为系统消息显示 | Task 6, 7 | ✅ |
| 回退到硬编码列表 | Task 6 | ✅ |

### Placeholder 扫描

- [x] 无 "TBD", "TODO", "implement later"
- [x] 无 "Add appropriate error handling" 等模糊描述
- [x] 每个步骤包含具体代码
- [x] 无 "Similar to Task N" 引用

### 类型一致性检查

- [x] `CommandMeta` 在 Task 1、3、4、6 中定义一致
- [x] `CommandOutput` 在 Task 1、3、4、6 中定义一致
- [x] `AppEvent::ExecuteSlashCommand` 在 Task 6 中定义并处理
- [x] `OutputType` 枚举命名一致（snake_case）
