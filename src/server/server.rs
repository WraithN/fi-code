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
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::agent::{agent_loop, LoopState};
use crate::commands::registry::{CommandContext, CommandHandler, CommandMeta, CommandOutput, CommandRegistry};
use crate::commands::slash::{InitCommandHandler, ModelCommandHandler};
use crate::config::Config;
use crate::provider::Provider;
use crate::session::message::{Message, Part, Role};

use super::file_api;
use super::models::ApiResponse;
use super::rpc::{handle_rpc, JsonRpcRequest, JsonRpcResponse};
use super::session::HttpSessionManager;
use super::session_api;
use super::sse::{create_sse_channel, format_sse_event, SseEvent, SseSender};

/// 服务器共享状态
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
    pub themes: Vec<crate::tui::theme::ThemePreset>,
    pub current_theme: Arc<RwLock<String>>,
    pub log_broadcaster: Option<Arc<crate::utils::log_store::LogBroadcaster>>,
}

pub struct Server {
    state: AppState,
    port: u16,
}

impl Server {
    pub fn new(
        provider: Arc<RwLock<Provider>>,
        config: Arc<RwLock<Config>>,
        port_override: Option<u16>,
    ) -> Self {
        let port = port_override
            .or_else(|| {
                let cfg = config.read().ok()?;
                let server_cfg = cfg.server.as_ref()?;
                server_cfg.port
            })
            .unwrap_or(4040);

        let sessions = Arc::new(HttpSessionManager::new());
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

        // 注册 /model 命令处理器
        commands.register(
            CommandMeta {
                name: "model".into(),
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

        // 注册 /theme 命令处理器
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
                    let mut current = self.current_theme.write().map_err(|_| anyhow!("主题锁中毒"))?;
                    *current = theme_name.clone();
                    Ok(CommandOutput::text(format!("✅ 已切换主题: {}", theme_name)))
                } else {
                    let current = self.current_theme.read().map_err(|_| anyhow!("主题锁中毒"))?;
                    Ok(CommandOutput::text(format!("当前主题: {}", *current)))
                }
            }
        }

        commands.register(
            CommandMeta {
                name: "theme".into(),
                description: "Switch theme".into(),
                args_hint: Some("[theme_name]".into()),
            },
            Box::new(ThemeHandler {
                current_theme: current_theme_for_handler,
            }),
        );

        // 注册 /skill 命令（TUI 端交互处理，Server 端仅注册元数据）
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
                name: "skill".into(),
                description: "List and load available skills".into(),
                args_hint: None,
            },
            Box::new(SkillCommandHandler),
        );

        let themes = crate::tui::theme::ThemePreset::all_presets();

        Self {
            state: AppState {
                provider,
                config,
                sessions,
                commands: Arc::new(commands),
                themes,
                current_theme,
                log_broadcaster: None,
            },
            port,
        }
    }

    pub fn with_log_broadcaster(mut self, broadcaster: Arc<crate::utils::log_store::LogBroadcaster>) -> Self {
        self.state.log_broadcaster = Some(broadcaster);
        self
    }

    pub async fn run(self) {
        let app = Router::new()
            .route("/rpc", post(handle_rpc_endpoint))
            .route("/chat", post(handle_chat_endpoint))
            .route(
                "/api/sessions",
                get(session_api::list_sessions).post(session_api::create_session),
            )
            .route(
                "/api/sessions/:id",
                put(session_api::rename_session).delete(session_api::delete_session),
            )
            .route(
                "/api/sessions/:id/switch",
                post(session_api::switch_session),
            )
            .route("/api/files", get(file_api::file_tree))
            .route("/api/files/content", get(file_api::file_content))
            .route("/api/commands", get(handle_list_commands))
            .route("/api/commands/:name/execute", post(handle_execute_command))
            .route("/api/themes", get(handle_list_themes))
            .route("/api/logs", get(crate::server::log_api::handle_list_logs))
            .route("/api/logs/stream", get(crate::server::log_api::handle_log_stream))
            .layer(cors_layer(self.state.config.clone()))
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .unwrap();

        println!("🚀 Server listening on http://0.0.0.0:{}", self.port);

        axum::serve(listener, app).await.unwrap();
    }
}

fn build_cors_layer(origins: &[String]) -> CorsLayer {
    let mut layer = CorsLayer::new();
    for origin in origins {
        let Ok(val) = origin.parse::<HeaderValue>() else { continue };
        layer = layer.allow_origin(val);
    }
    layer
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

/// CORS 中间件配置
fn cors_layer(config: Arc<RwLock<Config>>) -> CorsLayer {
    let cfg = config.read().unwrap();
    let Some(server_cfg) = &cfg.server else { return CorsLayer::permissive() };
    let Some(origins) = &server_cfg.allowed_origins else { return CorsLayer::permissive() };
    build_cors_layer(origins)
}

/// JSON-RPC 端点处理器
async fn handle_rpc_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    // 认证检查
    if let Some(resp) = check_auth(&headers, &state.config).await {
        return Json(resp);
    }

    let resp = handle_rpc(req, state.provider.clone(), state.config.clone()).await;
    Json(resp)
}

/// 认证检查
async fn check_auth(headers: &HeaderMap, config: &Arc<RwLock<Config>>) -> Option<JsonRpcResponse> {
    let cfg = config.read().ok()?;
    let server_cfg = cfg.server.as_ref()?;
    let expected_token = server_cfg.api_token.as_ref()?;

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !auth.starts_with("Bearer ") || auth.len() <= 7 || &auth[7..] != expected_token {
        return Some(JsonRpcResponse::error(
            -32000,
            "Unauthorized",
            Some(Value::Null),
        ));
    }

    None
}

/// Chat 请求体
#[derive(Deserialize)]
struct ChatRequest {
    session_id: Option<String>,
    message: String,
}

/// Chat 端点处理器 — 返回 SSE
async fn handle_chat_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    // 认证检查
    if let Some(resp) = check_auth(&headers, &state.config).await {
        return Json(resp).into_response();
    }

    let session_id = match req.session_id {
        Some(id) => {
            if state.sessions.get(&id).is_none() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(JsonRpcResponse::error(
                        -32001,
                        "Session not found",
                        Some(Value::Null),
                    )),
                )
                    .into_response();
            }
            id
        }
        None => state.sessions.create(),
    };

    let (sse_sender, sse_stream) = create_sse_channel(128);

    // 在后台 task 中运行 agent_chat
    tokio::spawn(run_agent_chat(
        state,
        session_id.clone(),
        req.message,
        sse_sender,
    ));

    // 返回 SSE 响应
    let stream = sse_stream.map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
    });
    axum::response::Sse::new(stream).into_response()
}

/// 命令执行请求体
#[derive(Deserialize)]
struct ExecuteCommandBody {
    args: Option<String>,
    session_id: Option<String>,
}

/// 列出所有可用命令
async fn handle_list_commands(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<CommandMeta>>> {
    let metas = state.commands.list();
    let owned: Vec<_> = metas.into_iter().cloned().collect();
    Json(ApiResponse::success(owned))
}

/// 执行指定命令
async fn handle_execute_command(
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

/// 列出所有可用主题
async fn handle_list_themes(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<crate::tui::theme::ThemePreset>>> {
    Json(ApiResponse::success(state.themes.clone()))
}

async fn send_last_assistant_text(messages: &[Message], sse_sender: &SseSender) {
    let Some(last_msg) = messages.last() else { return };
    if last_msg.role != Role::Assistant {
        return;
    }
    let text = last_msg
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        return;
    }
    let _ = sse_sender.send(SseEvent::Message { content: text }).await;
}

/// 后台运行 Agent 对话
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    sse_sender: SseSender,
) {
    // 获取或创建 LoopState
    let mut loop_state = match state.sessions.get(&session_id) {
        Some(state) => state,
        None => {
            let _ = sse_sender
                .send(SseEvent::Error {
                    message: "Session not found".to_string(),
                })
                .await;
            return;
        }
    };

    // 添加用户消息
    let user_msg = Message::new(
        session_id.clone(),
        Role::User,
        vec![Part::Text { text: message }],
    );
    loop_state.messages.push(user_msg);

    // 获取客户端（先读取并释放锁，避免 guard 跨越 await）
    let client_result = match state.provider.read() {
        Ok(p) => p
            .get_client()
            .map_err(|e| format!("Failed to create client: {}", e)),
        Err(_) => Err("Provider lock poisoned".to_string()),
    };
    let client = match client_result {
        Ok(c) => c,
        Err(msg) => {
            let _ = sse_sender.send(SseEvent::Error { message: msg }).await;
            return;
        }
    };

    // 运行 agent_loop
    if let Err(e) = agent_loop(client.as_ref(), &mut loop_state).await {
        let _ = sse_sender
            .send(SseEvent::Error {
                message: format!("Agent loop error: {}", e),
            })
            .await;
    } else {
        // 发送 assistant 的最后回复
        send_last_assistant_text(&loop_state.messages, &sse_sender).await;
    }

    // 保存会话状态
    state.sessions.save(&session_id, loop_state);

    // 发送 done 事件
    let _ = sse_sender.send(SseEvent::Done { session_id }).await;
}
