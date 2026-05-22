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

use std::path::PathBuf;
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
use tower_http::set_header::SetResponseHeaderLayer;

use crate::agent::agent_loop;
use crate::commands::registry::CommandRegistry;
use crate::config::Config;
use crate::provider::Provider;

use super::api::file_api;
use super::api::session_api;
use super::models::ApiResponse;
use super::session::HttpSessionManager;
use super::transport::rpc::{handle_rpc, JsonRpcRequest, JsonRpcResponse};
use super::transport::sse::SseEvent;

/// 从可执行文件位置向上查找 frontend/dist 目录
fn find_frontend_dist() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    let candidates = [
        exe_dir.join("frontend/dist"),
        exe_dir.join("../frontend/dist"),
        exe_dir.join("../../frontend/dist"),
        PathBuf::from("frontend/dist"),
    ];

    for path in &candidates {
        if path.join("index.html").exists() {
            return Some(path.clone());
        }
    }
    None
}

/// 服务器共享状态
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
    pub themes: Vec<crate::theme_preset::ThemePreset>,
    pub current_theme: Arc<RwLock<String>>,
    pub log_broadcaster: Option<Arc<crate::utils::log_store::LogBroadcaster>>,
    /// 磁盘会话管理器，用于将内存会话持久化到 JSONL
    pub session_manager: Option<Arc<crate::session::SessionManager>>,
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
        let (commands, current_theme) = super::commands::build_command_registry(sessions.clone());

        let themes = crate::theme_preset::ThemePreset::all_presets();

        // 初始化磁盘会话管理器
        let session_manager = {
            let config_dir = directories::ProjectDirs::from("", "", "fi-code")
                .map(|d| d.config_dir().to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from(".config/fi-code"));
            let sessions_dir = config_dir.join("sessions");
            Some(Arc::new(crate::session::SessionManager::new(sessions_dir)))
        };

        Self {
            state: AppState {
                provider,
                config,
                sessions,
                commands: Arc::new(commands),
                themes,
                current_theme,
                log_broadcaster: None,
                session_manager,
            },
            port,
        }
    }

    pub fn with_log_broadcaster(
        mut self,
        broadcaster: Arc<crate::utils::log_store::LogBroadcaster>,
    ) -> Self {
        self.state.log_broadcaster = Some(broadcaster);
        self
    }

    pub async fn run(self) {
        let frontend_dist = find_frontend_dist();
        if frontend_dist.is_none() {
            eprintln!("Warning: frontend/dist not found, serving API only");
        }

        let mut app = Router::new()
            .route("/rpc", post(handle_rpc_endpoint))
            .route("/chat", post(super::api::chat_api::handle_chat_endpoint))
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
            .route(
                "/api/sessions/:id/messages",
                get(session_api::get_session_messages),
            )
            .route("/api/files", get(file_api::file_tree))
            .route("/api/files/content", get(file_api::file_content))
            .route("/api/commands", get(super::commands::handle_list_commands))
            .route(
                "/api/commands/:name/execute",
                post(super::commands::handle_execute_command),
            )
            .route("/api/themes", get(handle_list_themes))
            .route("/api/skills", get(super::api::skill_api::list_skills))
            .route("/api/config", get(super::api::chat_api::handle_get_config))
            .route(
                "/api/models",
                get(super::api::chat_api::handle_list_models_endpoint),
            )
            .route(
                "/api/model/switch",
                post(super::api::chat_api::handle_switch_model),
            )
            .route(
                "/api/logs",
                get(crate::server::api::log_api::handle_list_logs),
            )
            .route(
                "/api/logs/stream",
                get(crate::server::api::log_api::handle_log_stream),
            )
            .route(
                "/api/permission/respond",
                post(crate::server::api::permission_api::handle_permission_respond),
            );

        // 如果找到 frontend/dist，挂载静态文件服务
        if let Some(dist_path) = frontend_dist {
            let index_path = dist_path.join("index.html");
            app = app.fallback_service(
                tower_http::services::ServeDir::new(&dist_path)
                    .fallback(tower_http::services::ServeFile::new(&index_path)),
            );
        }

        let app = app
            .layer(SetResponseHeaderLayer::overriding(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store, no-cache, must-revalidate"),
            ))
            .layer(cors_layer(self.state.config.clone()))
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to bind server to port {}: {}. Please check if the port is already in use.",
                    self.port,
                    e
                );
            });

        println!("🚀 Server listening on http://0.0.0.0:{}", self.port);

        axum::serve(listener, app).await.unwrap();
    }
}

fn build_cors_layer(origins: &[String]) -> CorsLayer {
    let mut layer = CorsLayer::new();
    for origin in origins {
        let Ok(val) = origin.parse::<HeaderValue>() else {
            continue;
        };
        layer = layer.allow_origin(val);
    }
    layer
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

/// CORS 中间件配置
fn cors_layer(config: Arc<RwLock<Config>>) -> CorsLayer {
    let cfg = config.read().unwrap();
    let Some(server_cfg) = &cfg.server else {
        return CorsLayer::permissive();
    };
    let Some(origins) = &server_cfg.allowed_origins else {
        return CorsLayer::permissive();
    };
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
pub(crate) async fn check_auth(
    headers: &HeaderMap,
    config: &Arc<RwLock<Config>>,
) -> Option<JsonRpcResponse> {
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

/// 列出所有可用主题
async fn handle_list_themes(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<crate::theme_preset::ThemePreset>>> {
    Json(ApiResponse::success(state.themes.clone()))
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::config::models::{
        Config, ModelConfig, ProviderConfig, ProviderOptions, ProviderType, ServerConfig,
    };
    use crate::provider::Provider;
    use std::collections::HashMap;

    /// 创建一个测试用的 Config，包含一个测试 Provider 和模型
    pub fn create_test_config() -> Config {
        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelConfig {
                name: "Test Model".to_string(),
                ..Default::default()
            },
        );

        let mut provider = HashMap::new();
        provider.insert(
            "test-provider".to_string(),
            ProviderConfig {
                provider_type: ProviderType::OpenAiCompatible,
                npm: "@test".to_string(),
                name: "Test Provider".to_string(),
                options: ProviderOptions {
                    api_key: "test-api-key".to_string(),
                    base_url: "http://localhost:11434".to_string(),
                    timeout: 300_000,
                    chunk_timeout: 10_000,
                    headers: None,
                },
                models,
            },
        );

        Config {
            model: "test-provider/test-model".to_string(),
            provider,
            mcp: None,
            server: Some(ServerConfig {
                port: Some(4040),
                api_token: Some("test-token".to_string()),
                allowed_origins: None,
            }),
            observability: None,
            source_path: Some("/test/config.json".to_string()),
        }
    }

    /// 创建一个测试用的 AppState
    pub fn create_test_app_state() -> AppState {
        let config = create_test_config();
        let config_arc = Arc::new(RwLock::new(config.clone()));

        let mut provider = Provider::default();
        provider
            .set_model("test-provider/test-model", &config)
            .unwrap();
        let provider_arc = Arc::new(RwLock::new(provider));

        let sessions = Arc::new(HttpSessionManager::new());
        let (commands, current_theme) =
            crate::server::commands::build_command_registry(sessions.clone());

        AppState {
            provider: provider_arc,
            config: config_arc,
            sessions,
            commands: Arc::new(commands),
            themes: crate::theme_preset::ThemePreset::all_presets(),
            current_theme,
            log_broadcaster: Some(Arc::new(crate::utils::log_store::LogBroadcaster::new(100))),
            session_manager: None,
        }
    }

    /// 创建一个没有 api_token 的 Config（用于测试无需认证的场景）
    pub fn create_test_config_no_auth() -> Config {
        let mut config = create_test_config();
        config.server = Some(ServerConfig {
            port: Some(4040),
            api_token: None,
            allowed_origins: None,
        });
        config
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use axum::http::HeaderMap;

    #[tokio::test]
    async fn test_check_auth_no_token_required() {
        let config = Arc::new(RwLock::new(create_test_config_no_auth()));
        let headers = HeaderMap::new();
        let result = check_auth(&headers, &config).await;
        assert!(result.is_none(), "无 token 配置时应直接通过");
    }

    #[tokio::test]
    async fn test_check_auth_missing_header() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let headers = HeaderMap::new();
        let result = check_auth(&headers, &config).await;
        assert!(result.is_some(), "缺少 Authorization 头应返回错误");
        let resp = result.unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().message, "Unauthorized");
    }

    #[tokio::test]
    async fn test_check_auth_invalid_token() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer wrong-token"),
        );
        let result = check_auth(&headers, &config).await;
        assert!(result.is_some(), "错误的 token 应返回错误");
    }

    #[tokio::test]
    async fn test_check_auth_valid_token() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test-token"),
        );
        let result = check_auth(&headers, &config).await;
        assert!(result.is_none(), "正确的 token 应通过认证");
    }

    #[tokio::test]
    async fn test_check_auth_malformed_header() {
        let config = Arc::new(RwLock::new(create_test_config()));
        let mut headers = HeaderMap::new();
        // 不以 "Bearer " 开头
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        let result = check_auth(&headers, &config).await;
        assert!(result.is_some(), "非 Bearer 认证应返回错误");
    }

    #[test]
    fn test_create_test_app_state() {
        let state = create_test_app_state();
        let provider = state.provider.read().unwrap();
        assert_eq!(provider.model_name().unwrap(), "test-model");
    }

    #[test]
    fn test_cors_layer_no_origins() {
        let config = Arc::new(RwLock::new(create_test_config_no_auth()));
        let layer = cors_layer(config);
        // CorsLayer::permissive 应该允许所有来源
        // 这里主要测试不 panic
        drop(layer);
    }
}
