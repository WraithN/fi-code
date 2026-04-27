use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::server::rpc::{JsonRpcRequest, JsonRpcResponse};
use crate::server::sse::SseEvent;

#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub message_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct SessionListResult {
    pub sessions: Vec<SessionInfo>,
    pub current_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct TuiClient {
    client: Client,
    base_url: String,
}

impl TuiClient {
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

    /// 执行指令（JSON-RPC）
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

    /// 对话（SSE）
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
}
