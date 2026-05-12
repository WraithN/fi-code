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
        async fn execute(
            &self,
            args: Option<String>,
            _ctx: &CommandContext,
        ) -> Result<CommandOutput> {
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
