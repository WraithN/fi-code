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

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use colored::Colorize;
use std::sync::{Arc, RwLock};

use crate::commands::registry::{CommandContext, CommandHandler, CommandOutput, OutputType};
use crate::config::Config;
use crate::provider::Provider;
use crate::session::message::{Message, Part, Role};

/// 可识别的 slash 指令
#[derive(Debug, PartialEq)]
pub enum SlashCommand {
    /// /model [model_key]
    Model(Option<String>),
    /// /init
    Init,
    /// 未知指令（携带指令名，空字符串表示非 slash 输入）
    Unknown(String),
}

/// 指令执行结果
#[derive(Debug, PartialEq)]
pub enum SlashCommandResult {
    /// 指令已处理，无需进入正常 LLM 对话流程
    Handled,
    /// 非 slash 指令，按正常用户输入处理
    Passthrough(String),
}

/// 解析用户输入为 slash 指令
pub fn parse(input: &str) -> SlashCommand {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return SlashCommand::Unknown("".to_string());
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim().to_string());

    match cmd {
        "/models" => SlashCommand::Model(arg.filter(|s| !s.is_empty())),
        "/init" => SlashCommand::Init,
        _ => SlashCommand::Unknown(cmd.trim_start_matches('/').to_string()),
    }
}

/// 指令执行器
pub struct SlashCommandHandler {
    provider: Arc<RwLock<Provider>>,
    config: Arc<RwLock<Config>>,
}

impl SlashCommandHandler {
    pub fn new(provider: Arc<RwLock<Provider>>, config: Arc<RwLock<Config>>) -> Self {
        Self { provider, config }
    }

    pub async fn execute(&self, cmd: SlashCommand) -> Result<SlashCommandResult> {
        let ctx = CommandContext {
            provider: self.provider.clone(),
            config: self.config.clone(),
            session_id: None,
        };

        match cmd {
            SlashCommand::Model(model_key) => {
                let output = ModelCommandHandler.execute(model_key, &ctx).await?;
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
            SlashCommand::Unknown(name) if name.is_empty() => {
                unreachable!()
            }
            SlashCommand::Unknown(name) => {
                eprintln!(
                    "{} 未知命令: /{}。可用命令: /init, /model",
                    "❌".red(),
                    name
                );
                Ok(SlashCommandResult::Handled)
            }
        }
    }
}

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

        // 辅助闭包：构造模型列表文本
        let build_list = |cfg: &Config| -> Result<String> {
            let models = ctx
                .provider
                .read()
                .map_err(|_| anyhow!("Provider锁中毒"))?
                .list_models(cfg);
            if models.is_empty() {
                return Ok("❌ 配置文件中未找到任何模型\n".to_string());
            }
            let mut output = String::from("可用模型列表：\n");
            for (i, (mkey, display)) in models.iter().enumerate() {
                let mut limit_str = String::new();
                for (_pname, pcfg) in &cfg.provider {
                    if let Some(mcfg) = pcfg.models.get(mkey) {
                        if let Some(limit) = &mcfg.limit {
                            limit_str =
                                format!(" (context: {}, output: {})", limit.context, limit.output);
                        }
                        break;
                    }
                }
                output.push_str(&format!(
                    "  [{}] {} — {}{}\n",
                    i + 1,
                    mkey,
                    display,
                    limit_str
                ));
            }
            Ok(output)
        };

        if let Some(key) = args {
            if provider.list_models(&cfg).iter().any(|(k, _)| k == &key) {
                provider.set_model(&key, &cfg)?;
                Ok(CommandOutput::text(format!("✅ 已切换模型: {}", key)))
            } else {
                let mut msg = format!("❌ 没有此模型: {}\n", key);
                drop(provider);
                msg.push_str(&build_list(&cfg)?);
                Ok(CommandOutput::error(msg))
            }
        } else {
            drop(provider);
            Ok(CommandOutput::text(build_list(&cfg)?))
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
            .get_client()?;
        let schema = tool_schema().await;

        let runner = AgentRunner::new(client, system_prompt, schema);
        let initial_messages = vec![Message::new(
            "init-session".to_string(),
            Role::User,
            vec![Part::Text { text: user_prompt }],
        )];

        let result = runner.run(initial_messages).await?;

        // 检查结果中是否包含 write 工具调用
        let has_write = result.messages.iter().any(|msg| {
            msg.parts
                .iter()
                .any(|part| matches!(part, Part::ToolUse { name, .. } if name == "write"))
        });

        if has_write || agents_path.exists() {
            Ok(CommandOutput::text(format!(
                "{} AGENTS.md 已生成: {}",
                "✅".green(),
                agents_path.display()
            )))
        } else {
            Ok(CommandOutput::text(format!(
                "{} AGENTS.md 可能未生成，请检查对话结果",
                "⚠️".yellow()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_no_args() {
        assert_eq!(parse("/models"), SlashCommand::Model(None));
    }

    #[test]
    fn test_parse_model_with_args() {
        assert_eq!(
            parse("/models gpt-4o"),
            SlashCommand::Model(Some("gpt-4o".to_string()))
        );
    }

    #[test]
    fn test_parse_init() {
        assert_eq!(parse("/init"), SlashCommand::Init);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(parse("/foo"), SlashCommand::Unknown("foo".to_string()));
    }

    #[test]
    fn test_parse_not_slash() {
        assert_eq!(parse("hello world"), SlashCommand::Unknown("".to_string()));
    }
}
