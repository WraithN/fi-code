// =============================================================================
// prompt 模块：系统提示词构建器
// =============================================================================
// 负责根据可用工具 schema 动态组装 System Prompt，让 Agent 明确自身能力边界。

use crate::skills::SkillRegistry;

const PROMPT_TEMPLATE: &str = r#"You are an autonomous coding assistant running in a terminal environment.

Your mission is to help the user with software engineering tasks by reasoning step-by-step, taking action when necessary, and reporting results clearly.

You have access to the following tools (described in JSON Schema):
{tools_schema}

Rules:
1. Analyze the user's request carefully before acting.
2. If a task requires file inspection, use `read` or `grep`.
3. If a task requires changing files, use `write` or `edit`.
4. If a task requires running commands (builds, tests, etc.), use `bash`.
5. When you need to fetch documentation from the web, use `web_fetch`.
6. Always prefer concrete actions over long explanations.
7. When you invoke a tool, wait for its result before proceeding to the next step.
8. If no tool is needed, reply directly to the user in a concise and helpful manner.
"#;

/// 系统提示词构建器。
pub struct PromptBuilder;

impl PromptBuilder {
    /// 创建一个新的提示词构建器。
    pub fn new() -> Self {
        Self
    }

    /// 根据工具 JSON Schema 和可用 Skills 构建系统提示词。
    ///
    /// # Arguments
    /// * `tools_schema` - 工具的 JSON Schema 描述
    /// * `registry` - Skill 注册表，用于在提示词末尾列出可用的 Skills
    pub fn build(&self, tools_schema: &serde_json::Value, registry: &SkillRegistry) -> String {
        let tools_str = serde_json::to_string_pretty(tools_schema).unwrap_or_default();
        let mut prompt = PROMPT_TEMPLATE.replace("{tools_schema}", &tools_str);

        // 如果注册表非空，追加 Available Skills 段落
        if !registry.entries.is_empty() {
            prompt.push_str("\n\n## Available Skills\n");
            prompt.push_str(
                "You can load any of the following skills on-demand by calling the `use_skill` tool:\n\n",
            );
            for entry in &registry.entries {
                prompt.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    entry.metadata.name, entry.scope, entry.metadata.description
                ));
            }
        }

        prompt
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillEntry, SkillMetadata, SkillSourceType};
    use std::path::PathBuf;

    #[test]
    fn test_prompt_builder_includes_schema() {
        let builder = PromptBuilder::new();
        let schema = serde_json::json!([
            {
                "name": "bash",
                "description": "Run shell commands"
            }
        ]);
        let prompt = builder.build(&schema, &SkillRegistry::new());
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("\"name\": \"bash\""));
        assert!(prompt.contains("Run shell commands"));
        assert!(prompt.contains("Rules:"));
    }

    #[test]
    fn test_prompt_builder_empty_schema() {
        let builder = PromptBuilder::default();
        let prompt = builder.build(&serde_json::json!([]), &SkillRegistry::new());
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("[]"));
    }

    #[tokio::test]
    async fn test_prompt_builder_with_real_tools() {
        let builder = PromptBuilder::new();
        let schema = crate::tools::tool_schema().await;
        let prompt = builder.build(&schema, &SkillRegistry::new());
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("read"));
        assert!(prompt.contains("write"));
        assert!(prompt.contains("edit"));
        assert!(prompt.contains("web_fetch"));
        assert!(prompt.contains("grep"));
    }

    /// 测试当注册表包含 Skill 时，提示词末尾会追加 "## Available Skills" 段落
    #[test]
    fn test_prompt_builder_with_skills() {
        let mut registry = SkillRegistry::new();
        registry.entries.push(SkillEntry {
            id: "test-commit".to_string(),
            scope: "test".to_string(),
            source_type: SkillSourceType::Project,
            symlink_path: PathBuf::from("/tmp/skills/test-commit"),
            target_path: PathBuf::from("/home/user/skills/test-commit"),
            metadata: SkillMetadata {
                name: "commit".to_string(),
                description: "Help write commit messages".to_string(),
                tags: vec![],
            },
        });

        let builder = PromptBuilder::new();
        let prompt = builder.build(&serde_json::json!([]), &registry);

        assert!(
            prompt.contains("## Available Skills"),
            "prompt should contain '## Available Skills'"
        );
        assert!(
            prompt.contains("`commit` (test): Help write commit messages"),
            "prompt should contain skill info"
        );
    }

    /// 测试当注册表为空时，提示词中不会包含 "## Available Skills"
    #[test]
    fn test_prompt_builder_without_skills() {
        let registry = SkillRegistry::new();
        let builder = PromptBuilder::new();
        let prompt = builder.build(&serde_json::json!([]), &registry);
        assert!(
            !prompt.contains("## Available Skills"),
            "prompt should NOT contain '## Available Skills' when registry is empty"
        );
    }
}
