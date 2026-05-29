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

use crate::agent::{agent_loop, profile::AgentProfile, LoopState};
use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_trace;
use crate::session::message::{current_timestamp_ms, Message, Part, Role};
use crate::session::session::{Session, SessionStatus};
use crate::tools::basic_tools::BasicTool;
use crate::tools::set_task_provider;
use crate::utils::workspace::workspace_dir;
use fi_code_shared::dto::AgentType;

use super::super::server::{check_auth, AppState};
use super::super::transport::rpc::JsonRpcResponse;
use super::super::transport::sse::{create_sse_channel, SseEvent, SseSender};

// 已从 fi-code-shared crate 重新导出，保留此 re-export 维持向后兼容
pub use fi_code_shared::dto::ChatRequest;

/// Chat 端点处理器 — 返回 SSE
pub async fn handle_chat_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    let request_received_at = std::time::Instant::now();
    log_info!(
        "[Server] handle_chat_endpoint | session_id={:?} | message_len={}",
        req.session_id,
        req.message.len()
    );
    // 认证检查
    if let Some(resp) = check_auth(&headers, &state.config).await {
        return Json(resp).into_response();
    }

    // 读取语言偏好并设置
    let lang = headers
        .get("X-Lang")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("en");
    crate::i18n::set_language(lang);

    let agent_type = req.agent.unwrap_or_default();
    log_info!("[Server] handle_chat_endpoint | agent={:?}", agent_type);

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
    log_info!(
        "[Server] spawning run_agent_chat task | session_id={}",
        session_id
    );
    let spawn_session_id = session_id.clone();
    tokio::spawn(async move {
        use futures::FutureExt;
        log_info!(
            "[Server] run_agent_chat task started | session_id={}",
            spawn_session_id
        );
        let result = std::panic::AssertUnwindSafe(run_agent_chat(
            state,
            spawn_session_id.clone(),
            req.message,
            agent_type,
            sse_sender,
            request_received_at,
        ))
        .catch_unwind()
        .await;
        match result {
            Ok(Ok(())) => {
                log_info!(
                    "[Server] run_agent_chat completed successfully | session_id={}",
                    spawn_session_id
                );
            }
            Ok(Err(e)) => {
                log_error!("[Server] run_agent_chat returned error | {}", e);
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };
                log_error!("[Server] run_agent_chat panicked | {}", msg);
            }
        }
        log_info!(
            "[Server] run_agent_chat task finished | session_id={}",
            spawn_session_id
        );
    });

    // 返回 SSE 响应
    let stream = sse_stream.map(|event| {
        log_trace!(
            "[Server] SSE outgoing | {:?}",
            std::mem::discriminant(&event)
        );
        let data = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
    });
    axum::response::Sse::new(stream).into_response()
}

/// 解析消息中的 @path 引用，将文件内容注入到消息中
fn inject_file_contents(message: &str) -> String {
    use regex::Regex;

    let re = match Regex::new(r"@(\S+)") {
        Ok(re) => re,
        Err(_) => return message.to_string(),
    };

    let mut injections: Vec<String> = Vec::new();
    let mut found_paths: Vec<String> = Vec::new();

    for cap in re.captures_iter(message) {
        let path_str = cap[1].to_string();
        if found_paths.contains(&path_str) {
            continue;
        }
        found_paths.push(path_str.clone());

        let content = read_file_for_injection(&path_str);
        match content {
            Some(text) => {
                let truncated = if text.len() > 10000 {
                    format!("{}\n... (truncated)", &text[..10000])
                } else {
                    text
                };
                injections.push(format!("File: {}\n```\n{}\n```", path_str, truncated));
            }
            None => {
                injections.push(format!(
                    "File: {}\n```\n[File not found or not accessible]\n```",
                    path_str
                ));
            }
        }
    }

    if injections.is_empty() {
        return message.to_string();
    }

    // 移除所有 @path 标记，在消息开头注入文件内容
    let cleaned = re.replace_all(message, "").trim().to_string();
    format!("{}\n\n{}", injections.join("\n\n"), cleaned)
}

/// 安全读取文件内容用于 @引用注入
fn read_file_for_injection(path_str: &str) -> Option<String> {
    let workspace = workspace_dir();
    let target = workspace.join(path_str);

    // safe_path 检查：确保文件在工作目录内
    let canonical = match std::fs::canonicalize(&target) {
        Ok(p) => p,
        Err(_) => return None,
    };
    let workspace_canonical = match std::fs::canonicalize(&workspace) {
        Ok(p) => p,
        Err(_) => return None,
    };
    if !canonical.starts_with(&workspace_canonical) {
        return None;
    }

    // 检查是文件而非目录
    if !canonical.is_file() {
        return None;
    }

    std::fs::read_to_string(&canonical).ok()
}

/// 后台运行 Agent 对话
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    agent_type: AgentType,
    sse_sender: SseSender,
    request_received_at: std::time::Instant,
) -> Result<(), String> {
    log_info!(
        "[Server] run_agent_chat start | session_id={} | message_len={} | agent={:?}",
        session_id,
        message.len(),
        agent_type
    );

    // 发送当前 Agent 信息
    let profile = AgentProfile::for_type(agent_type);
    let _ = sse_sender
        .send(SseEvent::AgentInfo {
            agent_type,
            agent_name: profile.name.to_string(),
        })
        .await;

    // 设置全局 Provider，供 handle_task_plan 工具使用
    set_task_provider(Arc::clone(&state.provider));

    // 设置全局上下文限制（供压缩模块使用）
    if let Ok(provider) = state.provider.read() {
        if let Ok(config) = state.config.read() {
            let limit = provider.context_limit(&config);
            crate::agent::compression::set_context_limit(limit);
            log_info!("[Server] Context limit set to {}", limit);
        }
    }

    // 获取或创建 LoopState
    let mut loop_state = match state.sessions.get(&session_id) {
        Some(state) => state,
        None => {
            let _ = sse_sender
                .send(SseEvent::Error {
                    message: "Session not found".to_string(),
                })
                .await;
            return Ok(());
        }
    };

    // 解析 @path 并注入文件内容
    let message = inject_file_contents(&message);
    // 缓存原始用户文本：后续 ChatSpan input/output 需要原文（user_msg 会将 message move 进去）
    let original_message = message.clone();

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
            return Ok(());
        }
    };

    // 运行 agent_loop，传入实时文本回调实现真流式
    let sse_sender_for_stream = sse_sender.clone();
    let first_text_sent = std::sync::atomic::AtomicBool::new(false);
    let session_id_for_ttft = session_id.clone();
    let mut on_text: Option<Box<dyn FnMut(&str) + Send>> = Some(Box::new(move |text: &str| {
        log_trace!("[Server] on_text callback | len={}", text.len());
        if !first_text_sent.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let elapsed_ms = request_received_at.elapsed().as_millis() as u64;
            log_info!(
                "[TTFT] first token SSE sent | total={}ms | session_id={}",
                elapsed_ms,
                session_id_for_ttft
            );
        }
        let _ = sse_sender_for_stream.try_send(SseEvent::Message {
            content: text.to_string(),
        });
    }));
    let sse_sender_for_tools = sse_sender.clone();
    let mut first_event_logged = false;
    let session_id_for_diag = session_id.clone();
    let mut on_tool_event: Option<Box<dyn FnMut(SseEvent) + Send>> =
        Some(Box::new(move |event: SseEvent| {
            let event_type = format!("{:?}", std::mem::discriminant(&event));
            if !first_event_logged {
                first_event_logged = true;
                let elapsed_ms = request_received_at.elapsed().as_millis() as u64;
                log_info!(
                    "[TTFT-DIAG] first SSE event sending | total={}ms | type={} | session_id={}",
                    elapsed_ms,
                    event_type,
                    session_id_for_diag
                );
            }
            log_trace!(
                "[Server] on_tool_event callback | {:?}",
                std::mem::discriminant(&event)
            );
            // 当 Usage Part 到达时，同步发送 CompressionStatus 更新上下文比例
            if let SseEvent::Part {
                part: fi_code_shared::dto::Part::Usage { prompt_tokens: input_tokens, .. },
            } = &event
            {
                let limit = crate::agent::compression::get_context_limit();
                let ratio = if limit > 0 {
                    ((*input_tokens as f64 / limit as f64) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                let _ = sse_sender_for_tools.try_send(SseEvent::CompressionStatus {
                    is_compressing: false,
                    progress: 0,
                    context_ratio: ratio,
                    summary: None,
                });
            }
            let _ = sse_sender_for_tools.try_send(event);
        }));
    log_info!(
        "[Server] agent_loop starting | messages={}",
        loop_state.messages.len()
    );

    // Task 4.1：在 agent_loop 之前打开 ChatSpan，作为本次请求的根 trace
    use crate::observability::otel;
    let chat_span = otel::start_chat_span(&session_id, &original_message, agent_type);
    let chat_cx = chat_span.context();

    let agent_loop_result = agent_loop(
        client.as_ref(),
        &mut loop_state,
        agent_type,
        &mut on_text,
        &mut on_tool_event,
        Some(&sse_sender),
        Some(&chat_cx),
    )
    .await;

    // 提取最后一条 Assistant 消息的文本作为 ChatSpan output
    let final_assistant_text: String = loop_state
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    crate::session::message::Part::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    chat_span.set_output(&final_assistant_text);

    if let Err(e) = agent_loop_result {
        log_error!("[Server] agent_loop error | {}", e);
        chat_span.record_error(&e.to_string());
        let _ = sse_sender
            .send(SseEvent::Error {
                message: format!("Agent loop error: {}", e),
            })
            .await;
    } else {
        log_info!("[Server] agent_loop completed successfully");
    }
    // chat_span 在作用域结束时 drop，自动 end

    log_info!(
        "[Server] run_agent_chat end | prompt_tokens={} | completion_tokens={}",
        loop_state.token_usage.prompt_tokens,
        loop_state.token_usage.completion_tokens
    );

    // 发送 Token 使用量
    let _ = sse_sender
        .send(SseEvent::Part {
            part: Part::Usage {
                prompt_tokens: loop_state.token_usage.prompt_tokens,
                completion_tokens: loop_state.token_usage.completion_tokens,
                latency_ms: 0,
                cost: None,
            },
        })
        .await;

    // 保存会话状态（内存）
    let messages_for_disk = loop_state.messages.clone();
    state.sessions.save(&session_id, loop_state);

    // 持久化到磁盘（JSONL）
    if let Some(ref sm) = state.session_manager {
        let model = match state.provider.read() {
            Ok(p) => p.model_name().unwrap_or("unknown").to_string(),
            Err(_) => "unknown".to_string(),
        };
        let now = current_timestamp_ms();
        // 尝试复用已有会话的 created_at，否则使用当前时间
        let created_at = sm
            .load_session(&session_id)
            .map(|s| s.created_at)
            .unwrap_or(now);
        let session = Session {
            id: session_id.clone(),
            project_path: workspace_dir().to_string_lossy().to_string(),
            created_at,
            updated_at: now,
            model,
            status: SessionStatus::Active,
            agent_type: fi_code_shared::dto::AgentType::Build,
            messages: messages_for_disk,
        };
        let sm = Arc::clone(sm);
        tokio::task::spawn_blocking(move || {
            if let Err(e) = sm.save_session(&session) {
                log_error!("[Server] Failed to save session to disk: {}", e);
            } else {
                log_info!("[Server] Session saved to disk | id={}", session.id);
            }
        });
    }

    // 发送 done 事件
    let _ = sse_sender
        .send(SseEvent::Done {
            session_id: session_id.clone(),
        })
        .await;
    log_info!("[Server] SSE Done sent | session_id={}", session_id);
    Ok(())
}

// 已从 fi-code-shared crate 重新导出，保留此 re-export 维持向后兼容
pub use fi_code_shared::dto::{SwitchModelRequest, SwitchModelResponse};

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
                Json(
                    super::super::models::ApiResponse::<serde_json::Value>::error(
                        "Provider lock poisoned".to_string(),
                        "INTERNAL_ERROR",
                    ),
                ),
            )
                .into_response();
        }
    };

    let config_path = match state.config.read() {
        Ok(c) => c
            .source_path
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
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
        assert_eq!(
            json["data"]["provider"]["base_url"],
            "http://localhost:11434"
        );
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

    #[test]
    fn test_inject_file_contents_no_mention() {
        let msg = "Hello world".to_string();
        let result = inject_file_contents(&msg);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_inject_file_contents_single_mention() {
        let msg = "@Cargo.toml explain this".to_string();
        let result = inject_file_contents(&msg);
        // 验证文件内容被注入，且 @Cargo.toml 被移除
        assert!(result.contains("File: Cargo.toml"));
        assert!(!result.contains("@Cargo.toml"));
        assert!(result.contains("explain this"));
    }

    #[test]
    fn test_inject_file_contents_multiple_mentions() {
        let msg = "@Cargo.toml and @README.md compare".to_string();
        let result = inject_file_contents(&msg);
        // 验证两个文件都被注入
        assert!(result.contains("File: Cargo.toml"));
        assert!(result.contains("File: README.md"));
        assert!(!result.contains("@Cargo.toml"));
        assert!(!result.contains("@README.md"));
        assert!(result.contains("compare"));
    }

    #[test]
    fn test_inject_file_contents_duplicate_mentions() {
        let msg = "@Cargo.toml and @Cargo.toml again".to_string();
        let result = inject_file_contents(&msg);
        // 验证只注入一次
        let count = result.matches("File: Cargo.toml").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_inject_file_contents_not_found() {
        let msg = "@nonexistent_file_xyz.txt hello".to_string();
        let result = inject_file_contents(&msg);
        assert!(result.contains("File: nonexistent_file_xyz.txt"));
        assert!(result.contains("[File not found or not accessible]"));
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
