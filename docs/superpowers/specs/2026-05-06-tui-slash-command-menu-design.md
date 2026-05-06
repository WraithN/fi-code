# TUI 斜杠命令菜单设计文档

> 日期：2026-05-06
> 状态：已评审，待实现

## 1. 需求概述

在 TUI 的输入框中输入 `/` 时，弹出一个上拉菜单展示所有可用指令。支持上下方向键或鼠标滚轮滚动，选中项高亮。回车或鼠标左键点击后，将指令自动填入输入框并执行，执行完成后清空输入框。

### 1.1 关键决策回顾

| 决策项 | 选择 |
|--------|------|
| 执行行为 | 自动执行并清空输入框（选中后短暂显示 `/name` 作为视觉反馈） |
| 鼠标支持 | 必须支持（滚轮滚动 + 左键点击） |
| 指令来源 | 使用 `commands` 模块中的指令，通过 `CommandRegistry` 统一管理 |
| 架构方案 | **方案 B：Server 端 Registry + HTTP API**（保持前后端分离） |

---

## 2. 架构设计

### 2.1 整体数据流

```
TUI (Input 组件)
  │ 1. 输入 `/` → 请求命令列表
  ▼
TuiClient ──HTTP GET──► Server (/api/commands)
  │ ◄── 返回 [CommandMeta]
  │ 2. 渲染上拉菜单
  │ 3. 用户选择 `/clear`
  ▼
TuiClient ──HTTP POST──► Server (/api/commands/clear/execute)
  │ ◄── 返回 CommandOutput { message, type, metadata }
  │ 4. 显示结果到 Chat 区
  ▼
Input 框清空
```

### 2.2 模块关系

```
commands/
  ├── mod.rs          # 导出 SlashCommand, SlashCommandHandler, CommandRegistry
  ├── slash.rs        # 现有 CLI 斜杠命令（保留，内部逻辑迁移到 Handler）
  └── registry.rs     # 新增：CommandRegistry, CommandHandler trait, CommandMeta, CommandOutput

server/
  ├── mod.rs
  ├── server.rs       # 新增 /api/commands 路由；启动时注册命令到 AppState.commands
  └── ...

tui/
  ├── mod.rs          # 启用/禁用鼠标捕获
  ├── app.rs          # 路由 Mouse 事件到 Input 组件
  ├── client.rs       # 新增 list_commands, execute_command
  └── components/
      └── input.rs    # 重构斜杠菜单：从 HTTP 获取列表、支持鼠标、执行后显示结果
```

---

## 3. CommandRegistry 设计

### 3.1 核心数据结构

```rust
/// 命令元数据，用于 TUI 展示和 HTTP API 返回
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandMeta {
    pub name: String,
    pub description: String,
    pub args_hint: Option<String>, // 如 "[model_key]"
}

/// 命令执行上下文，由调用方（Server）传入
pub struct CommandContext {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub session_id: Option<String>,
}

/// 命令执行结果类型
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    Text,   // 普通文本，显示在聊天区
    Error,  // 错误信息，红色显示
    Silent, // 静默执行，不显示任何内容
}

/// 命令执行结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandOutput {
    pub message: String,
    pub r#type: OutputType,
    pub metadata: Option<serde_json::Value>, // 可选，如 {"current_model": "gpt-4o"}
}

/// 命令处理器 trait
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, args: Option<String>, ctx: &CommandContext) -> Result<CommandOutput>;
}

/// 命令注册表
pub struct CommandRegistry {
    commands: HashMap<String, CommandEntry>,
}

struct CommandEntry {
    meta: CommandMeta,
    handler: Box<dyn CommandHandler>,
}
```

### 3.2 Registry API

```rust
impl CommandRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, meta: CommandMeta, handler: Box<dyn CommandHandler>);
    pub fn list(&self) -> Vec<&CommandMeta>;
    pub async fn execute(&self, name: &str, args: Option<String>, ctx: &CommandContext) -> Result<CommandOutput>;
}
```

### 3.3 命令注册（Server 端）

Server 启动时在 `AppState` 中创建并配置 `CommandRegistry`：

```rust
let mut commands = CommandRegistry::new();

// /clear —— 清空当前 HTTP 会话消息
commands.register(
    CommandMeta { name: "clear".into(), description: "Clear conversation".into(), args_hint: None },
    Box::new(ClearCommandHandler { sessions: sessions.clone() }),
);

// /model —— 迁移自 SlashCommandHandler::handle_model
commands.register(
    CommandMeta { name: "model".into(), description: "Switch model".into(), args_hint: Some("[model_key]".into()) },
    Box::new(ModelCommandHandler),
);

// /init —— 迁移自 SlashCommandHandler::handle_init
commands.register(
    CommandMeta { name: "init".into(), description: "Generate AGENTS.md".into(), args_hint: None },
    Box::new(InitCommandHandler),
);
```

### 3.4 SlashCommandHandler 迁移

将 `commands/slash.rs` 中的 `handle_model` 和 `handle_init` 逻辑提取为独立的 `CommandHandler` 实现：

- `ModelCommandHandler`：复用切换模型和列出模型的逻辑，返回 `CommandOutput` 而非 `println!`
- `InitCommandHandler`：复用 AGENTS.md 生成逻辑，返回 `CommandOutput`

`SlashCommandHandler` 本身保留用于 CLI 模式。为减少重复代码，CLI 模式可以复用新的 `CommandHandler` 实现（将 `println!` 输出包装为 `CommandOutput`，或在 CLI 中直接调用 handler 并打印结果）。

---

## 4. HTTP API 设计

### 4.1 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/commands` | 获取所有可用命令的元数据列表 |
| POST | `/api/commands/:name/execute` | 执行指定命令 |

### 4.2 GET /api/commands

**Response:**
```json
{
  "success": true,
  "data": [
    { "name": "clear", "description": "Clear conversation", "args_hint": null },
    { "name": "model", "description": "Switch model", "args_hint": "[model_key]" },
    { "name": "init", "description": "Generate AGENTS.md", "args_hint": null }
  ],
  "error": null
}
```

### 4.3 POST /api/commands/:name/execute

**Request Body:**
```json
{
  "args": "gpt-4o",
  "session_id": "01HV8..."
}
```

**Response (Success):**
```json
{
  "success": true,
  "data": {
    "message": "✅ 已切换模型: gpt-4o",
    "type": "text",
    "metadata": { "current_model": "gpt-4o" }
  },
  "error": null
}
```

**Response (Error):**
```json
{
  "success": false,
  "data": null,
  "error": "Unknown command: foo"
}
```

---

## 5. TUI 交互增强

### 5.1 鼠标事件支持

**启用鼠标捕获**（`tui/mod.rs`）：
```rust
pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // 启用鼠标事件捕获（滚轮 + 点击）
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture
    );

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    // 退出前禁用鼠标捕获
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture
    );
    ratatui::restore();
    result
}
```

**事件路由**（`tui/app.rs`）：
- `Event::Mouse(mouse)`：如果 `Input` 的下拉菜单可见，将鼠标事件转发给 `Input` 处理
- `MouseEventKind::ScrollUp/ScrollDown`：菜单向上/向下滚动
- `MouseEventKind::Down(MouseButton::Left)`：计算点击位置与菜单项的碰撞，触发执行

### 5.2 斜杠菜单交互流程

1. **触发**：用户在 Input 框输入 `/`
2. **获取列表**：`Input` 组件通过 `TuiClient::list_commands()` 获取可用命令列表（首次触发时缓存，后续复用）
3. **显示**：在输入框上方绘制上拉菜单，显示 `name` + `description`
4. **导航**：
   - `↑/↓` 方向键：移动选中项
   - `ScrollUp/ScrollDown`：同上
   - `Esc`：关闭菜单
5. **执行**（无参数命令，如 `/clear`）：
   - `Enter` 或 `LeftClick`：
     a. 将 `/clear` 填入输入框（短暂显示，作为视觉反馈）
     b. 调用 `TuiClient::execute_command("clear", None, session_id)`
     c. 清空输入框
     d. 将执行结果（`CommandOutput.message`）作为系统消息显示在 `Chat` 区
     e. 若 `metadata` 包含 `current_model`，更新 `Header` 的模型显示
6. **执行**（有参数命令，如 `/model`）：
   - `Enter` 或 `LeftClick`：将 `/model ` 填入输入框，光标定位在参数位置，菜单关闭，等待用户补全参数后按回车执行

### 5.3 TuiClient 新增方法

```rust
pub async fn list_commands(&self) -> Result<Vec<CommandMeta>>;
pub async fn execute_command(
    &self,
    name: &str,
    args: Option<String>,
    session_id: Option<String>,
) -> Result<CommandOutput>;
```

---

## 6. 边界情况与错误处理

| 场景 | 处理方式 |
|------|----------|
| Server 未启动时 TUI 获取命令列表 | `TuiClient::list_commands()` 返回 Err，Input 组件回退到硬编码的基础命令列表，保证 TUI 仍可正常使用 |
| 用户输入 `/` 后快速输入其他字符（如 `/foo`） | 菜单立即关闭，按普通文本处理；若 `/foo` 匹配不到命令，提交时作为普通消息发送给 LLM |
| 鼠标点击在菜单区域外 | 关闭下拉菜单，焦点保持或转移到 Input |
| 命令执行失败（如 `/model xxx` 模型不存在） | `CommandOutput` type 为 Error，在 Chat 区以红色显示错误消息 |
| 命令执行耗时较长（如 `/init` 生成 AGENTS.md） | TUI 显示 "Executing /init..." 状态，执行完成后显示结果；期间不阻塞 UI（通过 tokio::spawn） |
| 终端不支持鼠标事件 | `EnableMouseCapture` 失败不阻断程序，静默降级为纯键盘交互 |
| 命令不存在 | Registry 返回 Err，HTTP 层包装为 `ApiResponse { success: false }`，TUI 显示错误消息 |

---

## 7. 测试策略

### 7.1 单元测试

- **commands/registry.rs**：
  - `test_register_and_list`：验证注册后 list 返回正确元数据
  - `test_execute_unknown_command`：验证未注册命令返回 Err
  - `test_command_output_serde`：验证 `CommandOutput` 序列化/反序列化

- **tui/components/input.rs**：
  - `test_slash_menu_navigation`：方向键滚动选中项
  - `test_slash_menu_execute`：回车选中后触发 AppEvent
  - `test_slash_menu_mouse_scroll`：模拟 Mouse Scroll 事件

### 7.2 集成测试

- 使用 `wiremock` 或启动本地 Server 测试 `/api/commands` 和 `/api/commands/:name/execute`
- 验证 `/model` 切换后 `get_status()` 返回新模型名

### 7.3 UI 验证（人工）

- 菜单渲染位置在输入框上方
- 选中项高亮样式正确
- 命令执行后 Chat 区正确显示系统消息
- 鼠标滚轮和点击行为正常

---

## 8. 文件变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/commands/mod.rs` | 修改 | 导出 `CommandRegistry` 相关类型 |
| `src/commands/registry.rs` | 新增 | `CommandRegistry`、`CommandHandler`、`CommandMeta`、`CommandOutput` |
| `src/commands/slash.rs` | 修改 | 提取 `ModelCommandHandler` 和 `InitCommandHandler` |
| `src/server/server.rs` | 修改 | `AppState` 增加 `commands`；新增 `/api/commands` 路由；启动时注册命令 |
| `src/tui/mod.rs` | 修改 | 启用/禁用鼠标捕获 |
| `src/tui/app.rs` | 修改 | 路由 Mouse 事件到 Input 组件 |
| `src/tui/client.rs` | 修改 | 新增 `list_commands`、`execute_command` |
| `src/tui/event.rs` | 修改 | 可能新增事件类型（如 `ExecuteCommandResult`） |
| `src/tui/components/input.rs` | 大幅修改 | 重构斜杠菜单：HTTP 获取列表、鼠标支持、执行结果反馈 |
| `src/tui/components/chat.rs` | 可能修改 | 支持显示系统/错误类型的 CommandOutput 消息 |
