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
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::commands::registry::{CommandMeta, CommandOutput};
use crate::server::rpc::{JsonRpcRequest, JsonRpcResponse};
use crate::server::sse::SseEvent;

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

/// 通用 REST API 响应包装器。
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

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
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:4040".to_string(),
        }
    }

    /// 获取当前模型名
    pub async fn get_status(&self) -> Result<String> {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "get_status".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let resp = self
            .client
            .post(format!("{}/rpc", self.base_url))
            .json(&req)
            .send()
            .await?
            .json::<JsonRpcResponse>()
            .await?;

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
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "execute".to_string(),
            params: Some(json!({ "command": command })),
            id: Some(json!(1)),
        };

        let resp = self
            .client
            .post(format!("{}/rpc", self.base_url))
            .json(&req)
            .send()
            .await?
            .json::<JsonRpcResponse>()
            .await?;

        match resp.result {
            Some(result) => Ok(result["message"].as_str().unwrap_or("OK").to_string()),
            None => Err(anyhow!(resp.error.map(|e| e.message).unwrap_or_default())),
        }
    }

    /// 发起对话并接收 SSE 流式响应。
    ///
    /// 解析每行 `data: ` 后的 JSON，转换为 `SseEvent` 并通过 channel 实时发送。
    /// 当收到 `SseEvent::Done` 时返回最终的 `session_id`。
    pub async fn chat(
        &self,
        session_id: Option<String>,
        message: String,
        tx: mpsc::Sender<SseEvent>,
    ) -> Result<String> {
        let req_body = json!({
            "session_id": session_id,
            "message": message
        });

        let response = self
            .client
            .post(format!("{}/chat", self.base_url))
            .json(&req_body)
            .send()
            .await?;

        let mut stream = response.bytes_stream();
        let mut final_session_id = session_id.clone();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if let Ok(text) = std::str::from_utf8(&chunk) {
                for line in text.lines() {
                    if line.starts_with("data: ") {
                        let json_str = &line[6..];
                        if let Ok(event) = serde_json::from_str::<SseEvent>(json_str) {
                            if let SseEvent::Done { session_id: sid } = &event {
                                final_session_id = Some(sid.clone());
                            }
                            let is_done = matches!(event, SseEvent::Done { .. });
                            tx.send(event).await?;
                            if is_done {
                                return Ok(final_session_id.unwrap_or_default());
                            }
                        }
                    }
                }
            }
        }

        Ok(final_session_id.unwrap_or_default())
    }

    /// 获取所有会话列表。
    pub async fn list_sessions(&self) -> anyhow::Result<SessionListResult> {
        let resp = self
            .client
            .get(format!("{}/api/sessions", self.base_url))
            .send()
            .await?
            .json::<ApiResponse<SessionListResult>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 创建新会话。
    pub async fn create_session(&self, name: &str) -> anyhow::Result<SessionInfo> {
        let body = serde_json::json!({"name": name});
        let resp = self
            .client
            .post(format!("{}/api/sessions", self.base_url))
            .json(&body)
            .send()
            .await?
            .json::<ApiResponse<SessionInfo>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 切换到指定会话。
    pub async fn switch_session(&self, id: &str) -> anyhow::Result<SessionInfo> {
        let resp = self
            .client
            .post(format!("{}/api/sessions/{}/switch", self.base_url, id))
            .send()
            .await?
            .json::<ApiResponse<SessionInfo>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取指定路径下的文件树。
    pub async fn get_file_tree(&self, path: &str) -> anyhow::Result<FileTreeResult> {
        let resp = self
            .client
            .get(format!("{}/api/files?path={}", self.base_url, path))
            .send()
            .await?
            .json::<ApiResponse<FileTreeResult>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    /// 获取所有可用命令的元数据列表
    pub async fn list_commands(&self) -> Result<Vec<CommandMeta>> {
        let resp = self
            .client
            .get(format!("{}/api/commands", self.base_url))
            .send()
            .await?
            .json::<ApiResponse<Vec<CommandMeta>>>()
            .await?;

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
        let body = serde_json::json!({
            "args": args,
            "session_id": session_id,
        });

        let resp = self
            .client
            .post(format!("{}/api/commands/{}/execute", self.base_url, name))
            .json(&body)
            .send()
            .await?
            .json::<ApiResponse<CommandOutput>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }
}
