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
use crate::tui_event::{AppEvent, QuestionAnswer};
use fi_code_shared::constants::*;
use fi_code_shared::dto::AgentType;
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
use crate::utils::file_type::file_type_from_path;

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
// GitHandler：通用 git 命令执行
// =============================================================================

#[derive(Debug)]
struct GitHandler;

impl ToolHandler for GitHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (command, args_opt) = match &params[..] {
            [ToolParameter::Json(v)] => {
                let cmd = get_json_param(v, "command");
                let args = v.get("args").and_then(|a| a.as_array());
                (cmd, args)
            }
            [ToolParameter::String(cmd)] => (cmd.clone(), None),
            _ => ("".to_string(), None),
        };

        if command.is_empty() {
            return Err("Missing command parameter".to_string());
        }

        let mut args_vec = vec![command.as_str()];
        if let Some(args) = args_opt {
            for arg in args {
                if let Some(s) = arg.as_str() {
                    args_vec.push(s);
                }
            }
        }

        Ok(BasicTool::run_git_command(&args_vec))
    }
}

#[derive(Debug)]
struct GitStatusHandler;

impl ToolHandler for GitStatusHandler {
    fn call(&self, _name: &str, _params: ToolParams) -> Result<String, String> {
        Ok(BasicTool::run_git_status())
    }
}

#[derive(Debug)]
struct GitDiffHandler;

impl ToolHandler for GitDiffHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let path = match &params[..] {
            [ToolParameter::Json(v)] => v.get("path").and_then(|p| p.as_str()),
            [ToolParameter::String(p)] => Some(p.as_str()),
            _ => None,
        };

        Ok(BasicTool::run_git_diff(path))
    }
}

#[derive(Debug)]
struct GitAddHandler;

impl ToolHandler for GitAddHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let files: Vec<String> = match &params[..] {
            [ToolParameter::Json(v)] => {
                if let Some(arr) = v.get("files").and_then(|a| a.as_array()) {
                    arr.iter()
                        .filter_map(|f| f.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    vec![]
                }
            }
            [ToolParameter::String(f)] => vec![f.clone()],
            _ => vec![],
        };

        if files.is_empty() {
            return Err("Missing files parameter".to_string());
        }

        let files_str: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        Ok(BasicTool::run_git_add(&files_str))
    }
}

#[derive(Debug)]
struct GitCommitHandler;

impl ToolHandler for GitCommitHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let message = match &params[..] {
            [ToolParameter::Json(v)] => get_json_param(v, "message"),
            [ToolParameter::String(m)] => m.clone(),
            _ => "".to_string(),
        };

        if message.is_empty() {
            return Err("Missing message parameter".to_string());
        }

        Ok(BasicTool::run_git_commit(&message))
    }
}

#[derive(Debug)]
struct GitLogHandler;

impl ToolHandler for GitLogHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let limit = match &params[..] {
            [ToolParameter::Json(v)] => v.get("limit").and_then(|l| l.as_u64()).map(|l| l as usize),
            [ToolParameter::Integer(l)] => Some(*l as usize),
            _ => None,
        };

        Ok(BasicTool::run_git_log(limit))
    }
}

#[derive(Debug)]
struct GitWorktreeHandler;

impl ToolHandler for GitWorktreeHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let (command, args_opt) = match &params[..] {
            [ToolParameter::Json(v)] => {
                let cmd = get_json_param(v, "command");
                let args = v.get("args").and_then(|a| a.as_array());
                (cmd, args)
            }
            [ToolParameter::String(cmd)] => (cmd.clone(), None),
            _ => ("".to_string(), None),
        };

        if command.is_empty() {
            return Err("Missing command parameter".to_string());
        }

        let mut args_vec = vec![command.as_str()];
        if let Some(args) = args_opt {
            for arg in args {
                if let Some(s) = arg.as_str() {
                    args_vec.push(s);
                }
            }
        }

        Ok(BasicTool::run_git_worktree(&args_vec))
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
            "将任务拆分为多个子任务并自动串行执行。当你需要执行多个步骤（如先查找文件、再分析内容、再修改代码）时，必须使用此工具一次性完成所有步骤。禁止自己手动逐个执行，因为每轮对话只能执行有限个工具，手动执行会导致任务中断。工具会返回所有子任务的执行结果汇总。参数示例：{\"tasks\":[{\"name\":\"分析代码\",\"description\":\"分析现有错误处理模式\"}]}",
            r#"{"type":"object","properties":{"tasks":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"description":{"type":"string"}},"required":["name","description"]}}},"required":["tasks"]}"#,
            Box::new(HandleTaskPlanHandler),
        )
        .expect("register handle_task_plan tool failed");
    registry
        .register(
            "ask_for_question",
            "Ask the user a question with predefined options. You MUST call this tool when you encounter any of the following situations instead of guessing on your own: (1) Path/file ambiguity: the file path provided by the user matches multiple files, or the path does not exist but there are multiple similar candidates; (2) Unclear intent: the user's instruction has multiple interpretations and the choice will affect subsequent implementation; (3) Destructive operations: before deleting files, modifying configuration files, or executing commands like rm/reset/drop; (4) Cross-module impact: modifying one file may affect 3+ other modules. Provide 2-5 most relevant options, mark the recommended one, and allow custom answers.",
            r#"{"type":"object","properties":{"question":{"type":"string","description":"The question to ask the user. Be specific about the ambiguity or risk."},"options":{"type":"array","maxItems":5,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"description":{"type":"string"}},"required":["id","label"]}},"recommended":{"type":"string","description":"The ID of the recommended option"},"allow_custom":{"type":"boolean","default":true,"description":"Whether to allow the user to enter a custom answer"}},"required":["question","options"]}"#,
            Box::new(AskForQuestionHandler),
        )
        .expect("register ask_for_question failed");
    registry
        .register(
            "git",
            "Generic git tool, execute any git command",
            r#"{"type":"object","properties":{"command":{"type":"string","description":"Git command to execute"},"args":{"type":"array","items":{"type":"string"},"description":"Optional arguments to the git command"}},"required":["command"]}"#,
            Box::new(GitHandler),
        )
        .expect("register git tool failed");
    registry
        .register(
            "git_status",
            "Get git status",
            r#"{"type":"object","properties":{}}"#,
            Box::new(GitStatusHandler),
        )
        .expect("register git_status tool failed");
    registry
        .register(
            "git_diff",
            "Show git diff",
            r#"{"type":"object","properties":{"path":{"type":"string","description":"Optional path to show diff for"}},"required":[]}"#,
            Box::new(GitDiffHandler),
        )
        .expect("register git_diff tool failed");
    registry
        .register(
            "git_add",
            "Add files to git staging area",
            r#"{"type":"object","properties":{"files":{"type":"array","items":{"type":"string"},"description":"Files to add"}},"required":["files"]}"#,
            Box::new(GitAddHandler),
        )
        .expect("register git_add tool failed");
    registry
        .register(
            "git_commit",
            "Commit staged changes",
            r#"{"type":"object","properties":{"message":{"type":"string","description":"Commit message"}},"required":["message"]}"#,
            Box::new(GitCommitHandler),
        )
        .expect("register git_commit tool failed");
    registry
        .register(
            "git_log",
            "Show git commit history",
            r#"{"type":"object","properties":{"limit":{"type":"integer","description":"Limit number of commits to show"}},"required":[]}"#,
            Box::new(GitLogHandler),
        )
        .expect("register git_log tool failed");
    registry
        .register(
            "git_worktree",
            "Manage git worktrees",
            r#"{"type":"object","properties":{"command":{"type":"string","description":"Worktree command (add, list, remove, etc.)"},"args":{"type":"array","items":{"type":"string"},"description":"Optional arguments to the worktree command"}},"required":["command"]}"#,
            Box::new(GitWorktreeHandler),
        )
        .expect("register git_worktree tool failed");
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

/// 获取指定 Agent 类型的工具 schema（已过滤）。
pub async fn tool_schema_for_agent(agent_type: AgentType) -> serde_json::Value {
    use crate::agent::profile::AgentProfile;
    let all = tool_schema().await;
    let profile = AgentProfile::for_type(agent_type);
    profile.tool_filter.apply(&all)
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

        let options: Vec<crate::tui_event::QuestionOption> = options_json
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        if options.is_empty() || options.len() > 5 {
            return Err("Options count must be between 1 and 5".to_string());
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

// MAX_TOOL_RETRIES, TOOL_RETRY_DELAY_MS 已从 fi_code_shared::constants 导入

async fn execute_single_tool_call(
    id: &str,
    name: &str,
    arguments: &serde_json::Value,
) -> (String, bool, u64) {
    let start = std::time::Instant::now();
    // =============================================================================
    // 前置检查：参数 JSON 完整性校验
    // =============================================================================
    // 如果 arguments 包含 `_raw` 字段，说明上游 SSE 解析时 JSON 不完整
    //（如 tool_calls 的增量参数还没收齐）。此时直接返回格式错误，
    // 而不是让工具 Handler 拿到空参数后报错，这样 LLM 能更清楚地知道
    // 需要重新生成完整参数。
    if let Some(raw) = arguments.get("_raw").and_then(|v| v.as_str()) {
        let error_msg = if raw.is_empty() {
            format!(
                "Error: 工具 '{}' 被调用但未收到任何参数。请重新调用此工具并提供完整参数。",
                name
            )
        } else {
            format!(
                "Error: 工具 '{}' 的参数 JSON 解析失败或不完整。收到的原始参数片段: {}\n\
                 请重新调用此工具，并确保提供完整且格式正确的 JSON 参数。",
                name, raw
            )
        };
        log_debug!(
            "execute_tool_call param_incomplete | name={} | raw={}",
            name,
            raw
        );
        return (error_msg, true, start.elapsed().as_millis() as u64);
    }

    let input: HashMap<String, serde_json::Value> = match arguments {
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => HashMap::new(),
    };

    // =============================================================================
    // Agent 层重试：对非参数类错误自动重试最多 MAX_TOOL_RETRIES 次
    // =============================================================================
    // 参数类错误（Missing xxx、JSON 解析失败）不重试，因为重试不会改变结果。
    // 其他错误（如网络超时、文件锁、临时 IO 错误）重试可能成功。
    let mut last_error = String::new();
    for attempt in 0..=MAX_TOOL_RETRIES {
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
                let elapsed = start.elapsed().as_millis() as u64;
                return (output, false, elapsed);
            }
            Err(e) => {
                log_trace!(
                    "execute_tool_call raw error | name={} | attempt={}/{} | err={}",
                    name,
                    attempt,
                    MAX_TOOL_RETRIES,
                    e
                );

                // 参数类错误：立即返回，不重试
                if e.starts_with("Missing") || e.starts_with("参数 JSON 解析失败") {
                    let enhanced_error = format!(
                        "Error: {}\n\
                         请检查工具 '{}' 的参数要求，确保提供了所有必需参数，然后重新调用此工具。",
                        e, name
                    );
                    return (enhanced_error, true, start.elapsed().as_millis() as u64);
                }

                last_error = e;
                if attempt < MAX_TOOL_RETRIES {
                    log_debug!(
                        "execute_tool_call retry | name={} | attempt={}/{} | delay={}ms",
                        name,
                        attempt + 1,
                        MAX_TOOL_RETRIES,
                        TOOL_RETRY_DELAY_MS
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(TOOL_RETRY_DELAY_MS))
                        .await;
                }
            }
        }
    }

    log_debug!(
        "execute_tool_call final error | name={} | retries_exhausted | err={}",
        name,
        last_error
    );
    (
        format!("Error: {} (已重试 {} 次)", last_error, MAX_TOOL_RETRIES),
        true,
        start.elapsed().as_millis() as u64,
    )
}

pub async fn execute_tool_calls(
    parts: &[Part],
    agent_type: fi_code_shared::dto::AgentType,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    is_aggressive: bool,
    // 父级 trace context（通常来自 TurnSpan）：每个 ToolSpan 以此为父
    parent_cx: Option<&opentelemetry::Context>,
) -> Vec<Part> {
    use crate::agent::profile::AgentProfile;
    use crate::server::transport::sse::SseEvent;
    use colored::Colorize;
    use futures::future::join_all;
    use std::sync::{Arc, Mutex};

    let profile = AgentProfile::for_type(agent_type);

    // 判断当前模式：有 SSE 回调为 Web/Server/TUI 模式，否则为 CLI 模式
    let is_web_mode = on_tool_event.is_some();

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
            let is_allowed = profile.tool_filter.allows(&name);
            let agent_name = profile.name;
            let is_web = is_web_mode;
            // 为每个工具调用复制父 context（Context 是 Clone-friendly）
            let parent_cx_cloned: Option<opentelemetry::Context> = parent_cx.cloned();
            Some(async move {
                // 二次拦截：检查该工具是否被当前 Agent 允许
                if !is_allowed {
                    let error_part = Part::ToolError {
                        tool_call_id: id.clone(),
                        content: format!("Tool '{}' is not allowed in {} Agent", name, agent_name),
                        error_message: "Permission denied by agent profile".to_string(),
                        for_context_only: false,
                    };
                    if let Ok(mut guard) = cb.lock() {
                        if let Some(ref mut callback) = *guard {
                            let _ = callback(SseEvent::Part {
                                part: error_part.clone(),
                            });
                        }
                    }
                    return vec![error_part];
                }

                // =============================================================================
                // 权限检查：系统级权限校验（Allow / Ask / Deny）
                // =============================================================================
                let input: HashMap<String, serde_json::Value> = match &arguments {
                    serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    _ => HashMap::new(),
                };
                let (action, risk, reason) = crate::permission::PermissionAction::match_action(&name, &input);

                // 启动 ToolSpan：以 Turn 为父，记录 tool_name / id / args
                // 必须在权限检查之前启动，以便在 Ask 流程中调用 add_permission_event 记录用户决策事件
                let tool_span = crate::observability::otel::start_tool_span(
                    parent_cx_cloned.as_ref(),
                    &name,
                    &id,
                    &serde_json::to_string(&arguments).unwrap_or_default(),
                );

                match action {
                    crate::permission::PermissionAction::Deny => {
                        let error_msg = format!("Permission denied: {}", reason);
                        log_debug!("execute_tool_call denied | name={} | reason={}", name, reason);
                        let error_part = Part::ToolError {
                            tool_call_id: id.clone(),
                            content: error_msg.clone(),
                            error_message: error_msg,
                            for_context_only: false,
                        };
                        if let Ok(mut guard) = cb.lock() {
                            if let Some(ref mut callback) = *guard {
                                let _ = callback(SseEvent::Part {
                                    part: error_part.clone(),
                                });
                            }
                        }
                        return vec![error_part];
                    }
                    crate::permission::PermissionAction::Allow => {
                        // Allow 级别直接放行，继续执行
                    }
                    crate::permission::PermissionAction::Ask => {
                        if is_web {
                            // Web/Server/TUI 模式：发送 PermissionAsk SSE 事件并等待用户确认
                            log_debug!("execute_tool_call ask (web) | name={} | risk={:?} | reason={}", name, risk, reason);
                            if let Ok(mut guard) = cb.lock() {
                                if let Some(ref mut callback) = *guard {
                                    let _ = callback(SseEvent::PermissionAsk {
                                        tool_call_id: id.clone(),
                                        tool_name: name.clone(),
                                        risk: format!("{:?}", risk),
                                        reason: reason.clone(),
                                    });
                                }
                            }
                            // 记录权限询问开始时间，用于上报到 ToolSpan 的 permission_ask 事件
                            let ask_start = std::time::Instant::now();
                            match crate::permission::wait_permission_response(&id, &name, risk, &reason).await {
                                Ok(true) => {
                                    tool_span.add_permission_event(
                                        "approved",
                                        true,
                                        ask_start.elapsed().as_millis() as u64,
                                    );
                                    log_debug!("execute_tool_call approved | name={} | id={}", name, id);
                                }
                                Ok(false) => {
                                    tool_span.add_permission_event(
                                        "rejected",
                                        false,
                                        ask_start.elapsed().as_millis() as u64,
                                    );
                                    let error_msg = "Permission denied: user rejected".to_string();
                                    log_debug!("execute_tool_call rejected | name={} | id={}", name, id);
                                    let error_part = Part::ToolError {
                                        tool_call_id: id.clone(),
                                        content: error_msg.clone(),
                                        error_message: error_msg,
                                        for_context_only: false,
                                    };
                                    if let Ok(mut guard) = cb.lock() {
                                        if let Some(ref mut callback) = *guard {
                                            let _ = callback(SseEvent::Part {
                                                part: error_part.clone(),
                                            });
                                        }
                                    }
                                    return vec![error_part];
                                }
                                Err(e) => {
                                    tool_span.add_permission_event(
                                        "timeout",
                                        false,
                                        ask_start.elapsed().as_millis() as u64,
                                    );
                                    let error_msg = format!("Permission error: {}", e);
                                    log_debug!("execute_tool_call permission error | name={} | err={}", name, e);
                                    let error_part = Part::ToolError {
                                        tool_call_id: id.clone(),
                                        content: error_msg.clone(),
                                        error_message: error_msg,
                                        for_context_only: false,
                                    };
                                    if let Ok(mut guard) = cb.lock() {
                                        if let Some(ref mut callback) = *guard {
                                            let _ = callback(SseEvent::Part {
                                                part: error_part.clone(),
                                            });
                                        }
                                    }
                                    return vec![error_part];
                                }
                            }
                        } else {
                            // CLI 模式：使用 check_cli（默认拒绝，--dangerous 时通过）
                            if let Err(err) = crate::permission::PermissionChecker::check_cli(&name, &input) {
                                log_debug!("execute_tool_call denied (cli) | name={} | err={}", name, err);
                                let error_part = Part::ToolError {
                                    tool_call_id: id.clone(),
                                    content: err.clone(),
                                    error_message: err,
                                    for_context_only: false,
                                };
                                return vec![error_part];
                            }
                        }
                    }
                }

                log_info!("calling tool: ${}", name);
                log_debug!("execute_tool_call | name={} | args={}", name, arguments);
                let (content, is_error, duration_ms) = execute_single_tool_call(&id, &name, &arguments).await;
                // 记录工具执行结果到 ToolSpan（drop 时自动结束 span）
                // 注意：tool_span 已在权限检查前启动（位于 input 解析之后），以便 Ask 流程上报 permission_ask 事件
                tool_span.record_result(&content, is_error);

                // 从参数中提取路径（用于 read/write/edit）
                let input: HashMap<String, serde_json::Value> = match &arguments {
                    serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    _ => HashMap::new(),
                };
                let file_path = input.get("path").and_then(|v| v.as_str());
                let language = file_path.and_then(file_type_from_path);
                let is_read_write_edit = name == "read" || name == "write" || name == "edit";

                // 构造元数据标题文本
                let display_content = if name == "read" {
                    let line_count = content.lines().count();
                    let char_count = content.chars().count();
                    format!(
                        "✓ read: {} | {} lines | {} chars ({}ms)",
                        file_path.unwrap_or("unknown"),
                        line_count,
                        char_count,
                        duration_ms
                    )
                } else if name == "write" {
                    let is_new = content.contains("New file:");
                    if is_new {
                        format!("✓ write: {} ({}ms) — 新增文件", file_path.unwrap_or("unknown"), duration_ms)
                    } else {
                        format!("✓ write: {} ({}ms)", file_path.unwrap_or("unknown"), duration_ms)
                    }
                } else if name == "edit" {
                    format!("✓ edit: {} ({}ms)", file_path.unwrap_or("unknown"), duration_ms)
                } else {
                    content.clone()
                };

                if let Ok(mut guard) = cb.lock() {
                    if let Some(ref mut callback) = *guard {
                        if is_error {
                            // 错误时发送 ToolError
                            let _ = callback(SseEvent::Part {
                                part: Part::ToolError {
                                    tool_call_id: id.clone(),
                                    content: content.clone(),
                                    error_message: content.clone(),
                                    for_context_only: false,
                                },
                            });
                        } else {
                            log_info!(
                                "[Tools] sending ToolResult SSE | id={} | display_len={}",
                                id,
                                display_content.len()
                            );
                            
                            // 构建工具结果元数据
                            let line_count = content.lines().count();
                            let byte_count = content.len();
                            
                            // 快速判断是否会被压缩，而不是真正调用 compress_tool_result（性能问题！）
                            let threshold = match name.as_str() {
                                "bash" => fi_code_shared::constants::BASH_COMPRESS_THRESHOLD,
                                "read" => fi_code_shared::constants::READ_COMPRESS_THRESHOLD,
                                _ => fi_code_shared::constants::DEFAULT_COMPRESS_THRESHOLD,
                            };
                            let is_compressed = byte_count > threshold as usize;
                            
                            let metadata = serde_json::json!({
                                "tool_name": name,
                                "tool_call_id": id,
                                "line_count": line_count,
                                "byte_count": byte_count,
                                "compressed": is_compressed,
                                "truncated": content.len() > 50000,
                                "content_type": if is_read_write_edit { "file" } else { "text" },
                            });
                            
                             // 正常时发送 ToolResult（元数据标题）
                            let _ = callback(SseEvent::Part {
                                part: Part::ToolResult {
                                    tool_call_id: id.clone(),
                                    content: display_content.clone(),
                                    duration_ms: Some(duration_ms),
                                    metadata: Some(metadata),
                                    for_context_only: false,
                                },
                            });

                            // 对于 read/write/edit，额外发送 CodeBlock（实际内容）
                            if is_read_write_edit {
                                let is_meta_only = content.starts_with("New file:")
                                    || content.starts_with("Wrote ")
                                    || content.starts_with("Edited ")
                                    || content.starts_with("Error:");
                                if !is_meta_only {
                                    let _ = callback(SseEvent::Part {
                                        part: Part::CodeBlock {
                                            language: language.clone().unwrap_or_default(),
                                            code: content.clone(),
                                            for_context_only: false,
                                        },
                                    });
                                }
                            }
                        }
                    }
                }

                let mut parts = Vec::new();
                
                if is_error {
                    parts.push(Part::ToolError {
                        tool_call_id: id,
                        content: content.clone(),
                        error_message: content,
                        for_context_only: true,
                    });
                } else {
                    let compressed = crate::agent::compression::compress_tool_result(&content, is_aggressive, Some(&name));
                    
                    // 构建工具结果元数据
                    let line_count = content.lines().count();
                    let byte_count = content.len();
                    let is_compressed = compressed != content;
                    
                    let metadata = serde_json::json!({
                        "tool_name": name,
                        "tool_call_id": id,
                        "line_count": line_count,
                        "byte_count": byte_count,
                        "compressed": is_compressed,
                        "truncated": content.len() > 50000,
                        "content_type": if is_read_write_edit { "file" } else { "text" },
                    });
                    
                    parts.push(Part::ToolResult {
                        tool_call_id: id.clone(),
                        content: display_content,
                        duration_ms: Some(duration_ms),
                        metadata: Some(metadata),
                        for_context_only: true,
                    });
                    
                    // 对于 read/write/edit，额外返回 CodeBlock（用于保存到会话历史）
                    if is_read_write_edit {
                        let is_meta_only = content.starts_with("New file:")
                            || content.starts_with("Wrote ")
                            || content.starts_with("Edited ")
                            || content.starts_with("Error:");
                        if !is_meta_only {
                            parts.push(Part::CodeBlock {
                                language: language.unwrap_or_default(),
                                code: content.clone(),
                                for_context_only: true,
                            });
                        }
                    }
                }
                
                parts
            })
        })
        .collect();

    let results = join_all(futures).await.into_iter().flatten().collect();

    // 恢复 on_tool_event 回调，以便调用方可以继续使用它进行后续的 SSE 发送
    // 安全地将回调恢复到原位置，避免 Poison 错误
    match Arc::try_unwrap(shared_cb) {
        Ok(mutex) => {
            match mutex.into_inner() {
                Ok(mut callback) => {
                    if let Some(cb) = callback.take() {
                        *on_tool_event = Some(cb);
                    }
                }
                Err(_) => {
                    log_debug!("Failed to restore SSE callback due to lock poisoning");
                }
            }
        }
        Err(_) => {
            log_debug!("Could not restore SSE callback, references still held");
        }
    }

    results
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
        assert!(list.contains("git"), "registry should contain git tool");
        assert!(
            list.contains("git_status"),
            "registry should contain git_status tool"
        );
        assert!(
            list.contains("git_diff"),
            "registry should contain git_diff tool"
        );
        assert!(
            list.contains("git_add"),
            "registry should contain git_add tool"
        );
        assert!(
            list.contains("git_commit"),
            "registry should contain git_commit tool"
        );
        assert!(
            list.contains("git_log"),
            "registry should contain git_log tool"
        );
        assert!(
            list.contains("git_worktree"),
            "registry should contain git_worktree tool"
        );
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
        let path = format!("target/test_tool_call_write_{}.txt", std::process::id());

        // 1. 调用 write 工具创建文件
        let mut write_input = HashMap::new();
        write_input.insert("path".to_string(), serde_json::json!(path));
        write_input.insert("content".to_string(), serde_json::json!("hello world"));

        let write_result = tool_call("write", &write_input).await.unwrap();
        assert!(
            write_result.contains("New file") || write_result.contains("Wrote"),
            "write output should contain 'New file' or 'Wrote', got: {}",
            write_result
        );

        // 2. 调用 edit 工具修改文件内容
        let mut edit_input = HashMap::new();
        edit_input.insert("path".to_string(), serde_json::json!(path));
        edit_input.insert("old_text".to_string(), serde_json::json!("world"));
        edit_input.insert("new_text".to_string(), serde_json::json!("rust"));

        let edit_result = tool_call("edit", &edit_input).await.unwrap();
        assert!(
            edit_result.contains('+') || edit_result.contains("Edited"),
            "edit output should contain diff markers, got: {}",
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

    /// 测试通过 tool_call 调用 git_status 工具
    #[tokio::test]
    async fn test_tool_call_git_status() {
        let result = tool_call("git_status", &HashMap::new()).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    /// 测试通过 tool_call 调用 git_log 工具
    #[tokio::test]
    async fn test_tool_call_git_log() {
        let mut input = HashMap::new();
        input.insert("limit".to_string(), serde_json::json!(3));
        let result = tool_call("git_log", &input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    /// 测试通过 tool_call 调用通用 git 工具
    #[tokio::test]
    async fn test_tool_call_git() {
        let mut input = HashMap::new();
        input.insert("command".to_string(), serde_json::json!("status"));
        let result = tool_call("git", &input).await;
        assert!(result.is_ok());
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

    /// 测试 web_fetch 工具成功获取网页内容
    #[tokio::test]
    async fn test_tool_call_web_fetch_success() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body>Hello from wiremock</body></html>"),
            )
            .mount(&mock_server)
            .await;

        let mut input = HashMap::new();
        input.insert("url".to_string(), serde_json::json!(mock_server.uri()));
        let result = tool_call("web_fetch", &input).await;
        assert!(
            result.is_ok(),
            "web_fetch should succeed, got: {:?}",
            result
        );
        let output = result.unwrap();
        assert!(
            output.contains("Hello from wiremock"),
            "web_fetch output should contain page content, got: {}",
            output
        );
    }

    /// 测试 git_diff 工具调用
    #[tokio::test]
    async fn test_tool_call_git_diff() {
        let mut input = HashMap::new();
        input.insert("path".to_string(), serde_json::json!("Cargo.toml"));
        let result = tool_call("git_diff", &input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // 输出可能是空（没有未提交的更改）或非空（有更改）
        // 我们只检查没有错误即可
        assert!(
            !output.contains("fatal:"),
            "git_diff should not contain fatal error"
        );
    }

    /// 测试 git_add 工具调用
    #[tokio::test]
    async fn test_tool_call_git_add() {
        let mut input = HashMap::new();
        input.insert("files".to_string(), serde_json::json!(["Cargo.toml"]));
        let result = tool_call("git_add", &input).await;
        assert!(result.is_ok(), "git_add should succeed for tracked file");
    }

    /// 测试 git_commit 工具调用
    #[tokio::test]
    async fn test_tool_call_git_commit() {
        let mut input = HashMap::new();
        input.insert(
            "message".to_string(),
            serde_json::json!("test commit from unit test"),
        );
        let result = tool_call("git_commit", &input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // git commit 在没有 staged 更改时可能返回提示信息，也可能因锁竞争返回错误
        // 我们只验证返回了有效的输出（非空或包含提示信息）
        assert!(
            !output.is_empty(),
            "git_commit should return some output, got empty"
        );
    }

    /// 测试 git_worktree 工具调用
    #[tokio::test]
    async fn test_tool_call_git_worktree() {
        let mut input = HashMap::new();
        input.insert("command".to_string(), serde_json::json!("list"));
        let result = tool_call("git_worktree", &input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            !output.is_empty(),
            "git_worktree should return non-empty output"
        );
    }

    /// 测试 bash 工具执行包含空格的命令
    #[tokio::test]
    async fn test_tool_call_bash_with_spaces() {
        let mut input = HashMap::new();
        input.insert("command".to_string(), serde_json::json!("echo hello world"));
        let result = tool_call("bash", &input).await;
        assert!(result.is_ok(), "bash should succeed");
        let output = result.unwrap();
        assert!(
            output.contains("hello world"),
            "bash output should contain 'hello world', got: {}",
            output
        );
    }

    /// 测试 ask_for_question 工具缺少参数时返回错误
    #[tokio::test]
    async fn test_tool_call_ask_for_question_missing_params() {
        let input = HashMap::new();
        let result = tool_call("ask_for_question", &input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Missing question parameter"),
            "error should mention missing question, got: {}",
            err
        );
    }

    /// 测试 use_skill 工具缺少 name 参数
    #[tokio::test]
    async fn test_tool_call_use_skill_missing_name() {
        let input = HashMap::new();
        let result = tool_call("use_skill", &input).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Missing name parameter"),
            "error should mention missing name, got: {}",
            err
        );
    }

    /// 测试 Plan Agent 会拦截不允许的工具调用
    #[tokio::test]
    async fn test_execute_tool_calls_plan_agent_blocks_write() {
        use fi_code_shared::dto::AgentType;
        let parts = vec![
            Part::ToolUse {
                id: "1".to_string(),
                name: "write".to_string(),
                arguments: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
            },
        ];
        let results = execute_tool_calls(&parts, AgentType::Plan, &mut None, false, None).await;
        assert_eq!(results.len(), 1);
        match &results[0] {
            Part::ToolError { error_message, .. } => {
                assert!(error_message.contains("Permission denied"));
            }
            _ => panic!("Expected ToolError for blocked tool"),
        }
    }

    /// 测试 Build Agent 不会拦截 write 工具
    #[tokio::test]
    async fn test_execute_tool_calls_build_agent_allows_write() {
        use fi_code_shared::dto::AgentType;
        // 开启 dangerous 模式以通过系统权限检查（CLI 模式下 Ask 默认拒绝）
        crate::permission::set_cli_dangerous(true);
        let parts = vec![
            Part::ToolUse {
                id: "1".to_string(),
                name: "write".to_string(),
                arguments: serde_json::json!({"path": "/tmp/test_fi_code_build.txt", "content": "hello"}),
            },
        ];
        let results = execute_tool_calls(&parts, AgentType::Build, &mut None, false, None).await;
        // 恢复默认值
        crate::permission::set_cli_dangerous(false);
        assert_eq!(results.len(), 1);
        // Build Agent 应该尝试执行（结果可能是成功或失败，但不是因为被拦截）
        match &results[0] {
            Part::ToolError { error_message, .. } => {
                assert!(!error_message.contains("Permission denied by agent profile"));
                assert!(!error_message.contains("Permission denied:"));
            }
            _ => {} // ToolResult 或其他结果都可以
        }
    }
}
