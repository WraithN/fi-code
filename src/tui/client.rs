use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::server::rpc::{JsonRpcRequest, JsonRpcResponse};
use crate::server::sse::SseEvent;

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
            Some(result) => Ok(result["message"]
                .as_str()
                .unwrap_or("OK")
                .to_string()),
            None => Err(anyhow!(
                resp.error.map(|e| e.message).unwrap_or_default()
            )),
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
}
