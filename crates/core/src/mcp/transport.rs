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

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use super::client::McpClient;
use super::types::*;

// =============================================================================
// LocalClient — 通过 stdio 子进程与 MCP 服务器通信
// =============================================================================

pub struct LocalClient {
    process: Child,
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    stdout: tokio::sync::Mutex<BufReader<tokio::process::ChildStdout>>,
    request_id: AtomicU64,
}

impl LocalClient {
    pub async fn new(command: &[String]) -> Result<Self> {
        if command.is_empty() {
            return Err(anyhow!("Local MCP server command is empty"));
        }

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut process = cmd.spawn().context("Failed to spawn MCP local process")?;
        let stdin = process.stdin.take().unwrap();
        let stdout = BufReader::new(process.stdout.take().unwrap());

        Ok(Self {
            process,
            stdin: tokio::sync::Mutex::new(stdin),
            stdout: tokio::sync::Mutex::new(stdout),
            request_id: AtomicU64::new(1),
        })
    }

    async fn send_request<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&request)? + "\n";
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(json.as_bytes()).await?;
            stdin.flush().await?;
        }

        let mut line = String::new();
        {
            let mut stdout = self.stdout.lock().await;
            stdout.read_line(&mut line).await?;
        }

        let response: JsonRpcResponse<R> = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse MCP response: {}", line.trim()))?;

        match response.result {
            JsonRpcResult::Success { result } => Ok(result),
            JsonRpcResult::Error { error } => {
                Err(anyhow!("MCP error {}: {}", error.code, error.message))
            }
        }
    }
}

impl Drop for LocalClient {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
    }
}

#[async_trait]
impl McpClient for LocalClient {
    async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: false,
                }),
            },
            client_info: ClientInfo {
                name: "fi-code".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        self.send_request("initialize", params).await
    }

    async fn list_tools(&self) -> Result<ListToolsResult> {
        self.send_request("tools/list", serde_json::json!({})).await
    }

    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };
        self.send_request("tools/call", params).await
    }
}

// =============================================================================
// RemoteClient — 通过 HTTP POST 与 MCP 服务器通信
// =============================================================================

pub struct RemoteClient {
    client: reqwest::Client,
    url: String,
    headers: HashMap<String, String>,
    request_id: AtomicU64,
}

impl RemoteClient {
    pub fn new(url: String, headers: Option<HashMap<String, String>>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            url,
            headers: headers.unwrap_or_default(),
            request_id: AtomicU64::new(1),
        })
    }

    async fn send_request<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let mut req = self.client.post(&self.url).json(&request);
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req.send().await?;
        let response: JsonRpcResponse<R> = response.json().await?;

        match response.result {
            JsonRpcResult::Success { result } => Ok(result),
            JsonRpcResult::Error { error } => {
                Err(anyhow!("MCP error {}: {}", error.code, error.message))
            }
        }
    }
}

#[async_trait]
impl McpClient for RemoteClient {
    async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: false,
                }),
            },
            client_info: ClientInfo {
                name: "fi-code".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        self.send_request("initialize", params).await
    }

    async fn list_tools(&self) -> Result<ListToolsResult> {
        self.send_request("tools/list", serde_json::json!({})).await
    }

    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };
        self.send_request("tools/call", params).await
    }
}
