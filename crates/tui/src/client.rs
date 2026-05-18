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
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json};
use tokio::sync::mpsc;

use fi_code_core::commands::registry::CommandOutput;
use fi_code_shared::dto::CommandMeta;
use fi_code_core::log_debug;
use fi_code_core::log_info;
use fi_code_core::log_warn;
use fi_code_core::server::transport::rpc::{JsonRpcRequest, JsonRpcResponse};
use fi_code_core::server::transport::sse::SseEvent;
use fi_code_shared::tui_event::{AppEvent, LogLevel, LogLine};
use fi_code_core::utils::log_store::LogEntry;

/// 单个会话的元信息。
#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub message_count: usize,
}

/// 会话列表接口的返回结构。
#[derive(Debug, Deserialize)]
pub struct SessionListResult {
    pub sessions: Vec<SessionInfo>,
    pub current_session_id: Option<String>,
}

// 已从 fi-code-shared crate 重新导出，保留此 re-export 维持向后兼容
pub use fi_code_shared::constants::*;
use fi_code_shared::dto::ApiResponse;

/// 文件树中的单个节点。
#[derive(Debug, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

/// 文件树接口的返回结构。
#[derive(Debug, Deserialize)]
pub struct FileTreeResult {
    pub root: String,
    pub entries: Vec<FileEntry>,
}

/// TUI 与后端服务通信的 HTTP 客户端。
///
/// 后端默认监听 `localhost:4040`，提供两类接口：
/// - JSON-RPC（`/rpc`）：状态查询、执行命令。
/// - REST（`/api/*`）：会话管理、文件树。
/// - SSE（`/chat`）：流式对话。
#[derive(Clone)]
pub struct TuiClient {
    client: Client,
    base_url: String,
}

impl TuiClient {
    /// 创建客户端，默认连接 `http://localhost:4040`。
    /// 配置 tcp_nodelay 禁用 Nagle 算法，对小数据包（SSE token）更友好。
    pub fn new() -> Self {
        let client = Client::builder()
            .tcp_nodelay(true)
            .timeout(std::time::Duration::from_secs(TUI_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(TUI_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: format!("http://localhost:{}", DEFAULT_SERVER_PORT),
        }
    }

    /// 使用指定的 base URL 创建客户端（主要用于测试）。
    pub fn with_base_url(base_url: &str) -> Self {
        let client = Client::builder()
            .tcp_nodelay(true)
            .timeout(std::time::Duration::from_secs(TUI_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(TUI_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    /// 获取当前模型名
    pub async fn get_status(&self) -> Result<String> {
        let url = format!("{}/rpc", self.base_url);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "get_status".to_string(),
            params: None,
            id: Some(json!(1)),
        };
        log_debug!("[Client] HTTP -> POST {} | method=get_status", url);

        let resp = self.client.post(&url).json(&req).send().await?;
        let status = resp.status();
        let resp = resp.json::<JsonRpcResponse>().await?;
        log_debug!(
            "[Client] HTTP <- POST {} | status={} | result={:?}",
            url,
            status,
            resp.result.is_some()
        );

        match resp.result {
            Some(result) => Ok(result["current_model"]
                .as_str()
                .unwrap_or("unknown")
                .to_string()),
            None => Err(anyhow!(resp.error.map(|e| e.message).unwrap_or_default())),
        }
    }

    /// 向后端发送执行指令（JSON-RPC）。
    ///
    /// 返回执行结果中的 `message` 字段。
    pub async fn execute(&self, command: &str) -> Result<String> {
        let url = format!("{}/rpc", self.base_url);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "execute".to_string(),
            params: Some(json!({ "command": command })),
            id: Some(json!(1)),
        };
        log_debug!(
            "[Client] HTTP -> POST {} | method=execute | command={}",
            url,
            command
        );

        let resp = self.client.post(&url).json(&req).send().await?;
        let status = resp.status();
        let resp = resp.json::<JsonRpcResponse>().await?;
        log_debug!(
            "[Client] HTTP <- POST {} | status={} | result={:?}",
            url,
            status,
            resp.result.is_some()
        );

        match resp.result {
            Some(result) => Ok(result["message"].as_str().unwrap_or("OK").to_string()),
            None => Err(anyhow!(resp.error.map(|e| e.message).unwrap_or_default())),
        }
    }

    /// 发起对话并接收 SSE 流式响应。
    ///
    /// 解析每行 `data: ` 后的 JSON，转换为 `AppEvent::SseEvent` 并通过 channel 实时发送到主事件循环。
    /// 当收到 `SseEvent::Done` 时返回最终的 `session_id`。
    pub async fn chat(
        &self,
        session_id: Option<String>,
        message: String,
        agent_type: fi_code_shared::dto::AgentType,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<String> {
        let url = format!("{}/chat", self.base_url);
        let req_body = json!({
            "session_id": session_id,
            "message": message,
            "agent": agent_type
        });
        log_info!(
            "[Client] HTTP -> POST {} | session_id={:?} | message_len={}",
            url,
            session_id,
            message.len()
        );

        let response = self.client.post(&url).json(&req_body).send().await?;
        log_debug!(
            "[Client] HTTP <- POST {} | status={}",
            url,
            response.status()
        );

        let mut stream = response.bytes_stream();
        let mut final_session_id = session_id.clone();
        let mut buffer = String::new();
        let mut event_data: Vec<String> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer.drain(..=pos).collect::<String>();
                let line = line.trim_end();

                if line.starts_with("data: ") {
                    event_data.push(line[6..].to_string());
                } else if line.is_empty() && !event_data.is_empty() {
                    // SSE 事件结束（空行），合并所有 data: 行
                    let json_str = event_data.join("\n");
                    event_data.clear();

                    if let Ok(event) = serde_json::from_str::<SseEvent>(&json_str) {
                        let event_preview = match &event {
                            SseEvent::Message { content } => {
                                format!("Message(len={})", content.len())
                            }
                            SseEvent::Part { part } => {
                                format!("Part({:?})", part)
                            }
                            SseEvent::TaskProgress { plan_id, tasks } => {
                                format!("TaskProgress(plan={} tasks={})", plan_id, tasks.len())
                            }
                            SseEvent::Error { message } => {
                                format!("Error(msg={})", message)
                            }
                            SseEvent::Done { .. } => "Done".to_string(),
                            SseEvent::AgentInfo { agent_type, agent_name } => {
                                format!("AgentInfo(type={:?} name={})", agent_type, agent_name)
                            }
                        };
                        log_debug!("[Client] HTTP SSE event | {}", event_preview);
                        if let SseEvent::Done { session_id: sid } = &event {
                            final_session_id = Some(sid.clone());
                        }
                        let is_done = matches!(event, SseEvent::Done { .. });
                        let _ = tx.send(AppEvent::SseEvent(event)).await;
                        if is_done {
                            log_info!(
                                "[Client] HTTP SSE stream done | session_id={:?}",
                                final_session_id
                            );
                            return Ok(final_session_id.unwrap_or_default());
                        }
                    } else {
                        log_warn!(
                            "[Client] HTTP SSE invalid JSON | len={} | prefix={:?}",
                            json_str.len(),
                            &json_str[..json_str.len().min(200)]
                        );
                    }
                }
                // 其他行（如注释、空行）忽略
            }
        }

        log_warn!("[Client] HTTP SSE stream ended without Done");
        Ok(final_session_id.unwrap_or_default())
    }

    /// 获取所有会话列表。
    pub async fn list_sessions(&self) -> anyhow::Result<SessionListResult> {
        let url = format!("{}/api/sessions", self.base_url);
        log_debug!("HTTP -> GET {}", url);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<SessionListResult>>().await?;
        log_debug!(
            "HTTP <- GET {} | status={} | sessions={}",
            url,
            status,
            resp.data.as_ref().map(|d| d.sessions.len()).unwrap_or(0)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 创建新会话。
    pub async fn create_session(&self, name: &str) -> anyhow::Result<SessionInfo> {
        let url = format!("{}/api/sessions", self.base_url);
        let body = serde_json::json!({"name": name});
        log_debug!("HTTP -> POST {} | name={}", url, name);

        let resp = self.client.post(&url).json(&body).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<SessionInfo>>().await?;
        log_debug!(
            "HTTP <- POST {} | status={} | id={:?}",
            url,
            status,
            resp.data.as_ref().map(|d| &d.id)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 切换到指定会话。
    pub async fn switch_session(&self, id: &str) -> anyhow::Result<SessionInfo> {
        let url = format!("{}/api/sessions/{}/switch", self.base_url, id);
        log_debug!("HTTP -> POST {} | id={}", url, id);

        let resp = self.client.post(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<SessionInfo>>().await?;
        log_debug!(
            "HTTP <- POST {} | status={} | result={:?}",
            url,
            status,
            resp.data.as_ref().map(|d| &d.id)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取指定路径下的文件树。
    pub async fn get_file_tree(&self, path: &str) -> anyhow::Result<FileTreeResult> {
        let url = format!("{}/api/files?path={}", self.base_url, path);
        log_debug!("HTTP -> GET {} | path={}", url, path);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<FileTreeResult>>().await?;
        log_debug!(
            "HTTP <- GET {} | status={} | entries={}",
            url,
            status,
            resp.data.as_ref().map(|d| d.entries.len()).unwrap_or(0)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取所有可用命令的元数据列表
    pub async fn list_commands(&self) -> Result<Vec<CommandMeta>> {
        let url = format!("{}/api/commands", self.base_url);
        log_debug!("HTTP -> GET {}", url);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<Vec<CommandMeta>>>().await?;
        log_debug!(
            "HTTP <- GET {} | status={} | count={}",
            url,
            status,
            resp.data.as_ref().map(|d| d.len()).unwrap_or(0)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 执行指定命令
    pub async fn execute_command(
        &self,
        name: &str,
        args: Option<String>,
        session_id: Option<String>,
    ) -> Result<CommandOutput> {
        let url = format!("{}/api/commands/{}/execute", self.base_url, name);
        let body = serde_json::json!({
            "args": args,
            "session_id": session_id,
        });
        log_debug!(
            "HTTP -> POST {} | name={} | session_id={:?}",
            url,
            name,
            session_id
        );

        let resp = self.client.post(&url).json(&body).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<CommandOutput>>().await?;
        log_debug!(
            "HTTP <- POST {} | status={} | result={:?}",
            url,
            status,
            resp.data.as_ref().map(|d| d.r#type.clone())
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取所有可用主题预设列表
    pub async fn list_themes(&self) -> Result<Vec<fi_code_shared::dto::ThemePreset>> {
        let url = format!("{}/api/themes", self.base_url);
        log_debug!("HTTP -> GET {}", url);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp
            .json::<ApiResponse<Vec<fi_code_shared::dto::ThemePreset>>>()
            .await?;
        log_debug!(
            "HTTP <- GET {} | status={} | count={}",
            url,
            status,
            resp.data.as_ref().map(|d| d.len()).unwrap_or(0)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    pub async fn get_logs(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let url = format!("{}/api/logs?limit={}", self.base_url, limit);
        log_debug!("HTTP -> GET {} | limit={}", url, limit);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<Vec<LogEntry>>>().await?;
        log_debug!(
            "HTTP <- GET {} | status={} | count={}",
            url,
            status,
            resp.data.as_ref().map(|d| d.len()).unwrap_or(0)
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取当前配置摘要（config.json 路径、Provider 信息等）。
    pub async fn get_config(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/config", self.base_url);
        log_debug!("[Client] HTTP -> GET {}", url);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<serde_json::Value>>().await?;
        log_debug!(
            "[Client] HTTP <- GET {} | status={} | has_data={}",
            url,
            status,
            resp.data.is_some()
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取所有可用模型列表（按 Provider 分组）。
    pub async fn list_models(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/models", self.base_url);
        log_debug!("HTTP -> GET {}", url);

        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<serde_json::Value>>().await?;
        log_debug!(
            "HTTP <- GET {} | status={} | has_data={}",
            url,
            status,
            resp.data.is_some()
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 切换当前使用的模型。
    pub async fn switch_model(
        &self,
        provider: &str,
        model: &str,
        api_key: Option<&str>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/api/model/switch", self.base_url);
        let body = serde_json::json!({
            "provider": provider,
            "model": model,
            "api_key": api_key.is_some(),
        });
        log_info!(
            "HTTP -> POST {} | provider={} | model={}",
            url,
            provider,
            model
        );

        let resp = self.client.post(&url).json(&body).send().await?;
        let status = resp.status();
        let resp = resp.json::<ApiResponse<serde_json::Value>>().await?;
        log_info!(
            "HTTP <- POST {} | status={} | success={}",
            url,
            status,
            resp.success
        );

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    pub async fn subscribe_logs(&self, tx: mpsc::Sender<AppEvent>) -> Result<()> {
        let url = format!("{}/api/logs/stream", self.base_url);
        log_debug!("[Client] HTTP -> GET {} | subscribe_logs", url);
        let response = self.client.get(&url).send().await?;
        log_debug!(
            "[Client] HTTP <- GET {} | status={}",
            url,
            response.status()
        );

        let mut stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find("\n\n") {
                let event = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                if let Some(data) = event.strip_prefix("data: ") {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(data.trim()) {
                        let line = LogLine {
                            timestamp: entry.timestamp,
                            level: match entry.level.as_str() {
                                "DEBUG" => LogLevel::Debug,
                                "TRACE" => LogLevel::Trace,
                                "ERROR" => LogLevel::Error,
                                _ => LogLevel::Info,
                            },
                            module: entry.module,
                            message: entry.message,
                        };
                        let _ = tx.send(AppEvent::AppendLog(line)).await;
                    }
                }
            }
        }

        let _ = tx.send(AppEvent::LogDisconnected).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_session_info_deserialize() {
        let json = r#"{"id":"sess_123","name":"test session","message_count":42}"#;
        let info: SessionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "sess_123");
        assert_eq!(info.name, "test session");
        assert_eq!(info.message_count, 42);
    }

    #[test]
    fn test_session_list_result_deserialize() {
        let json = r#"{
            "sessions": [
                {"id":"s1","name":"Session 1","message_count":5}
            ],
            "current_session_id": "s1"
        }"#;
        let result: SessionListResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].id, "s1");
        assert_eq!(result.current_session_id, Some("s1".to_string()));
    }

    #[test]
    fn test_api_response_success_deserialize() {
        let json = r#"{"success":true,"data":{"key":"value"},"error":null}"#;
        let resp: ApiResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_api_response_error_deserialize() {
        let json = r#"{"success":false,"data":null,"error":"something went wrong"}"#;
        let resp: ApiResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert_eq!(resp.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_file_entry_deserialize() {
        let json = r#"{"path":"/home/user/project/src/main.rs","name":"main.rs","is_dir":false,"depth":2}"#;
        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.path, "/home/user/project/src/main.rs");
        assert_eq!(entry.name, "main.rs");
        assert!(!entry.is_dir);
        assert_eq!(entry.depth, 2);
    }

    #[test]
    fn test_file_tree_result_deserialize() {
        let json = r#"{
            "root": "/home/user/project",
            "entries": [
                {"path":"/home/user/project/src","name":"src","is_dir":true,"depth":0},
                {"path":"/home/user/project/src/main.rs","name":"main.rs","is_dir":false,"depth":1}
            ]
        }"#;
        let result: FileTreeResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.root, "/home/user/project");
        assert_eq!(result.entries.len(), 2);
        assert!(result.entries[0].is_dir);
        assert!(!result.entries[1].is_dir);
    }

    #[test]
    fn test_tui_client_new() {
        let client = TuiClient::new();
        // TuiClient 没有公开字段，只能验证创建成功
        // clone 需要实现 Clone trait，已在 struct 上标注
        let _cloned = client.clone();
    }

    #[test]
    fn test_tui_client_with_base_url() {
        let client = TuiClient::with_base_url("http://localhost:9999");
        let _cloned = client.clone();
    }
}
