# Agent 系统设计规格书

> 设计日期：2026-05-18
> 状态：待实现

---

## 1. 背景与目标

### 1.1 背景

当前 fi-code 只有一个默认 Agent，可以调用所有工具。随着使用场景多样化，用户需要：
- **Build Agent**：全功能编码助手，可以读写文件、执行命令（当前默认行为）
- **Plan Agent**：只读规划助手，只能读取代码和资料，制定计划但不实际执行

### 1.2 设计目标

1. 将 `AgentRunner` 抽离为独立的调度器，Agent 只负责定义行为（工具集、提示词、权限）
2. 支持两种 Agent：Build（默认）和 Plan
3. TUI 状态栏展示当前 Agent 名称，支持 `CTRL+A` 切换
4. CLI 通过 `--agent` 参数支持，Server 通过 API `agent` 字段支持
5. Agent 类型与会话绑定，持久化到 JSONL
6. 向后兼容：无 `agent_type` 字段的旧会话默认使用 Build Agent

---

## 2. 核心数据结构

### 2.1 AgentType（类型标识）

```rust
// crates/shared/src/dto.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Build,
    Plan,
}

impl Default for AgentType {
    fn default() -> Self { AgentType::Build }
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Build => "Build",
            AgentType::Plan => "Plan",
        }
    }
}
```

### 2.2 ToolFilter（工具过滤策略）

```rust
// crates/core/src/agent/profile.rs
#[derive(Debug, Clone)]
pub enum ToolFilter {
    AllowList(HashSet<String>),
    BlockList(HashSet<String>),
    Predicate(fn(&str) -> bool),
}

impl ToolFilter {
    pub fn apply(&self, tools_schema: &Value) -> Value {
        // 过滤 tools_schema 数组，只保留符合条件的工具
    }
    
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            ToolFilter::AllowList(set) => set.contains(tool_name),
            ToolFilter::BlockList(set) => !set.contains(tool_name),
            ToolFilter::Predicate(pred) => pred(tool_name),
        }
    }
}
```

### 2.3 AgentProfile（行为配置）

```rust
pub struct AgentProfile {
    pub name: &'static str,
    pub prompt_suffix: &'static str,
    pub tool_filter: ToolFilter,
    pub can_execute_tasks: bool,
}

impl AgentProfile {
    pub fn for_type(agent_type: AgentType) -> &'static Self {
        static PROFILES: LazyLock<HashMap<AgentType, AgentProfile>> = LazyLock::new(|| {
            let mut m = HashMap::new();
            
            let mut build_tools = HashSet::new();
            // Build Agent 允许所有本地工具
            build_tools.extend([
                "bash", "read", "read_file", "write", "edit",
                "grep", "glob", "web_fetch",
                "git", "git_status", "git_diff", "git_add", "git_commit", "git_log", "git_worktree",
                "create_task_plan", "handle_task_plan",
                "ask_for_question", "use_skill",
            ].map(String::from));
            
            let mut plan_tools = HashSet::new();
            // Plan Agent 只允许只读工具
            plan_tools.extend([
                "read", "read_file", "grep", "glob",
                "git_status", "git_log", "git_diff",
                "web_fetch",
                "create_task_plan", "handle_task_plan",
            ].map(String::from));
            
            m.insert(AgentType::Build, AgentProfile {
                name: "Build",
                prompt_suffix: concat!(
                    "You are a full-featured coding assistant. ",
                    "You can read and write files, execute shell commands, ",
                    "manage Git operations, and perform any task necessary ",
                    "to help the user with their project."
                ),
                tool_filter: ToolFilter::AllowList(build_tools),
                can_execute_tasks: true,
            });
            
            m.insert(AgentType::Plan, AgentProfile {
                name: "Plan",
                prompt_suffix: concat!(
                    "You are a planning assistant. You can only read code ",
                    "and materials, but you cannot modify files or execute commands. ",
                    "Your task is to analyze requirements, examine the codebase, ",
                    "and produce detailed implementation plans. ",
                    "When using create_task_plan or handle_task_plan, ",
                    "you should create the plan and mark it complete, ",
                    "but do not actually execute the sub-tasks."
                ),
                tool_filter: ToolFilter::AllowList(plan_tools),
                can_execute_tasks: false,
            });
            
            m
        });
        
        PROFILES.get(&agent_type).expect("profile must exist")
    }
}
```

### 2.4 会话绑定 Agent 类型

```rust
// crates/shared/src/dto.rs - Session 结构体新增字段
pub struct Session {
    pub id: String,
    pub project_path: String,
    pub model: String,
    pub status: SessionStatus,
    pub agent_type: AgentType,  // 新增
    pub messages: Vec<Message>,
}
```

**JSONL 持久化格式变更**：

```json
{"type":"session","id":"01HV8J...","project_path":"/home/nan/project","model":"kimi-k2.5","status":"active","agent_type":"plan"}
```

- 向后兼容：旧会话缺失 `agent_type` 字段时，反序列化默认使用 `AgentType::Build`
- 新增会话默认使用 `AgentType::Build`

---

## 3. Runner 调度器改造

### 3.1 AgentRunner 重构

```rust
// crates/core/src/agent/runner.rs
pub struct AgentRunner {
    client: Box<dyn AIClient>,
    profile: &'static AgentProfile,
    max_turns: usize,
}

impl AgentRunner {
    pub fn new(client: Box<dyn AIClient>, agent_type: AgentType) -> Self {
        Self {
            client,
            profile: AgentProfile::for_type(agent_type),
            max_turns: 25,
        }
    }

    pub async fn run(
        &self,
        initial_messages: Vec<Message>,
    ) -> Result<AgentRunResult> {
        // 1. 根据 profile 过滤 tools_schema
        let tools_schema = self.profile.tool_filter.apply(&all_tools_schema());
        
        // 2. 组装 system_prompt：基础模板 + profile.prompt_suffix
        let system_prompt = PromptBuilder::new()
            .with_agent_profile(self.profile)
            .build()?;
        
        // 3. 执行多轮对话循环
        self.run_loop(initial_messages, system_prompt, tools_schema).await
    }
}
```

### 3.2 工具过滤集成

`AgentProfile::tool_filter.apply()` 在 Runner 启动时过滤一次 `tools_schema`。LLM 看到的可用工具列表已经是过滤后的。

二次拦截层（防御性编程）：

```rust
// crates/core/src/tools/mod.rs
pub async fn execute_tool_calls(
    parts: &[Part],
    agent_type: AgentType,
    on_tool_event: impl Fn(ToolEvent),
) -> Vec<Part> {
    let profile = AgentProfile::for_type(agent_type);
    
    let futures = parts.iter()
        .filter_map(|p| match p {
            Part::ToolUse { id, name, arguments } => {
                if !profile.tool_filter.allows(name) {
                    Some(async move {
                        Part::ToolError {
                            tool_call_id: id.clone(),
                            content: format!("Tool '{}' is not allowed in {} Agent", name, profile.name),
                            error_message: "Permission denied by agent profile".to_string(),
                        }
                    })
                } else {
                    Some(execute_single_tool_call(id, name, arguments, on_tool_event))
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    
    join_all(futures).await
}
```

### 3.3 Task 执行的权限控制

Plan Agent 的 `can_execute_tasks: false` 在 `handle_task_plan` 工具中生效：

```rust
// crates/core/src/tools/task/...
pub async fn handle_task_plan(params: &HashMap<String, Value>) -> Result<String, String> {
    let current_agent = get_current_agent_type(); // 从上下文获取
    
    if let AgentType::Plan = current_agent {
        let plan = generate_plan_text(params)?;
        return Ok(format!("Plan created (not executed in Plan Agent):\n\n{}", plan));
    }
    
    execute_tasks(params).await
}
```

### 3.4 MCP 工具过滤

MCP 工具没有内置的读写属性标注。Plan Agent 对 MCP 工具的默认策略：

```rust
// crates/core/src/tools/mod.rs
pub async fn tool_schema_for_agent(agent_type: AgentType) -> Value {
    let mut schemas = REGISTRY.tool_schema(); // 本地工具
    
    if let Some(mcp) = get_mcp_manager() {
        for (full_name, desc) in mcp.tools_list().await {
            let profile = AgentProfile::for_type(agent_type);
            
            // Plan Agent：只有显式标记为 read_only 的 MCP 工具才允许
            if agent_type == AgentType::Plan {
                let is_read_only = mcp.is_tool_read_only(&full_name).await;
                if !is_read_only {
                    continue; // 跳过非只读 MCP 工具
                }
            }
            
            schemas.push(json!({
                "name": full_name,
                "description": desc,
                "input_schema": {}
            }));
        }
    }
    
    schemas
}
```

> 注：MCP 工具的 `read_only` 属性需要在 MCP 服务器配置中由用户显式标注，或基于工具名启发式判断（如包含 `read`, `get`, `list` 等前缀）。第一版实现采用保守策略：Plan Agent 默认禁止所有 MCP 工具，除非配置中显式声明。

---

## 4. TUI 集成

### 4.1 AppEvent 新增

```rust
// crates/shared/src/tui_event.rs
pub enum AppEvent {
    // ... 现有事件 ...
    
    SwitchAgent(AgentType),
    AgentSwitched {
        agent_type: AgentType,
        agent_name: String,
    },
}
```

### 4.2 键盘事件路由

```rust
// crates/tui/src/app.rs
match event {
    Event::Key(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        ..
    }) => {
        let current = self.current_agent_type();
        let next = match current {
            AgentType::Build => AgentType::Plan,
            AgentType::Plan => AgentType::Build,
        };
        Some(AppEvent::SwitchAgent(next))
    }
    // ...
}
```

### 4.3 状态栏展示

```rust
// crates/tui/src/components/status_bar.rs
pub struct StatusBar {
    // ... 现有字段 ...
    agent_name: String,
}

impl StatusBar {
    pub fn set_agent(&mut self, agent_name: String) {
        self.agent_name = agent_name;
    }
}
```

渲染布局（标准模式 ≥100 列）：

```
FiCode │ AGT: Plan │ CTX: [█████░░░░░] 64k/128k │ TOK: ↑24k ↓18k │ LAT: 2.4s │ MDL: kimi-k2.5 │ 09:14
```

- `AGT` 字段固定 4 字符宽度（Build/Plan 均 ≤4 字符）
- 紧凑模式（≥80 列）：保留 `AGT` 缩写
- 极限模式（<80 列）：隐藏 `AGT`，优先保留 CTX 和模型名

### 4.4 状态机处理

```rust
// crates/tui/src/app.rs
async fn handle_app_event(&mut self, event: AppEvent) {
    match event {
        AppEvent::SwitchAgent(agent_type) => {
            if self.is_generating {
                self.chat.add_system_message(
                    "Please wait for the current response to complete before switching agents.".to_string()
                );
                return;
            }
            
            if let Some(session) = &mut self.current_session {
                session.agent_type = agent_type;
                
                if let Err(e) = self.session_manager.save_session(session).await {
                    log_error!("Failed to save session agent type: {}", e);
                }
            }
            
            let profile = AgentProfile::for_type(agent_type);
            self.status_bar.set_agent(profile.name.to_string());
            
            self.event_tx.send(AppEvent::AgentSwitched {
                agent_type,
                agent_name: profile.name.to_string(),
            }).ok();
        }
        
        AppEvent::AgentSwitched { agent_name, .. } => {
            self.chat.add_system_message(format!("Switched to {} Agent", agent_name));
        }
        // ...
    }
}
```

---

## 5. CLI / Server 集成

### 5.1 CLI 参数

```rust
// crates/cli/src/cli_args.rs
#[derive(Parser, Debug)]
#[command(name = "fi-code")]
pub struct CliArgs {
    // ... 现有参数 ...
    
    #[arg(long, value_enum, default_value = "build")]
    pub agent: AgentType,
}
```

### 5.2 Server API

```rust
// crates/core/src/server/api/chat_api.rs
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<String>,
    pub agent: Option<AgentType>, // 新增
}

pub async fn chat_handler(...) -> Result<...> {
    let agent_type = req.agent.unwrap_or_default();
    let runner = AgentRunner::new(state.client.clone(), agent_type);
    // ...
}
```

### 5.3 SSE 响应扩展

```rust
// crates/shared/src/dto.rs
pub enum SseEvent {
    // ... 现有变体 ...
    AgentInfo {
        agent_type: AgentType,
        agent_name: String,
    },
}
```

Server 在 SSE 流开始时推送 `AgentInfo` 事件，告知前端当前会话的 Agent 类型。

---

## 6. 数据流

### 6.1 TUI 切换 Agent 完整流程

```
用户按 CTRL+A
    │
    ▼
crossterm::Event
    │
    ▼
TuiApp::handle_crossterm_result ──► AppEvent::SwitchAgent(Plan)
    │
    ▼
TuiApp::handle_app_event
    ├──► 检查 is_generating ──► 若 true，拒绝并提示
    ├──► 更新 current_session.agent_type = Plan
    ├──► SessionManager::save_session() 持久化
    ├──► StatusBar.set_agent("Plan")
    └──► AppEvent::AgentSwitched
              │
              ▼
        Chat.add_system_message("Switched to Plan Agent")
              │
              ▼
        下次 SubmitMessage ──► AgentRunner::new(Plan)
              │
              ▼
        PromptBuilder 注入 Plan Agent 提示词后缀
        ToolFilter 过滤为只读工具集
```

### 6.2 新会话创建流程

```
用户发送第一条消息
    │
    ▼
SessionManager::create_session(model)
    │
    ▼
Session { agent_type: Build, ... }  // 默认 Build
    │
    ▼
AgentRunner::new(Build) ──► 使用 Build Profile
```

---

## 7. 错误处理

| 场景 | 处理方式 |
|------|----------|
| 切换时正在生成 | 拒绝切换，Chat 区域提示用户等待 |
| Session 持久化失败 | 记录错误日志，不阻塞切换（内存状态已更新） |
| JSONL 恢复时 agent_type 缺失 | 向后兼容，默认 `AgentType::Build` |
| LLM 产生不允许的 ToolUse | Runner 二次拦截，返回 `ToolError` |
| Plan Agent 调用 handle_task_plan | 只生成计划文本，不执行子任务 |
| Server API 传入未知 agent 值 | 400 Bad Request，提示有效值为 build/plan |
| MCP 工具无 read_only 标注 | Plan Agent 默认禁止，除非配置显式声明 |

---

## 8. 边界情况

1. **会话历史中的 ToolResult**：切换 Agent 后，历史消息中的 `ToolResult`（之前 Build Agent 执行的结果）仍然保留在上下文中。LLM 可以看到之前的执行结果，但后续只能发起只读工具调用。

2. **Task 子任务的 Agent 类型**：`create_task_plan` 创建的子任务，其执行时的 Agent 类型与父会话一致。Plan Agent 的子任务也是 Plan Agent，只规划不执行。

3. **MCP 工具动态加载**：如果 MCP 服务器在会话进行中动态加载了新工具，Plan Agent 的新工具默认不可用，除非显式标记为只读。

4. **模型切换与 Agent 切换的关系**：模型切换（`CTRL+N`）和 Agent 切换（`CTRL+A`）是独立操作，互不影响。用户可以先切换模型再切换 Agent，或反之。

---

## 9. 测试策略

### 9.1 单元测试

| 测试目标 | 位置 |
|----------|------|
| `AgentType` 序列化/反序列化 | `crates/shared/src/dto.rs` |
| `ToolFilter::apply` 过滤正确性 | `crates/core/src/agent/profile.rs` |
| `AgentProfile::for_type` 返回正确配置 | `crates/core/src/agent/profile.rs` |
| `execute_tool_calls` 二次拦截 | `crates/core/src/tools/mod.rs` |
| `Session` 的 `agent_type` 默认值为 Build | `crates/core/src/session/` |
| `PromptBuilder::with_agent_profile` 正确注入后缀 | `crates/core/src/agent/prompt.rs` |

### 9.2 E2E / BDD 测试

| 测试场景 | 说明 |
|----------|------|
| TUI 启动默认 Build Agent | 状态栏显示 "Build" |
| TUI CTRL+A 切换为 Plan Agent | 状态栏变为 "Plan"，Chat 显示系统消息 |
| TUI 生成中禁止切换 Agent | 按键无响应或提示等待 |
| CLI `--agent plan` 启动 | 只读工具可用，写工具被拦截 |
| Server API 传入 `"agent": "plan"` | 响应中 SSE 包含 `AgentInfo` |
| JSONL 恢复旧会话 | 无 `agent_type` 字段时默认 Build |
| Plan Agent 调用 write 工具 | 返回 ToolError，内容提示权限不足 |
| Plan Agent 调用 handle_task_plan | 返回计划文本，不执行子任务 |

---

## 10. 文件改动清单

### 新增文件

| 文件 | 说明 |
|------|------|
| `crates/core/src/agent/profile.rs` | `AgentProfile`, `ToolFilter` 定义 |

### 修改文件

| 文件 | 改动内容 |
|------|----------|
| `crates/shared/src/dto.rs` | 新增 `AgentType` enum；`Session` 新增 `agent_type` 字段；`SseEvent` 新增 `AgentInfo` |
| `crates/shared/src/tui_event.rs` | 新增 `SwitchAgent`, `AgentSwitched` |
| `crates/core/src/agent/mod.rs` | 导出 `AgentType`, `AgentProfile`, `ToolFilter` |
| `crates/core/src/agent/runner.rs` | 重构 `AgentRunner`，接受 `AgentType`，集成 profile |
| `crates/core/src/agent/prompt.rs` | `PromptBuilder` 新增 `with_agent_profile` 方法 |
| `crates/core/src/tools/mod.rs` | `execute_tool_calls` 增加 `agent_type` 参数和二次拦截；`tool_schema_for_agent` |
| `crates/core/src/tools/task/` | `handle_task_plan` 检查 `can_execute_tasks` |
| `crates/core/src/session/session.rs` | 序列化/反序列化支持 `agent_type` |
| `crates/core/src/server/api/chat_api.rs` | `ChatRequest` 新增 `agent` 字段 |
| `crates/tui/src/app.rs` | `CTRL+A` 事件路由；`SwitchAgent` 状态机处理 |
| `crates/tui/src/components/status_bar.rs` | 新增 `agent_name` 字段和展示逻辑 |
| `crates/cli/src/cli_args.rs` | 新增 `--agent` 参数 |
| `crates/cli/src/entry.rs` | 传递 `args.agent` 给 `AgentRunner` |

---

## 11. 回滚策略

所有改动遵循模块化原则：
- `AgentProfile` 是新增模块，不影响现有代码
- `AgentRunner` 的改动是接口扩展（新增 `agent_type` 参数），旧调用点传入 `AgentType::Build` 即可保持原有行为
- 若需回滚，只需将各处 `AgentType::Build` 硬编码，移除 profile 系统
- JSONL 格式的 `agent_type` 字段是可选的，回滚后旧代码会忽略该字段
