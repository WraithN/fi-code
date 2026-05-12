// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::log_debug;
use crate::log_info;
use crate::log_trace;
use crate::mcp::manager::McpManager;
use crate::provider::Provider;
use crate::session::message::Part;
use crate::tui::event::{AppEvent, QuestionAnswer};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use tokio::sync::mpsc;

// =============================================================================
// Rust 基础概念：模块声明
// =============================================================================
// `pub mod` 声明当前模块包含的子模块，Rust 编译器会在同级目录下查找同名文件
// 例如 `basic_tools` 对应 `src/tools/basic_tools.rs`

pub mod basic_tools;
pub mod task;
pub mod tools_registry;
pub mod tools_type;
pub mod windows_compat;

// =============================================================================
// 模块内部导入
// =============================================================================
// `use` 把其他模块中的类型引入当前作用域，避免每次写全限定路径

use basic_tools::BasicTool;
use tools_registry::ToolsRegistry;
use tools_type::{ToolHandler, ToolParameter, ToolParams};

// 全局事件发送器（TuiApp 初始化时设置）
static EVENT_TX: RwLock<Option<mpsc::Sender<AppEvent>>> = RwLock::new(None);

// 问题答案通道
type QuestionResponseSender = tokio::sync::oneshot::Sender<QuestionAnswer>;
pub static QUESTION_CHANNEL: LazyLock<Mutex<Option<QuestionResponseSender>>> =
    LazyLock::new(|| Mutex::new(None));

// 设置全局事件发送器
pub fn set_event_tx(tx: mpsc::Sender<AppEvent>) {
    let mut event_tx = EVENT_TX.write().unwrap();
    *event_tx = Some(tx);
}

// 获取全局事件发送器
pub fn get_event_tx() -> Option<mpsc::Sender<AppEvent>> {
    EVENT_TX.read().unwrap().clone()
}

// =============================================================================
// 辅助函数：从 JSON 对象中提取字符串参数
// =============================================================================
// `and_then` 是 Option/Result 的链式操作方法：
// 如果上一步是 Some，则继续执行闭包；如果是 None，则直接传递 None

fn get_json_param(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

// =============================================================================
// BashHandler：执行 shell 命令
// =============================================================================
// 这是一个空结构体（unit struct），因为它不需要任何状态字段。
// 所有逻辑都在 `ToolHandler` trait 的 `call` 实现中。

#[derive(Debug)]
struct BashHandler;

impl ToolHandler for BashHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        // 模式匹配 `params` 切片，支持两种调用方式：
        // 1. 传入一个 JSON 对象（比如 {"command": "ls"}）
        // 2. 直接传入字符串参数
        let command = match &params[..] {
            [ToolParameter::Json(v)] => get_json_param(v, "command"),
            [ToolParameter::String(cmd)] => cmd.clone(),
            _ => "".to_string(),
        };

        if command.is_empty() {
            return Err("Missing command parameter".to_string());
        }

        Ok(BasicTool::run_bash(&command))
    }
}

// =============================================================================
// ReadHandler：读取文件内容
// =============================================================================
// 通过 `run_read` 实现，支持可选的 `limit` 参数限制返回行数

#[derive(Debug)]
struct ReadHandler;

impl ToolHandler for ReadHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (path, limit) = match &params[..] {
            [ToolParameter::Json(v)] => {
                let path = get_json_param(v, "path");
                let limit = v.get("limit").and_then(|x| x.as_u64()).map(|n| n as usize);
                (path, limit)
            }
            [ToolParameter::String(p), ToolParameter::Integer(l)] => (p.clone(), Some(*l as usize)),
            [ToolParameter::String(p)] => (p.clone(), None),
            _ => ("".to_string(), None),
        };

        if path.is_empty() {
            return Err("Missing path parameter".to_string());
        }

        BasicTool::run_read(&path, limit)
    }
}

// =============================================================================
// WriteHandler：写入文件内容
// =============================================================================
// 如果文件所在目录不存在，`run_write` 内部会自动 `create_dir_all`

#[derive(Debug)]
struct WriteHandler;

impl ToolHandler for WriteHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (path, content) = match &params[..] {
            [ToolParameter::Json(v)] => (get_json_param(v, "path"), get_json_param(v, "content")),
            [ToolParameter::String(p), ToolParameter::String(c)] => (p.clone(), c.clone()),
            _ => ("".to_string(), "".to_string()),
        };

        if path.is_empty() {
            return Err("Missing path parameter".to_string());
        }

        BasicTool::run_write(&path, &content)
    }
}

// =============================================================================
// EditHandler：编辑文件内容（文本替换）
// =============================================================================
// 要求 `old_text` 必须在文件中存在，且只替换第一次出现的位置

#[derive(Debug)]
struct EditHandler;

impl ToolHandler for EditHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (path, old_text, new_text) = match &params[..] {
            [ToolParameter::Json(v)] => (
                get_json_param(v, "path"),
                get_json_param(v, "old_text"),
                get_json_param(v, "new_text"),
            ),
            [ToolParameter::String(p), ToolParameter::String(o), ToolParameter::String(n)] => {
                (p.clone(), o.clone(), n.clone())
            }
            _ => ("".to_string(), "".to_string(), "".to_string()),
        };

        if path.is_empty() {
            return Err("Missing path parameter".to_string());
        }

        BasicTool::run_edit(&path, &old_text, &new_text)
    }
}

// =============================================================================
// WebFetchHandler：获取网页并转为 Markdown
// =============================================================================

#[derive(Debug)]
struct WebFetchHandler;

impl ToolHandler for WebFetchHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let url = match &params[..] {
            [ToolParameter::Json(v)] => get_json_param(v, "url"),
            [ToolParameter::String(u)] => u.clone(),
            _ => "".to_string(),
        };

        if url.is_empty() {
            return Err("Missing url parameter".to_string());
        }

        BasicTool::run_web_fetch(&url)
    }
}

// =============================================================================
// GrepHandler：递归搜索目录下匹配正则的文件内容
// =============================================================================

#[derive(Debug)]
struct GrepHandler;

impl ToolHandler for GrepHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (dir, pattern) = match &params[..] {
            [ToolParameter::Json(v)] => (get_json_param(v, "dir"), get_json_param(v, "pattern")),
            [ToolParameter::String(d), ToolParameter::String(p)] => (d.clone(), p.clone()),
            _ => ("".to_string(), "".to_string()),
        };

        if dir.is_empty() {
            return Err("Missing dir parameter".to_string());
        }
        if pattern.is_empty() {
            return Err("Missing pattern parameter".to_string());
        }

        BasicTool::run_grep(&dir, &pattern)
    }
}

// =============================================================================
// GlobHandler：使用 glob 模式搜索文件
// =============================================================================

#[derive(Debug)]
struct GlobHandler;

impl ToolHandler for GlobHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (pattern, dir) = match &params[..] {
            [ToolParameter::Json(v)] => {
                let pattern = get_json_param(v, "pattern");
                let dir = v.get("dir").and_then(|x| x.as_str()).map(|s| s.to_string());
                (pattern, dir)
            }
            [ToolParameter::String(p)] => (p.clone(), None),
            [ToolParameter::String(p), ToolParameter::String(d)] => (p.clone(), Some(d.clone())),
            _ => ("".to_string(), None),
        };

        if pattern.is_empty() {
            return Err("Missing pattern parameter".to_string());
        }

        BasicTool::run_glob(&pattern, dir.as_deref())
    }
}

// =============================================================================
// UseSkillHandler：按需加载 Skill 内容
// =============================================================================
// 允许 Agent 在运行时通过名称或 ID 加载 Skill 的完整说明内容。

#[derive(Debug)]
struct UseSkillHandler;

impl ToolHandler for UseSkillHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let name = match &params[..] {
            [ToolParameter::Json(v)] => v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            [ToolParameter::String(n)] => n.clone(),
            _ => "".to_string(),
        };
        if name.is_empty() {
            return Err("Missing name parameter".to_string());
        }
        crate::skills::load_skill_content(&name)
    }
}

// =============================================================================
// CreateTaskPlanHandler：将复杂任务拆分为子任务计划
// =============================================================================

#[derive(Debug)]
struct CreateTaskPlanHandler;

impl ToolHandler for CreateTaskPlanHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let input = match &params[..] {
            [ToolParameter::Json(v)] => v.clone(),
            _ => return Err("Expected JSON parameters".to_string()),
        };

        // 兼容某些模型将 JSON 数组序列化为字符串传递的情况
        let tasks_value = input
            .get("tasks")
            .ok_or("Missing or invalid 'tasks' array")?;
        let tasks_arr: serde_json::Value = if let Some(arr) = tasks_value.as_array() {
            serde_json::Value::Array(arr.clone())
        } else if let Some(s) = tasks_value.as_str() {
            serde_json::from_str(s).map_err(|_| "Missing or invalid 'tasks' array".to_string())?
        } else {
            return Err("Missing or invalid 'tasks' array".to_string());
        };
        let tasks_arr = tasks_arr
            .as_array()
            .ok_or("Missing or invalid 'tasks' array")?;

        let mut plan = task::TaskPlan::new("");
        for (idx, task_val) in tasks_arr.iter().enumerate() {
            let name = task_val.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = task_val
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }
            plan.tasks
                .push(task::Task::new(format!("{}", idx + 1), name, description));
        }

        let json =
            serde_json::to_string(&plan).map_err(|e| format!("Serialize plan failed: {}", e))?;
        Ok(json)
    }
}

// =============================================================================
// HandleTaskPlanHandler：将复杂任务拆分为子任务并自动执行
// =============================================================================
// 实际执行逻辑在 tool_call 中通过异步拦截完成，这里只注册占位。

#[derive(Debug)]
struct HandleTaskPlanHandler;

impl ToolHandler for HandleTaskPlanHandler {
    fn call(&self, _name: &str, _params: ToolParams) -> Result<String, String> {
        Err("This tool is handled internally by async executor".to_string())
    }
}

// =============================================================================
// AskForQuestionHandler：向用户询问问题并获取答案
// =============================================================================

#[derive(Debug)]
struct AskForQuestionHandler;

impl ToolHandler for AskForQuestionHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        Err("AskForQuestion handled in tool_call".to_string())
    }
}

// =============================================================================
// MCP Manager 全局状态
// =============================================================================
// `McpManager` 在程序启动后由 `main.rs` 异步初始化并设置到这里。
// 运行期间只读（通过 `Arc` 共享），因此 `RwLock` 仅用于初始设置。

static MCP_MANAGER: RwLock<Option<Arc<McpManager>>> = RwLock::new(None);

pub fn set_mcp_manager(manager: Arc<McpManager>) {
    let mut lock = MCP_MANAGER.write().unwrap();
    *lock = Some(manager);
}

pub fn get_mcp_manager() -> Option<Arc<McpManager>> {
    MCP_MANAGER.read().unwrap().clone()
}

// =============================================================================
// Task Provider 全局状态
// =============================================================================
// `Provider` 在程序启动后由 entry.rs 或 server 设置，供 handle_task_plan 工具使用。

static TASK_PROVIDER: RwLock<Option<Arc<RwLock<Provider>>>> = RwLock::new(None);

pub fn set_task_provider(provider: Arc<RwLock<Provider>>) {
    let mut lock = TASK_PROVIDER.write().unwrap();
    *lock = Some(provider);
}

pub fn get_task_provider() -> Option<Arc<RwLock<Provider>>> {
    TASK_PROVIDER.read().unwrap().clone()
}

// =============================================================================
// 全局注册表：LazyLock 实现懒加载的单例
// =============================================================================
// `LazyLock` 保证这段初始化代码只会在首次访问 `REGISTRY` 时执行一次，
// 且执行过程是线程安全的。
//
// `static` 变量默认对外不可见（没有 `pub`），因此其他模块无法直接操作注册表，
// 只能通过我们暴露的 `init_tools()`、`tool_schema()`、`tool_call()` 来间接使用。

static REGISTRY: LazyLock<ToolsRegistry> = LazyLock::new(|| {
    let mut registry = ToolsRegistry::new();
    registry
        .register(
            "bash",
            "Run a shell command in the current workspace.",
            r#"{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}"#,
            Box::new(BashHandler),
        )
        .expect("register bash tool failed");
    registry
        .register(
            "read",
            "Read the contents of a file.",
            r#"{"type":"object","properties":{"path":{"type":"string"},"limit":{"type":"integer"}},"required":["path"]}"#,
            Box::new(ReadHandler),
        )
        .expect("register read tool failed");
    registry
        .register(
            "write",
            "Write content to a file.",
            r#"{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}"#,
            Box::new(WriteHandler),
        )
        .expect("register write tool failed");
    registry
        .register(
            "edit",
            "Replace old_text with new_text in a file.",
            r#"{"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"]}"#,
            Box::new(EditHandler),
        )
        .expect("register edit tool failed");
    registry
        .register(
            "web_fetch",
            "Fetch a web page by URL and convert HTML to Markdown.",
            r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}"#,
            Box::new(WebFetchHandler),
        )
        .expect("register web_fetch tool failed");
    registry
        .register(
            "grep",
            "Recursively search files in a directory using a regex pattern.",
            r#"{"type":"object","properties":{"dir":{"type":"string"},"pattern":{"type":"string"}},"required":["dir","pattern"]}"#,
            Box::new(GrepHandler),
        )
        .expect("register grep tool failed");
    registry
        .register(
            "glob",
            "使用 glob 模式搜索文件，支持 *、**、?、[] 等模式",
            r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Glob 模式，如 **/*.rs、src/**/*、*.md"},"dir":{"type":"string","description":"可选，搜索根目录，默认为当前工作目录"}},"required":["pattern"]}"#,
            Box::new(GlobHandler),
        )
        .expect("register glob tool failed");
    registry
        .register(
            "use_skill",
            "Load a skill by name or ID to inject its instructions into the conversation.",
            r#"{"type":"object","properties":{"name":{"type":"string","description":"Skill name or full ID (e.g., 'commit' or 'claude-commit')"}},"required":["name"]}"#,
            Box::new(UseSkillHandler),
        )
        .expect("register use_skill tool failed");
    registry
        .register(
            "create_task_plan",
            "将复杂任务拆分为多个子任务。仅在任务确实复杂、需要多步骤完成时调用。参数示例：{\"tasks\":[{\"name\":\"分析代码\",\"description\":\"分析现有错误处理模式\"}]}",
            r#"{"type":"object","properties":{"tasks":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"description":{"type":"string"}},"required":["name","description"]}}},"required":["tasks"]}"#,
            Box::new(CreateTaskPlanHandler),
        )
        .expect("register create_task_plan tool failed");
    registry
        .register(
            "handle_task_plan",
            "将复杂任务拆分为多个子任务并自动执行。仅在任务确实复杂、需要多步骤完成时调用。工具会返回所有子任务的执行结果汇总。参数示例：{\"tasks\":[{\"name\":\"分析代码\",\"description\":\"分析现有错误处理模式\"}]}",
            r#"{"type":"object","properties":{"tasks":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"description":{"type":"string"}},"required":["name","description"]}}},"required":["tasks"]}"#,
            Box::new(HandleTaskPlanHandler),
        )
        .expect("register handle_task_plan tool failed");
    registry
        .register(
            "ask_for_question",
            "Ask the user a question with predefined options",
            r#"{"type":"object","properties":{"question":{"type":"string"},"options":{"type":"array","maxItems":3,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"description":{"type":"string"}},"required":["id","label"]}},"recommended":{"type":"string"},"allow_custom":{"type":"boolean","default":true}},"required":["question","options"]}"#,
            Box::new(AskForQuestionHandler),
        )
        .expect("register ask_for_question failed");
    registry
});

// =============================================================================
// 显式触发工具注册表的初始化
// =============================================================================
// `LazyLock::force` 强制立即执行 `LazyLock` 的初始化闭包。
// 虽然首次访问 `REGISTRY` 时也会自动初始化，但显式调用 `init_tools()`
// 可以在程序启动阶段就把所有错误（如注册失败）暴露出来，而不是延迟到首次调用工具时。

pub fn init_tools() {
    let _ = LazyLock::force(&REGISTRY);
}

// =============================================================================
// 生成工具的 JSON Schema
// =============================================================================
// 现在 schema 完全从注册表里动态生成，新增工具时不需要再手动维护这段代码。
// MCP 工具的轻量 schema（name + description，input_schema 为空）也在这里合并。

pub async fn tool_schema() -> serde_json::Value {
    let mut schemas = Vec::new();

    // basic_tools：完整 schema（从注册表获取）
    let basic = REGISTRY.tool_schema();
    if let Some(arr) = basic.as_array() {
        schemas.extend(arr.iter().cloned());
    }

    // mcp_tools：轻量 schema（仅 name + description，input_schema 为空对象）
    let mcp = MCP_MANAGER.read().ok().and_then(|lock| lock.clone());
    if let Some(mcp) = mcp {
        for (full_name, desc) in mcp.tools_list().await {
            schemas.push(serde_json::json!({
                "name": full_name,
                "description": desc,
                "input_schema": serde_json::Value::Object(serde_json::Map::new()),
            }));
        }
    }

    serde_json::Value::Array(schemas)
}

// =============================================================================
// 生成 Subagent 可用的工具 schema（不含 create_task_plan，避免递归拆分）
// =============================================================================

pub async fn subagent_tool_schema() -> serde_json::Value {
    let mut schemas = Vec::new();

    let basic = REGISTRY.tool_schema();
    if let Some(arr) = basic.as_array() {
        for tool in arr {
            let is_excluded = tool
                .get("name")
                .and_then(|n| n.as_str())
                .map(|name| name == "create_task_plan" || name == "handle_task_plan")
                .unwrap_or(false);
            if is_excluded {
                continue;
            }
            schemas.push(tool.clone());
        }
    }

    let mcp = MCP_MANAGER.read().ok().and_then(|lock| lock.clone());
    if let Some(mcp) = mcp {
        for (full_name, desc) in mcp.tools_list().await {
            schemas.push(serde_json::json!({
                "name": full_name,
                "description": desc,
                "input_schema": serde_json::Value::Object(serde_json::Map::new()),
            }));
        }
    }

    serde_json::Value::Array(schemas)
}

// =============================================================================
// 列出所有已注册的工具
// =============================================================================
// 返回格式为 `name: description`，每行一个工具

pub fn tool_list() -> String {
    REGISTRY.list_tools().unwrap_or_default()
}

// =============================================================================
// 执行单个工具调用（异步方法）
// =============================================================================
// 将调用方传入的 HashMap 参数打包成 `ToolParameter::Json`，
// 通过注册表分发给对应的 handler。
// 支持 MCP 工具（mcp: 前缀）和本地工具。

pub async fn tool_call(
    name: &str,
    input: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    if name == "ask_for_question" {
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or("Missing question parameter")?
            .to_string();

        let options_json = input
            .get("options")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid options parameter")?;

        let options: Vec<crate::tui::event::QuestionOption> = options_json
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        if options.is_empty() || options.len() > 3 {
            return Err("Options count must be between 1 and 3".to_string());
        }

        let recommended = input
            .get("recommended")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let allow_custom = input
            .get("allow_custom")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channel = QUESTION_CHANNEL.lock().unwrap();
            *channel = Some(tx);
        }

        let event_tx_opt = { EVENT_TX.read().unwrap().clone() };
        if let Some(event_tx) = event_tx_opt {
            let _ = event_tx
                .send(AppEvent::ShowQuestionDialog {
                    question,
                    options,
                    recommended,
                    allow_custom,
                })
                .await;
        }

        match rx.await {
            Ok(answer) => {
                let result = serde_json::to_string(&answer)
                    .map_err(|e| format!("Serialize error: {}", e))?;
                return Ok(result);
            }
            Err(_) => return Err("No answer received".to_string()),
        }
    }
    if name == "handle_task_plan" {
        let provider = get_task_provider().ok_or("Task provider not initialized")?;
        return task::tool::execute_handle_task_plan(provider, input).await;
    }

    if name.starts_with("mcp:") {
        let input_json = serde_json::to_value(input).unwrap_or_default();
        let mcp = get_mcp_manager().ok_or("MCP manager not initialized".to_string())?;
        match mcp.tool_call(name, input_json).await {
            Ok(result) => {
                let texts: Vec<String> = result.content.iter().map(|c| c.text.clone()).collect();
                Ok(texts.join("\n"))
            }
            Err(e) => Err(format!("MCP call failed: {}", e)),
        }
    } else {
        let input_json = serde_json::to_value(input).unwrap_or_default();
        let params = vec![ToolParameter::Json(input_json)];
        REGISTRY.call(name, params)
    }
}

// =============================================================================
// 批量执行工具调用（处理 Part 列表）
// =============================================================================
// 遍历 LLM 返回的 `Part`，如果是 `ToolUse` 类型，就逐个调用 `tool_call`。
// 返回的 `Part::ToolResult` 列表将被打包为 User 消息回传给模型。
//
// 设计演进：此前返回的是裸 JSON Value 数组；为了与新的 `Message`/`Part` 模型对齐，
// 现在直接返回结构化的 `Vec<Part>`，省去上层再做一次格式转换。
// 因 MCP 调用需要异步，此函数已升级为 `async`。

async fn execute_single_tool_call(
    id: &str,
    name: &str,
    arguments: &serde_json::Value,
) -> (String, bool) {
    let input: HashMap<String, serde_json::Value> = match arguments {
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => HashMap::new(),
    };

    match tool_call(name, &input).await {
        Ok(output) => {
            log_trace!(
                "execute_tool_call raw output | name={} | output={}",
                name,
                output
            );
            log_debug!(
                "execute_tool_call success | name={} | output_len={}",
                name,
                output.len()
            );
            (output, false)
        }
        Err(e) => {
            log_trace!("execute_tool_call raw error | name={} | err={}", name, e);
            log_debug!("execute_tool_call error | name={} | err={}", name, e);
            (format!("Error: {}", e), true)
        }
    }
}

pub async fn execute_tool_calls(
    parts: &[Part],
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Vec<Part> {
    use colored::Colorize;
    use futures::future::join_all;
    use crate::server::transport::sse::SseEvent;
    use std::sync::{Arc, Mutex};

    // 将回调提取到 Arc<Mutex<...>> 中，以便在并行的 async 块之间安全共享
    let callback = on_tool_event.take();
    let shared_cb = Arc::new(Mutex::new(callback));

    let futures: Vec<_> = parts
        .iter()
        .filter_map(|part| {
            let Part::ToolUse {
                id,
                name,
                arguments,
            } = part
            else {
                return None;
            };

            let id = id.clone();
            let name = name.clone();
            let arguments = arguments.clone();
            let cb = shared_cb.clone();
            Some(async move {
                log_info!("calling tool: ${}", name);
                log_debug!("execute_tool_call | name={} | args={}", name, arguments);
                let (content, is_error) = execute_single_tool_call(&id, &name, &arguments).await;
                
                // Parse JSON content to extract diff if present
                let (display_content, diff, is_new_file) = if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if json_val.get("diff").is_some() {
                        (
                            json_val["content"].as_str().unwrap_or(&content).to_string(),
                            json_val["diff"].as_str().map(|s| s.to_string()),
                            json_val["is_new_file"].as_bool().unwrap_or(false),
                        )
                    } else {
                        (content.clone(), None, false)
                    }
                } else {
                    (content.clone(), None, false)
                };
                
                if let Ok(mut guard) = cb.lock() {
                    if let Some(ref mut callback) = *guard {
                        let _ = callback(SseEvent::ToolResult {
                            tool_use_id: id.clone(),
                            content: display_content,
                            diff,
                            is_new_file,
                        });
                    }
                }
                
                Part::ToolResult {
                    tool_call_id: id,
                    content,
                    is_error,
                }
            })
        })
        .collect();

    join_all(futures).await
}

// =============================================================================
// 单元测试
// =============================================================================
// 测试所有已注册工具的可用性，以及注册表的 list_tools 功能

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试注册表中所有工具都已正确注册
    #[test]
    fn test_list_tools() {
        let list = REGISTRY.list_tools().unwrap();
        assert!(list.contains("bash"), "registry should contain bash tool");
        assert!(list.contains("read"), "registry should contain read tool");
        assert!(list.contains("write"), "registry should contain write tool");
        assert!(list.contains("edit"), "registry should contain edit tool");
        assert!(
            list.contains("web_fetch"),
            "registry should contain web_fetch tool"
        );
        assert!(list.contains("grep"), "registry should contain grep tool");
        assert!(list.contains("glob"), "registry should contain glob tool");
    }

    /// 测试 use_skill 工具已注册
    #[test]
    fn test_list_tools_includes_use_skill() {
        let list = REGISTRY.list_tools().unwrap();
        assert!(
            list.contains("use_skill"),
            "registry should contain use_skill tool, got: {}",
            list
        );
    }

    /// 测试通过 tool_call 调用 bash 工具
    #[tokio::test]
    async fn test_tool_call_bash() {
        let mut input = HashMap::new();
        input.insert(
            "command".to_string(),
            serde_json::json!("echo hello_registry"),
        );

        let result = tool_call("bash", &input).await.unwrap();
        assert!(
            result.contains("hello_registry"),
            "bash output should contain 'hello_registry', got: {}",
            result
        );
    }

    /// 测试通过 tool_call 调用 read 工具
    #[tokio::test]
    async fn test_tool_call_read() {
        let mut input = HashMap::new();
        input.insert("path".to_string(), serde_json::json!("src/tools/mod.rs"));

        let result = tool_call("read", &input).await.unwrap();
        assert!(
            result.contains("tool_call"),
            "read output should contain 'tool_call', got: {}",
            result
        );
    }

    /// 测试通过 tool_call 调用 write 和 edit 工具
    #[tokio::test]
    async fn test_tool_call_write_and_edit() {
        let path = "target/test_tool_call_write.txt";

        // 1. 调用 write 工具创建文件
        let mut write_input = HashMap::new();
        write_input.insert("path".to_string(), serde_json::json!(path));
        write_input.insert("content".to_string(), serde_json::json!("hello world"));

        let write_result = tool_call("write", &write_input).await.unwrap();
        assert!(
            write_result.contains("Wrote"),
            "write output should contain 'Wrote', got: {}",
            write_result
        );

        // 2. 调用 edit 工具修改文件内容
        let mut edit_input = HashMap::new();
        edit_input.insert("path".to_string(), serde_json::json!(path));
        edit_input.insert("old_text".to_string(), serde_json::json!("world"));
        edit_input.insert("new_text".to_string(), serde_json::json!("rust"));

        let edit_result = tool_call("edit", &edit_input).await.unwrap();
        assert!(
            edit_result.contains("Edited"),
            "edit output should contain 'Edited', got: {}",
            edit_result
        );

        // 3. 调用 read 工具验证修改结果
        let mut read_input = HashMap::new();
        read_input.insert("path".to_string(), serde_json::json!(path));

        let read_result = tool_call("read", &read_input).await.unwrap();
        assert!(
            read_result.contains("hello rust"),
            "read output should contain 'hello rust', got: {}",
            read_result
        );

        // 清理临时文件
        let _ = std::fs::remove_file(path);
    }

    /// 测试通过 tool_call 调用 grep 工具
    #[tokio::test]
    async fn test_tool_call_grep() {
        let mut input = HashMap::new();
        input.insert("dir".to_string(), serde_json::json!("src/tools"));
        input.insert("pattern".to_string(), serde_json::json!("run_read"));

        let result = tool_call("grep", &input).await.unwrap();
        assert!(
            result.contains("run_read"),
            "grep output should contain 'run_read', got: {}",
            result
        );
    }

    /// 测试通过 tool_call 调用 glob 工具
    #[tokio::test]
    async fn test_tool_call_glob() {
        let mut input = HashMap::new();
        input.insert("pattern".to_string(), serde_json::json!("**/Cargo.toml"));
        let result = tool_call("glob", &input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Cargo.toml"));
    }

    /// 测试 web_fetch 工具参数缺失时返回错误
    #[tokio::test]
    async fn test_tool_call_web_fetch_missing_url() {
        let input = HashMap::new();
        let result = tool_call("web_fetch", &input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing url"));
    }

    /// 测试调用不存在的工具会返回错误
    #[tokio::test]
    async fn test_tool_call_not_found() {
        let input = HashMap::new();
        let result = tool_call("non_existent_tool", &input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    /// 测试调用 use_skill 工具但 Skill 不存在时返回错误
    #[tokio::test]
    async fn test_tool_call_use_skill_not_found() {
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            serde_json::json!("nonexistent-skill-abc"),
        );

        let result = tool_call("use_skill", &input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not found"),
            "error should contain 'not found', got: {}",
            err
        );
    }

    /// 测试 create_task_plan handler 解析任务列表
    #[test]
    fn test_create_task_plan_handler() {
        use crate::tools::tools_type::{ToolHandler, ToolParameter};
        use serde_json::json;

        let handler = CreateTaskPlanHandler;
        let input = json!({
            "tasks": [
                {"name": "Analyze", "description": "Analyze current code"},
                {"name": "Refactor", "description": "Refactor errors"}
            ]
        });
        let result = handler.call("create_task_plan", vec![ToolParameter::Json(input)]);
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("Analyze"));
        assert!(json_str.contains("Refactor"));
    }

    /// 测试 create_task_plan handler 兼容字符串形式的 tasks 数组
    #[test]
    fn test_create_task_plan_handler_string_tasks() {
        use crate::tools::tools_type::{ToolHandler, ToolParameter};
        use serde_json::json;

        let handler = CreateTaskPlanHandler;
        let input = json!({
            "tasks": "[{\"name\": \"Test\", \"description\": \"Test desc\"}]"
        });
        let result = handler.call("create_task_plan", vec![ToolParameter::Json(input)]);
        assert!(result.is_ok());
        let json_str = result.unwrap();
        assert!(json_str.contains("Test"));
        assert!(json_str.contains("Test desc"));
    }

    /// 测试 subagent_tool_schema 不包含 create_task_plan
    #[tokio::test]
    async fn test_subagent_tool_schema_excludes_create_task_plan() {
        let schema = subagent_tool_schema().await;
        let arr = schema.as_array().unwrap();
        let has_task_plan = arr
            .iter()
            .any(|v| v.get("name").and_then(|n| n.as_str()) == Some("create_task_plan"));
        assert!(
            !has_task_plan,
            "subagent schema should not contain create_task_plan"
        );
    }

    /// 测试 subagent_tool_schema 不包含 handle_task_plan
    #[tokio::test]
    async fn test_subagent_tool_schema_excludes_handle_task_plan() {
        let schema = subagent_tool_schema().await;
        let arr = schema.as_array().unwrap();
        let has_handle_task = arr
            .iter()
            .any(|v| v.get("name").and_then(|n| n.as_str()) == Some("handle_task_plan"));
        assert!(
            !has_handle_task,
            "subagent schema should not contain handle_task_plan"
        );
    }

    /// 测试注册表包含 handle_task_plan
    #[test]
    fn test_list_tools_includes_handle_task_plan() {
        let list = REGISTRY.list_tools().unwrap();
        assert!(
            list.contains("handle_task_plan"),
            "registry should contain handle_task_plan tool, got: {}",
            list
        );
    }

    /// MCP 端到端验证：tool_schema 合并 + tool_call 路由
    #[tokio::test]
    async fn test_mcp_end_to_end() {
        use crate::config::models::{McpServerConfig, McpServerType};
        use crate::mcp::manager::McpManager;
        use std::collections::HashMap;

        // 检查 npx 是否可用
        if std::process::Command::new("npx")
            .arg("--version")
            .output()
            .is_err()
        {
            panic!(
                "npx is not available in PATH. \
                 MCP end-to-end test requires Node.js/npm to install the mock server."
            );
        }

        // 初始化 mock MCP 服务器（通过 npx 安装并启动）
        let mut config = HashMap::new();
        config.insert(
            "mock".to_string(),
            McpServerConfig {
                server_type: McpServerType::Local,
                enabled: true,
                command: Some(vec![
                    "npx".to_string(),
                    "-y".to_string(),
                    "@modelcontextprotocol/server-everything".to_string(),
                ]),
                url: None,
                headers: None,
            },
        );
        let manager = McpManager::from_config(&config).await.unwrap();
        set_mcp_manager(std::sync::Arc::new(manager));

        // 1. 验证 tool_schema 合并了 MCP 工具（server-everything 提供 12 个工具）
        let schema = tool_schema().await;
        let arr = schema.as_array().unwrap();
        let mcp_tools: Vec<_> = arr
            .iter()
            .filter(|v| {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.starts_with("mcp:"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            mcp_tools.len() >= 2,
            "expected at least 2 mcp tools in schema, got: {}",
            mcp_tools.len()
        );
        assert!(mcp_tools
            .iter()
            .any(|v| { v.get("name").and_then(|n| n.as_str()) == Some("mcp:mock/echo") }));

        // 2. 验证 tool_call 正确路由到 MCP
        let mut input = HashMap::new();
        input.insert("message".to_string(), serde_json::json!("world"));
        let result = tool_call("mcp:mock/echo", &input).await.unwrap();
        assert!(result.contains("Echo: world"), "got: {}", result);

        // 3. 验证本地工具不受影响
        let mut input = HashMap::new();
        input.insert("command".to_string(), serde_json::json!("echo hello_local"));
        let result = tool_call("bash", &input).await.unwrap();
        assert!(result.contains("hello_local"));
    }
}
