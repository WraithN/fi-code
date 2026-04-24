use std::sync::{Arc, RwLock};
use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::Config;
use crate::provider::Provider;

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
        "/model" => SlashCommand::Model(arg.filter(|s| !s.is_empty())),
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

    pub fn execute(&self, cmd: SlashCommand) -> Result<SlashCommandResult> {
        match cmd {
            SlashCommand::Model(model_key) => self.handle_model(model_key),
            SlashCommand::Init => {
                // /init 的具体实现在 Task 7 中完善
                println!("{} /init 指令已识别，完整实现将在后续任务中添加", "ℹ️".blue());
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

    fn handle_model(&self, model_key: Option<String>) -> Result<SlashCommandResult> {
        let cfg = self.config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let mut provider = self.provider.write().map_err(|_| anyhow!("Provider锁中毒"))?;
        
        if let Some(key) = model_key {
            if provider.list_models(&cfg).iter().any(|(k, _)| k == &key) {
                provider.set_model(&key, &cfg)?;
                println!("✅ 已切换模型: {}", key);
            } else {
                eprintln!("❌ 没有此模型: {}", key);
                // drop provider lock before print_model_list
                drop(provider);
                self.print_model_list(&cfg)?;
            }
        } else {
            drop(provider);
            self.print_model_list(&cfg)?;
        }
        Ok(SlashCommandResult::Handled)
    }

    fn print_model_list(&self, cfg: &Config) -> Result<()> {
        let models = self.provider.read().map_err(|_| anyhow!("Provider锁中毒"))?.list_models(cfg);
        if models.is_empty() {
            println!("{} 配置文件中未找到任何模型", "❌".red());
            return Ok(());
        }

        println!("可用模型列表：");
        for (i, (key, display)) in models.iter().enumerate() {
            let mut limit_str = String::new();
            for (_pname, pcfg) in &cfg.provider {
                if let Some(mcfg) = pcfg.models.get(key) {
                    limit_str = format!(
                        " (context: {}, output: {})",
                        mcfg.limit.context, mcfg.limit.output
                    );
                    break;
                }
            }
            println!("  [{}] {} — {}{}", i + 1, key, display, limit_str);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_no_args() {
        assert_eq!(parse("/model"), SlashCommand::Model(None));
    }

    #[test]
    fn test_parse_model_with_args() {
        assert_eq!(
            parse("/model gpt-4o"),
            SlashCommand::Model(Some("gpt-4o".to_string()))
        );
    }

    #[test]
    fn test_parse_init() {
        assert_eq!(parse("/init"), SlashCommand::Init);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            parse("/foo"),
            SlashCommand::Unknown("foo".to_string())
        );
    }

    #[test]
    fn test_parse_not_slash() {
        assert_eq!(
            parse("hello world"),
            SlashCommand::Unknown("".to_string())
        );
    }
}
