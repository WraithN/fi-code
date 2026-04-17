use crate::log_debug;
use crate::log_trace;
use crate::session::message::Part;
use std::collections::HashMap;
use std::sync::LazyLock;

// =============================================================================
// Rust 基础概念：模块声明
// =============================================================================
// `pub mod` 声明当前模块包含的子模块，Rust 编译器会在同级目录下查找同名文件
// 例如 `basic_tools` 对应 `src/tools/basic_tools.rs`

pub mod basic_tools;
pub mod tools_registry;
pub mod tools_type;

// =============================================================================
// 模块内部导入
// =============================================================================
// `use` 把其他模块中的类型引入当前作用域，避免每次写全限定路径

use basic_tools::BasicTool;
use tools_registry::ToolsRegistry;
use tools_type::{ToolHandler, ToolParameter, ToolParams};

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
            "use_skill",
            "Load a skill by name or ID to inject its instructions into the conversation.",
            r#"{"type":"object","properties":{"name":{"type":"string","description":"Skill name or full ID (e.g., 'commit' or 'claude-commit')"}},"required":["name"]}"#,
            Box::new(UseSkillHandler),
        )
        .expect("register use_skill tool failed");
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

pub fn tool_schema() -> serde_json::Value {
    REGISTRY.tool_schema()
}

// =============================================================================
// 列出所有已注册的工具
// =============================================================================
// 返回格式为 `name: description`，每行一个工具

pub fn tool_list() -> String {
    REGISTRY.list_tools().unwrap_or_default()
}

// =============================================================================
// 执行单个工具调用（同步方法）
// =============================================================================
// 将调用方传入的 HashMap 参数打包成 `ToolParameter::Json`，
// 通过注册表分发给对应的 handler。

pub fn tool_call(name: &str, input: &HashMap<String, serde_json::Value>) -> Result<String, String> {
    let input_json = serde_json::to_value(input).unwrap_or_default();
    let params = vec![ToolParameter::Json(input_json)];
    REGISTRY.call(name, params)
}

// =============================================================================
// 批量执行工具调用（处理 Part 列表）
// =============================================================================
// 遍历 LLM 返回的 `Part`，如果是 `ToolUse` 类型，就逐个调用 `tool_call`。
// 返回的 `Part::ToolResult` 列表将被打包为 User 消息回传给模型。
//
// 设计演进：此前返回的是裸 JSON Value 数组；为了与新的 `Message`/`Part` 模型对齐，
// 现在直接返回结构化的 `Vec<Part>`，省去上层再做一次格式转换。

pub fn execute_tool_calls(parts: &[Part]) -> Vec<Part> {
    use colored::Colorize;

    let mut results = Vec::new();

    for part in parts {
        // 只处理 ToolUse 类型的 Part
        if let Part::ToolUse {
            id,
            name,
            arguments,
        } = part
        {
            println!("{}", format!("${}", name).yellow());
            log_debug!("execute_tool_call | name={} | args={}", name, arguments);

            // `arguments` 是 serde_json::Value，需要转成 HashMap 才能传给 `tool_call`
            let input: HashMap<String, serde_json::Value> = match arguments {
                serde_json::Value::Object(map) => {
                    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                }
                _ => HashMap::new(),
            };

            let (content, is_error) = match tool_call(name, &input) {
                Ok(output) => {
                    log_trace!(
                        "execute_tool_call raw output | name={} | output={}",
                        name,
                        output
                    );
                    println!("{}", &output[..output.len().min(200)]);
                    log_debug!(
                        "execute_tool_call success | name={} | output_len={}",
                        name,
                        output.len()
                    );
                    (output, false)
                }
                Err(e) => {
                    log_trace!("execute_tool_call raw error | name={} | err={}", name, e);
                    eprintln!("Tool call error: {}", e);
                    log_debug!("execute_tool_call error | name={} | err={}", name, e);
                    (format!("Error: {}", e), true)
                }
            };

            results.push(Part::ToolResult {
                tool_call_id: id.clone(),
                content,
                is_error,
            });
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
    #[test]
    fn test_tool_call_bash() {
        let mut input = HashMap::new();
        input.insert(
            "command".to_string(),
            serde_json::json!("echo hello_registry"),
        );

        let result = tool_call("bash", &input).unwrap();
        assert!(
            result.contains("hello_registry"),
            "bash output should contain 'hello_registry', got: {}",
            result
        );
    }

    /// 测试通过 tool_call 调用 read 工具
    #[test]
    fn test_tool_call_read() {
        let mut input = HashMap::new();
        input.insert("path".to_string(), serde_json::json!("src/tools/mod.rs"));

        let result = tool_call("read", &input).unwrap();
        assert!(
            result.contains("tool_call"),
            "read output should contain 'tool_call', got: {}",
            result
        );
    }

    /// 测试通过 tool_call 调用 write 和 edit 工具
    #[test]
    fn test_tool_call_write_and_edit() {
        let path = "target/test_tool_call_write.txt";

        // 1. 调用 write 工具创建文件
        let mut write_input = HashMap::new();
        write_input.insert("path".to_string(), serde_json::json!(path));
        write_input.insert("content".to_string(), serde_json::json!("hello world"));

        let write_result = tool_call("write", &write_input).unwrap();
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

        let edit_result = tool_call("edit", &edit_input).unwrap();
        assert!(
            edit_result.contains("Edited"),
            "edit output should contain 'Edited', got: {}",
            edit_result
        );

        // 3. 调用 read 工具验证修改结果
        let mut read_input = HashMap::new();
        read_input.insert("path".to_string(), serde_json::json!(path));

        let read_result = tool_call("read", &read_input).unwrap();
        assert!(
            read_result.contains("hello rust"),
            "read output should contain 'hello rust', got: {}",
            read_result
        );

        // 清理临时文件
        let _ = std::fs::remove_file(path);
    }

    /// 测试通过 tool_call 调用 grep 工具
    #[test]
    fn test_tool_call_grep() {
        let mut input = HashMap::new();
        input.insert("dir".to_string(), serde_json::json!("src/tools"));
        input.insert("pattern".to_string(), serde_json::json!("run_read"));

        let result = tool_call("grep", &input).unwrap();
        assert!(
            result.contains("run_read"),
            "grep output should contain 'run_read', got: {}",
            result
        );
    }

    /// 测试 web_fetch 工具参数缺失时返回错误
    #[test]
    fn test_tool_call_web_fetch_missing_url() {
        let input = HashMap::new();
        let result = tool_call("web_fetch", &input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing url"));
    }

    /// 测试调用不存在的工具会返回错误
    #[test]
    fn test_tool_call_not_found() {
        let input = HashMap::new();
        let result = tool_call("non_existent_tool", &input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    /// 测试调用 use_skill 工具但 Skill 不存在时返回错误
    #[test]
    fn test_tool_call_use_skill_not_found() {
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            serde_json::json!("nonexistent-skill-abc"),
        );

        let result = tool_call("use_skill", &input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not found"),
            "error should contain 'not found', got: {}",
            err
        );
    }
}
