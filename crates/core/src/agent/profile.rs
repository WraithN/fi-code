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

// AgentProfile 与 ToolFilter：为不同 Agent 类型提供声明式的工具过滤与提示词后缀配置

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use fi_code_shared::dto::AgentType;
use serde_json::Value;

/// 工具过滤器，用于控制 Agent 可访问的工具集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolFilter {
    /// 白名单模式：仅允许列表中的工具。
    AllowList(HashSet<String>),
    /// 黑名单模式：允许除列表外的所有工具。
    BlockList(HashSet<String>),
}

impl ToolFilter {
    /// 检查单个工具名称是否被允许。
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            ToolFilter::AllowList(set) => set.contains(tool_name),
            ToolFilter::BlockList(set) => !set.contains(tool_name),
        }
    }

    /// 对一个工具 schema 数组进行过滤，返回仅包含被允许工具的新数组。
    ///
    /// 输入 `tools_schema` 应为 JSON Array，每个元素包含 `"name"` 字段。
    /// 如果不是数组，则返回原值。
    pub fn apply(&self, tools_schema: &Value) -> Value {
        let Some(arr) = tools_schema.as_array() else {
            return tools_schema.clone();
        };

        let filtered: Vec<Value> = arr
            .iter()
            .filter(|tool| {
                tool.get("name")
                    .and_then(|n| n.as_str())
                    .map(|name| self.allows(name))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        Value::Array(filtered)
    }
}

/// Agent 配置画像，包含名称、提示词后缀、工具过滤器和任务执行权限。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    /// Agent 类型名称（如 `"Build"`、`"Plan"`）。
    pub name: &'static str,
    /// 追加到系统提示词末尾的后缀文本。
    pub prompt_suffix: &'static str,
    /// 该 Agent 可使用的工具过滤器。
    pub tool_filter: ToolFilter,
    /// 该 Agent 是否可以执行子任务（如 `handle_task_plan` 中的子任务）。
    pub can_execute_tasks: bool,
}

impl AgentProfile {
    /// 根据 `AgentType` 获取对应的静态 `AgentProfile`。
    pub fn for_type(agent_type: AgentType) -> &'static Self {
        static PROFILES: LazyLock<HashMap<AgentType, AgentProfile>> = LazyLock::new(|| {
            let mut map = HashMap::new();

            // Build Agent：全功能，允许所有本地工具
            let build_tools: HashSet<String> = [
                "bash",
                "read",
                "read_file",
                "write",
                "edit",
                "grep",
                "glob",
                "web_fetch",
                "git",
                "git_status",
                "git_diff",
                "git_add",
                "git_commit",
                "git_log",
                "git_worktree",
                "create_task_plan",
                "handle_task_plan",
                "ask_for_question",
                "use_skill",
            ]
            .iter()
            .map(|&s| s.to_string())
            .collect();

            map.insert(
                AgentType::Build,
                AgentProfile {
                    name: "Build",
                    prompt_suffix: "\n\n## Agent Mode: Build\nYou are a full-featured coding assistant. You can read and write files, execute shell commands, manage Git operations, and perform any task necessary to help the user with their project.",
                    tool_filter: ToolFilter::AllowList(build_tools),
                    can_execute_tasks: true,
                },
            );

            // Plan Agent：只读规划模式
            let plan_tools: HashSet<String> = [
                "read",
                "read_file",
                "grep",
                "glob",
                "git_status",
                "git_log",
                "git_diff",
                "web_fetch",
                "create_task_plan",
                "handle_task_plan",
                "ask_for_question",
            ]
            .iter()
            .map(|&s| s.to_string())
            .collect();

            map.insert(
                AgentType::Plan,
                AgentProfile {
                    name: "Plan",
                    prompt_suffix: "\n\n## Agent Mode: Plan\nYou are a planning assistant. You can only read code and materials, but you cannot modify files or execute commands. Your task is to analyze requirements, examine the codebase, and produce detailed implementation plans. When using create_task_plan or handle_task_plan, you should create the plan and mark it complete, but do not actually execute the sub-tasks.",
                    tool_filter: ToolFilter::AllowList(plan_tools),
                    can_execute_tasks: false,
                },
            );

            map
        });

        PROFILES
            .get(&agent_type)
            .expect("AgentProfile for_type: unknown agent type")
    }
}

// =============================================================================
// 单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_allow_list() {
        let allow_list: HashSet<String> = ["read".to_string(), "grep".to_string()]
            .into_iter()
            .collect();
        let filter = ToolFilter::AllowList(allow_list);

        assert!(filter.allows("read"));
        assert!(filter.allows("grep"));
        assert!(!filter.allows("write"));
        assert!(!filter.allows("bash"));
    }

    #[test]
    fn test_tool_filter_apply() {
        let schema = serde_json::json!([
            {"name": "bash", "description": "Run shell command"},
            {"name": "read", "description": "Read file"},
            {"name": "write", "description": "Write file"},
            {"name": "grep", "description": "Search files"}
        ]);

        // AllowList 仅保留白名单工具
        let allow_list: HashSet<String> = ["read".to_string(), "grep".to_string()]
            .into_iter()
            .collect();
        let allow_filter = ToolFilter::AllowList(allow_list);
        let allowed = allow_filter.apply(&schema);
        let allowed_names: Vec<&str> = allowed
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.get("name").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(allowed_names, vec!["read", "grep"]);

        // BlockList 排除黑名单工具
        let block_list: HashSet<String> = ["bash".to_string(), "write".to_string()]
            .into_iter()
            .collect();
        let block_filter = ToolFilter::BlockList(block_list);
        let blocked = block_filter.apply(&schema);
        let blocked_names: Vec<&str> = blocked
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.get("name").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(blocked_names, vec!["read", "grep"]);
    }

    #[test]
    fn test_profile_for_build() {
        let profile = AgentProfile::for_type(AgentType::Build);

        assert_eq!(profile.name, "Build");
        assert!(profile.prompt_suffix.contains("Agent Mode: Build"));
        assert!(profile.can_execute_tasks);

        // Build Agent 允许所有列出的工具
        assert!(profile.tool_filter.allows("bash"));
        assert!(profile.tool_filter.allows("read"));
        assert!(profile.tool_filter.allows("read_file"));
        assert!(profile.tool_filter.allows("write"));
        assert!(profile.tool_filter.allows("edit"));
        assert!(profile.tool_filter.allows("grep"));
        assert!(profile.tool_filter.allows("glob"));
        assert!(profile.tool_filter.allows("web_fetch"));
        assert!(profile.tool_filter.allows("git"));
        assert!(profile.tool_filter.allows("git_status"));
        assert!(profile.tool_filter.allows("git_diff"));
        assert!(profile.tool_filter.allows("git_add"));
        assert!(profile.tool_filter.allows("git_commit"));
        assert!(profile.tool_filter.allows("git_log"));
        assert!(profile.tool_filter.allows("git_worktree"));
        assert!(profile.tool_filter.allows("create_task_plan"));
        assert!(profile.tool_filter.allows("handle_task_plan"));
        assert!(profile.tool_filter.allows("ask_for_question"));
        assert!(profile.tool_filter.allows("use_skill"));
    }

    #[test]
    fn test_profile_for_plan() {
        let profile = AgentProfile::for_type(AgentType::Plan);

        assert_eq!(profile.name, "Plan");
        assert!(profile.prompt_suffix.contains("Agent Mode: Plan"));
        assert!(!profile.can_execute_tasks);

        // Plan Agent 仅允许只读和规划工具
        assert!(profile.tool_filter.allows("read"));
        assert!(profile.tool_filter.allows("read_file"));
        assert!(profile.tool_filter.allows("grep"));
        assert!(profile.tool_filter.allows("glob"));
        assert!(profile.tool_filter.allows("git_status"));
        assert!(profile.tool_filter.allows("git_log"));
        assert!(profile.tool_filter.allows("git_diff"));
        assert!(profile.tool_filter.allows("web_fetch"));
        assert!(profile.tool_filter.allows("create_task_plan"));
        assert!(profile.tool_filter.allows("handle_task_plan"));

        // Plan Agent 不允许写操作和执行命令
        assert!(!profile.tool_filter.allows("bash"));
        assert!(!profile.tool_filter.allows("write"));
        assert!(!profile.tool_filter.allows("edit"));
        assert!(!profile.tool_filter.allows("git_add"));
        assert!(!profile.tool_filter.allows("git_commit"));
        assert!(profile.tool_filter.allows("ask_for_question"));
        assert!(!profile.tool_filter.allows("use_skill"));
    }
}
