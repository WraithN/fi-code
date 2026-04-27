use std::sync::{Arc, RwLock};

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
use crate::config::Config;
use crate::provider::Provider;
use crate::session::message::{Message, Part, Role};

use super::file_api;
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

        Self {
            state: AppState {
                provider,
                config,
                sessions: Arc::new(HttpSessionManager::new()),
            },
            port,
        }
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
            .layer(cors_layer(self.state.config.clone()))
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .unwrap();

        println!("🚀 Server listening on http://0.0.0.0:{}", self.port);

        axum::serve(listener, app).await.unwrap();
    }
}

/// CORS 中间件配置
fn cors_layer(config: Arc<RwLock<Config>>) -> CorsLayer {
    let cfg = config.read().unwrap();
    if let Some(server_cfg) = &cfg.server {
        if let Some(origins) = &server_cfg.allowed_origins {
            let mut layer = CorsLayer::new();
            for origin in origins {
                if let Ok(val) = origin.parse::<HeaderValue>() {
                    layer = layer.allow_origin(val);
                }
            }
            return layer
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
        }
    }
    CorsLayer::permissive()
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
        if let Some(last_msg) = loop_state.messages.last() {
            if last_msg.role == Role::Assistant {
                let text = last_msg
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    let _ = sse_sender.send(SseEvent::Message { content: text }).await;
                }
            }
        }
    }

    // 保存会话状态
    state.sessions.save(&session_id, loop_state);

    // 发送 done 事件
    let _ = sse_sender.send(SseEvent::Done { session_id }).await;
}
