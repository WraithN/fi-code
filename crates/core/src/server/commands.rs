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

use std::sync::{Arc, RwLock};

use anyhow::anyhow;
use axum::{extract::State, Json};
use serde::Deserialize;

use crate::agent::LoopState;
use crate::commands::registry::{
    CommandContext, CommandHandler, CommandMeta, CommandOutput, CommandRegistry,
};
use crate::commands::slash::{InitCommandHandler, ModelCommandHandler};

use super::models::ApiResponse;
use super::server::AppState;
use super::session::HttpSessionManager;

/// 构建命令注册表，注册所有可用的 slash 命令处理器。
///
/// 返回 `(CommandRegistry, current_theme)`，其中 `current_theme` 需要被共享给 AppState。
pub fn build_command_registry(
    sessions: Arc<HttpSessionManager>,
) -> (CommandRegistry, Arc<RwLock<String>>) {
    let mut commands = CommandRegistry::new();

    // 注册 /clear 命令处理器
    let sessions_for_clear = sessions.clone();
    struct ClearHandler {
        sessions: Arc<HttpSessionManager>,
    }

    #[async_trait::async_trait]
    impl CommandHandler for ClearHandler {
        async fn execute(
            &self,
            _args: Option<String>,
            ctx: &CommandContext,
        ) -> anyhow::Result<CommandOutput> {
            if let Some(id) = &ctx.session_id {
                self.sessions.save(id, LoopState::new(Vec::new()));
            }
            Ok(CommandOutput::text("Conversation cleared"))
        }
    }

    commands.register(
        CommandMeta {
            name: "clear".into(),
            description: "Clear conversation".into(),
            args_hint: None,
        },
        Box::new(ClearHandler {
            sessions: sessions_for_clear,
        }),
    );

    // 注册 /models 命令处理器
    commands.register(
        CommandMeta {
            name: "models".into(),
            description: "Switch model".into(),
            args_hint: Some("[model_key]".into()),
        },
        Box::new(ModelCommandHandler),
    );

    // 注册 /init 命令处理器
    commands.register(
        CommandMeta {
            name: "init".into(),
            description: "Generate AGENTS.md".into(),
            args_hint: None,
        },
        Box::new(InitCommandHandler),
    );

    // 注册 /themes 命令处理器
    let current_theme = Arc::new(RwLock::new("deep_ocean".to_string()));
    let current_theme_for_handler = current_theme.clone();
    struct ThemeHandler {
        current_theme: Arc<RwLock<String>>,
    }

    #[async_trait::async_trait]
    impl CommandHandler for ThemeHandler {
        async fn execute(
            &self,
            args: Option<String>,
            _ctx: &CommandContext,
        ) -> anyhow::Result<CommandOutput> {
            if let Some(theme_name) = args.filter(|s| !s.is_empty()) {
                let mut current = self
                    .current_theme
                    .write()
                    .map_err(|_| anyhow!("主题锁中毒"))?;
                *current = theme_name.clone();
                Ok(CommandOutput::text(format!(
                    "✅ 已切换主题: {}",
                    theme_name
                )))
            } else {
                let current = self
                    .current_theme
                    .read()
                    .map_err(|_| anyhow!("主题锁中毒"))?;
                Ok(CommandOutput::text(format!("当前主题: {}", *current)))
            }
        }
    }

    commands.register(
        CommandMeta {
            name: "themes".into(),
            description: "Switch theme".into(),
            args_hint: Some("[theme_name]".into()),
        },
        Box::new(ThemeHandler {
            current_theme: current_theme_for_handler,
        }),
    );

    // 注册 /skills 命令（TUI 端交互处理，Server 端仅注册元数据）
    struct SkillCommandHandler;

    #[async_trait::async_trait]
    impl CommandHandler for SkillCommandHandler {
        async fn execute(
            &self,
            _args: Option<String>,
            _ctx: &CommandContext,
        ) -> anyhow::Result<CommandOutput> {
            Ok(CommandOutput {
                message: String::new(),
                r#type: crate::commands::registry::OutputType::Silent,
                metadata: None,
            })
        }
    }

    commands.register(
        CommandMeta {
            name: "skills".into(),
            description: "List and load available skills".into(),
            args_hint: None,
        },
        Box::new(SkillCommandHandler),
    );

    (commands, current_theme)
}

/// 命令执行请求体
#[derive(Deserialize)]
pub struct ExecuteCommandBody {
    pub args: Option<String>,
    pub session_id: Option<String>,
}

/// 列出所有可用命令
pub async fn handle_list_commands(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<CommandMeta>>> {
    let metas = state.commands.list();
    let owned: Vec<_> = metas.into_iter().cloned().collect();
    Json(ApiResponse::success(owned))
}

/// 执行指定命令
pub async fn handle_execute_command(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(body): Json<ExecuteCommandBody>,
) -> Json<ApiResponse<CommandOutput>> {
    let ctx = CommandContext {
        provider: state.provider.clone(),
        config: state.config.clone(),
        session_id: body.session_id,
    };

    match state.commands.execute(&name, body.args, &ctx).await {
        Ok(output) => Json(ApiResponse::success(output)),
        Err(e) => Json(ApiResponse::error(e.to_string(), "COMMAND_ERROR")),
    }
}
