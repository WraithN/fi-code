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

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio_stream::StreamExt;

use crate::agent::{agent_loop, LoopState};
use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_trace;
use crate::session::message::{Message, Part, Role};
use crate::tools::set_task_provider;

use super::super::server::{check_auth, AppState};
use super::super::transport::rpc::JsonRpcResponse;
use super::super::transport::sse::{create_sse_channel, SseEvent, SseSender};

/// Chat 请求体
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
}

/// Chat 端点处理器 — 返回 SSE
pub async fn handle_chat_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    log_info!("[Server] handle_chat_endpoint | session_id={:?} | message_len={}", req.session_id, req.message.len());
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
    log_debug!("[Server] SSE channel created | session_id={}", session_id);

    // 在后台 task 中运行 agent_chat
    tokio::spawn(run_agent_chat(
        state,
        session_id.clone(),
        req.message,
        sse_sender,
    ));

    // 返回 SSE 响应
    let stream = sse_stream.map(|event| {
        log_trace!("[Server] SSE outgoing | {:?}", std::mem::discriminant(&event));
        let data = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
    });
    axum::response::Sse::new(stream).into_response()
}

/// 发送 Assistant 消息的结构化详情（思考过程、工具调用等）。
/// 文本内容已通过实时流式发送，此处不再重复发送文本。
async fn send_last_assistant_details(messages: &[Message], sse_sender: &SseSender) {
    let Some(last_msg) = messages.last() else {
        return;
    };
    if last_msg.role != Role::Assistant {
        return;
    }

    // 发送结构化详情（思考过程、工具调用等）
    let blocks: Vec<crate::server::transport::sse::DetailBlock> = last_msg
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::Reasoning { thinking, .. } => {
                Some(crate::server::transport::sse::DetailBlock::Reasoning {
                    thinking: thinking.clone(),
                })
            }
            Part::ToolUse {
                id,
                name,
                arguments,
            } => {
                let args_str = serde_json::to_string_pretty(arguments).unwrap_or_default();
                Some(crate::server::transport::sse::DetailBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: args_str,
                })
            }
            Part::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => Some(crate::server::transport::sse::DetailBlock::ToolResult {
                tool_use_id: tool_call_id.clone(),
                content: content.clone(),
                is_error: *is_error,
            }),
            _ => None,
        })
        .collect();

    if !blocks.is_empty() {
        let _ = sse_sender.send(SseEvent::MessageDetails { blocks }).await;
    }
}

/// 后台运行 Agent 对话
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    sse_sender: SseSender,
) {
    log_info!("[Server] run_agent_chat start | session_id={} | message_len={}", session_id, message.len());
    // 设置全局 Provider，供 handle_task_plan 工具使用
    set_task_provider(Arc::clone(&state.provider));

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
        Ok(p) => {
            let model_name = p.model_name().unwrap_or("unknown").to_string();
            log_info!("[Server] Provider config | model={}", model_name);
            p.get_client()
                .map_err(|e| format!("Failed to create client: {}", e))
        }
        Err(_) => Err("Provider lock poisoned".to_string()),
    };
    let client = match client_result {
        Ok(c) => c,
        Err(msg) => {
            log_error!("[Server] Failed to get client | {}", msg);
            let _ = sse_sender.send(SseEvent::Error { message: msg }).await;
            return;
        }
    };

    // 运行 agent_loop，传入实时文本回调实现真流式
    let sse_sender_for_stream = sse_sender.clone();
    let mut on_text: Option<Box<dyn FnMut(&str) + Send>> = Some(Box::new(move |text: &str| {
        log_trace!("[Server] on_text callback | len={}", text.len());
        let _ = sse_sender_for_stream.try_send(SseEvent::Message {
            content: text.to_string(),
        });
    }));
    let sse_sender_for_tools = sse_sender.clone();
    let mut on_tool_event: Option<Box<dyn FnMut(SseEvent) + Send>> = Some(Box::new(move |event: SseEvent| {
        log_trace!("[Server] on_tool_event callback | {:?}", std::mem::discriminant(&event));
        let _ = sse_sender_for_tools.try_send(event);
    }));
    log_info!("[Server] agent_loop starting | messages={}", loop_state.messages.len());
    if let Err(e) = agent_loop(client.as_ref(), &mut loop_state, &mut on_text, &mut on_tool_event).await {
        log_error!("[Server] agent_loop error | {}", e);
        let _ = sse_sender
            .send(SseEvent::Error {
                message: format!("Agent loop error: {}", e),
            })
            .await;
    } else {
        log_info!("[Server] agent_loop completed successfully");
        // 发送结构化详情（文本已通过实时流式发送，此处不再重复）
        send_last_assistant_details(&loop_state.messages, &sse_sender).await;
    }

    log_info!("[Server] run_agent_chat end | prompt_tokens={} | completion_tokens={}",
        loop_state.token_usage.prompt_tokens,
        loop_state.token_usage.completion_tokens
    );

    // 发送 Token 使用量
    let _ = sse_sender
        .send(SseEvent::Usage {
            prompt_tokens: loop_state.token_usage.prompt_tokens,
            completion_tokens: loop_state.token_usage.completion_tokens,
        })
        .await;

    // 保存会话状态
    state.sessions.save(&session_id, loop_state);

    // 发送 done 事件
    let _ = sse_sender.send(SseEvent::Done { session_id: session_id.clone() }).await;
    log_info!("[Server] SSE Done sent | session_id={}", session_id);
}

/// 模型切换请求体
#[derive(Deserialize)]
pub struct SwitchModelRequest {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
}

/// 模型切换响应
#[derive(serde::Serialize)]
pub struct SwitchModelResponse {
    pub provider: String,
    pub model: String,
}

/// POST /api/model/switch — 切换当前使用的模型
pub async fn handle_switch_model(
    State(state): State<AppState>,
    Json(req): Json<SwitchModelRequest>,
) -> Response {
    let cfg = match state.config.read() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    super::super::models::ApiResponse::<SwitchModelResponse>::error(
                        "Config lock poisoned".to_string(),
                        "INTERNAL_ERROR",
                    ),
                ),
            )
                .into_response();
        }
    };

    let mut provider = match state.provider.write() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    super::super::models::ApiResponse::<SwitchModelResponse>::error(
                        "Provider lock poisoned".to_string(),
                        "INTERNAL_ERROR",
                    ),
                ),
            )
                .into_response();
        }
    };

    match provider.set_model_by_provider(&req.provider, &req.model, &cfg, req.api_key.as_deref()) {
        Ok(()) => {
            let resp = SwitchModelResponse {
                provider: req.provider,
                model: req.model,
            };
            Json(super::super::models::ApiResponse::success(resp)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(
                super::super::models::ApiResponse::<SwitchModelResponse>::error(
                    e.to_string(),
                    "BAD_REQUEST",
                ),
            ),
        )
            .into_response(),
    }
}

/// GET /api/config — 获取当前配置摘要
pub async fn handle_get_config(State(state): State<AppState>) -> Response {
    let provider_info = match state.provider.read() {
        Ok(p) => p.info(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(super::super::models::ApiResponse::<serde_json::Value>::error(
                    "Provider lock poisoned".to_string(),
                    "INTERNAL_ERROR",
                )),
            )
                .into_response();
        }
    };

    let config_path = match state.config.read() {
        Ok(c) => c.source_path.clone().unwrap_or_else(|| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };

    let resp = serde_json::json!({
        "config_path": config_path,
        "provider": provider_info,
    });

    Json(super::super::models::ApiResponse::success(resp)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::server::test_helpers::create_test_app_state;
    use axum::{extract::State, Json};

    #[tokio::test]
    async fn test_handle_get_config() {
        let state = create_test_app_state();
        let response = handle_get_config(State(state)).await;

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["config_path"], "/test/config.json");
        assert_eq!(json["data"]["provider"]["model_name"], "test-model");
        assert_eq!(json["data"]["provider"]["base_url"], "http://localhost:11434");
        assert_eq!(json["data"]["provider"]["model_type"], "openai_compatible");
    }

    #[tokio::test]
    async fn test_handle_list_models_endpoint() {
        let state = create_test_app_state();
        let response = handle_list_models_endpoint(State(state)).await;

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["current_model"], "test-model");

        let providers = json["data"]["providers"].as_array().unwrap();
        assert!(!providers.is_empty());

        // 验证预设 Provider 也被合并进来了
        let provider_keys: Vec<&str> = providers
            .iter()
            .map(|p| p["key"].as_str().unwrap())
            .collect();
        assert!(provider_keys.contains(&"test-provider"));
    }

    #[tokio::test]
    async fn test_handle_switch_model_success() {
        let state = create_test_app_state();
        let req = SwitchModelRequest {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            api_key: None,
        };
        let response = handle_switch_model(State(state.clone()), Json(req)).await;

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["provider"], "test-provider");
        assert_eq!(json["data"]["model"], "test-model");

        // 验证 Provider 确实被更新了
        let provider = state.provider.read().unwrap();
        assert_eq!(provider.model_name().unwrap(), "test-model");
    }

    #[tokio::test]
    async fn test_handle_switch_model_not_found() {
        let state = create_test_app_state();
        let req = SwitchModelRequest {
            provider: "nonexistent".to_string(),
            model: "nonexistent".to_string(),
            api_key: None,
        };
        let response = handle_switch_model(State(state), Json(req)).await;

        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::BAD_REQUEST);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!json["success"].as_bool().unwrap());
        assert!(json["error"].as_str().unwrap().contains("nonexistent"));
    }
}

/// GET /api/models — 列出所有可用模型（按 Provider 分组）
pub async fn handle_list_models_endpoint(State(state): State<AppState>) -> Response {
    let cfg = match state.config.read() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    super::super::models::ApiResponse::<serde_json::Value>::error(
                        "Config lock poisoned".to_string(),
                        "INTERNAL_ERROR",
                    ),
                ),
            )
                .into_response();
        }
    };

    let current_model = match state.provider.read() {
        Ok(p) => p.model_name().unwrap_or("unknown").to_string(),
        Err(_) => "unknown".to_string(),
    };

    let providers: Vec<serde_json::Value> = cfg
        .provider
        .iter()
        .map(|(key, p_cfg)| {
            let models: Vec<serde_json::Value> = p_cfg
                .models
                .iter()
                .map(|(m_key, m_cfg)| {
                    let mut obj = serde_json::json!({
                        "key": m_key,
                        "name": m_cfg.name,
                    });
                    if let Some(limit) = &m_cfg.limit {
                        obj["limit"] = serde_json::json!({
                            "context": limit.context,
                            "output": limit.output
                        });
                    }
                    obj
                })
                .collect();
            serde_json::json!({
                "key": key,
                "name": p_cfg.name,
                "type": match p_cfg.provider_type {
                    crate::config::models::ProviderType::Anthropic => "anthropic",
                    crate::config::models::ProviderType::OpenAiCompatible => "openai_compatible",
                },
                "models": models
            })
        })
        .collect();

    let resp = serde_json::json!({
        "providers": providers,
        "current_model": current_model,
    });

    Json(super::super::models::ApiResponse::success(resp)).into_response()
}
