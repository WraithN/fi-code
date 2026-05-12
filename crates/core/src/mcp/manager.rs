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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::config::models::{McpServerConfig, McpServerType};
use crate::log_info;

use super::client::McpClient;
use super::transport::{LocalClient, RemoteClient};
use super::types::*;

// =============================================================================
// MCP 服务器状态
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum McpServerStatus {
    Healthy,
    Reconnecting,
    Failed(String),
}

// =============================================================================
// MCP 工具缓存（合并 tools_summary + tools_full）
// =============================================================================

#[derive(Default)]
struct McpToolCache {
    /// full_name -> description（轻量信息，用于生成 schema）
    summary: HashMap<String, String>,
    /// full_name -> 完整 McpTool（含 input_schema，用于 Two-Step-Discovery）
    full: HashMap<String, McpTool>,
}

// =============================================================================
// McpManager — 多服务器聚合、自动重连、状态监控
// =============================================================================
// 设计说明：
// - McpManager 本身会被 `Arc<McpManager>` 包裹后共享，因此内部字段不需要再套 `Arc`。
// - 使用 `tokio::sync::RwLock` 而非 `std::sync::RwLock`，避免在 async 上下文中阻塞线程。
// - `clients` 中存储 `Arc<dyn McpClient>` 而非 `Box<dyn McpClient>`，
//   使得调用方可以在获取 client 后立即释放锁，再执行耗时的 `call_tool().await`。
// - `tools_summary` 与 `tools_full` 合并为 `McpToolCache`，减少字段数量和锁竞争面。

pub struct McpManager {
    clients: tokio::sync::RwLock<HashMap<String, Arc<dyn McpClient>>>,
    configs: HashMap<String, McpServerConfig>,
    status: tokio::sync::RwLock<HashMap<String, McpServerStatus>>,
    tools: tokio::sync::RwLock<McpToolCache>,
    max_retries: u32,
}

impl McpManager {
    pub async fn from_config(config: &HashMap<String, McpServerConfig>) -> Result<Self> {
        let mut manager = Self {
            clients: tokio::sync::RwLock::new(HashMap::new()),
            configs: config.clone(),
            status: tokio::sync::RwLock::new(HashMap::new()),
            tools: tokio::sync::RwLock::new(McpToolCache::default()),
            max_retries: 3,
        };

        for (name, server_config) in config {
            if !server_config.enabled {
                continue;
            }

            match manager.connect_and_load_tools(name, server_config).await {
                Ok(()) => {
                    manager
                        .status
                        .write()
                        .await
                        .insert(name.clone(), McpServerStatus::Healthy);
                }
                Err(e) => {
                    log_info!(
                        "Warning: MCP server '{}' initialization failed: {}",
                        name,
                        e
                    );
                    manager
                        .status
                        .write()
                        .await
                        .insert(name.clone(), McpServerStatus::Failed(e.to_string()));
                }
            }
        }

        Ok(manager)
    }

    /// 创建客户端、完成初始化握手、获取工具列表并缓存
    async fn connect_and_load_tools(&self, name: &str, config: &McpServerConfig) -> Result<()> {
        let client = Self::create_client(name, config).await?;
        let list_result = client.list_tools().await?;

        {
            let mut clients = self.clients.write().await;
            clients.insert(name.to_string(), client);
        }

        {
            let mut tools = self.tools.write().await;
            for tool in list_result.tools {
                let full_name = format!("mcp:{}/{}", name, tool.name);
                tools
                    .summary
                    .insert(full_name.clone(), tool.description.clone());
                tools.full.insert(full_name, tool);
            }
        }

        Ok(())
    }

    /// 根据配置创建对应类型的客户端（不初始化）。
    /// 注意：这是一个纯函数，不依赖 `self` 的任何字段，因此设计为关联函数。
    /// 返回 `Arc<dyn McpClient>`，以便在锁外安全持有并调用。
    async fn create_client(name: &str, config: &McpServerConfig) -> Result<Arc<dyn McpClient>> {
        match config.server_type {
            McpServerType::Local => {
                let cmd = config
                    .command
                    .as_ref()
                    .ok_or_else(|| anyhow!("Local MCP server '{}' missing command", name))?;
                let mut client = LocalClient::new(cmd).await?;
                client.initialize().await?;
                Ok(Arc::new(client))
            }
            McpServerType::Remote => {
                let url = config
                    .url
                    .as_ref()
                    .ok_or_else(|| anyhow!("Remote MCP server '{}' missing url", name))?;
                let mut client = RemoteClient::new(url.clone(), config.headers.clone())?;
                client.initialize().await?;
                Ok(Arc::new(client))
            }
        }
    }

    /// 列出所有 MCP 工具的轻量信息（full_name -> description）
    pub async fn tools_list(&self) -> Vec<(String, String)> {
        let guard = self.tools.read().await;
        guard
            .summary
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// 获取指定工具的完整 schema
    pub async fn tool_schema(&self, full_name: &str) -> Option<McpTool> {
        self.tools.read().await.full.get(full_name).cloned()
    }

    /// 调用 MCP 工具
    pub async fn tool_call(
        &self,
        full_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        let parts: Vec<&str> = full_name
            .strip_prefix("mcp:")
            .ok_or_else(|| anyhow!("Invalid MCP tool name: {}", full_name))?
            .splitn(2, '/')
            .collect();

        if parts.len() != 2 {
            return Err(anyhow!(
                "MCP tool name format error, expected mcp:server/tool: {}",
                full_name
            ));
        }

        let server_name = parts[0];
        let tool_name = parts[1];

        // 检查状态
        {
            let status = self.status.read().await;
            if let Some(McpServerStatus::Failed(err)) = status.get(server_name) {
                return Err(anyhow!(
                    "MCP server '{}' is in failed state: {}",
                    server_name,
                    err
                ));
            }
        }

        // 获取 client 引用，立即释放锁，再执行耗时的 call_tool
        let client = {
            let clients = self.clients.read().await;
            clients
                .get(server_name)
                .cloned()
                .ok_or_else(|| anyhow!("MCP server '{}' not found", server_name))?
        };

        // 尝试调用
        match client.call_tool(tool_name, arguments.clone()).await {
            Ok(r) => Ok(r),
            Err(e) => {
                log_info!("MCP call failed: {}, triggering reconnect...", e);
                self.reconnect(server_name).await?;

                // 重连后重新获取 client
                let client = {
                    let clients = self.clients.read().await;
                    clients.get(server_name).cloned().ok_or_else(|| {
                        anyhow!("MCP server '{}' not found after reconnect", server_name)
                    })?
                };
                client.call_tool(tool_name, arguments).await
            }
        }
    }

    /// 自动重连：指数退避，最大 3 次
    async fn reconnect(&self, server_name: &str) -> Result<()> {
        {
            let mut status = self.status.write().await;
            status.insert(server_name.to_string(), McpServerStatus::Reconnecting);
        }

        let config = self
            .configs
            .get(server_name)
            .ok_or_else(|| anyhow!("No config for server: {}", server_name))?;

        for attempt in 1..=self.max_retries {
            tokio::time::sleep(Duration::from_secs(2_u64.pow(attempt - 1))).await;

            match Self::create_client(server_name, config).await {
                Ok(client) => {
                    let mut clients = self.clients.write().await;
                    clients.insert(server_name.to_string(), client);

                    let mut status = self.status.write().await;
                    status.insert(server_name.to_string(), McpServerStatus::Healthy);

                    log_info!("MCP server '{}' reconnected successfully", server_name);
                    return Ok(());
                }
                Err(e) => {
                    log_info!(
                        "MCP server '{}' reconnect failed (attempt {}/{}): {}",
                        server_name,
                        attempt,
                        self.max_retries,
                        e
                    );
                }
            }
        }

        let mut status = self.status.write().await;
        status.insert(
            server_name.to_string(),
            McpServerStatus::Failed("Reconnect exhausted".to_string()),
        );
        Err(anyhow!(
            "MCP server '{}' reconnect failed after {} attempts",
            server_name,
            self.max_retries
        ))
    }

    pub async fn server_status(&self, name: &str) -> Option<McpServerStatus> {
        self.status.read().await.get(name).cloned()
    }

    pub async fn all_status(&self) -> Vec<(String, McpServerStatus)> {
        self.status
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::{McpServerConfig, McpServerType};
    use std::collections::HashMap;

    fn mock_server_command() -> Vec<String> {
        vec![
            "npx".to_string(),
            "-y".to_string(),
            "@modelcontextprotocol/server-everything".to_string(),
        ]
    }

    /// 检查 npx 是否可用，若不可用则 panic 并给出明确提示。
    fn ensure_npx_available() {
        if std::process::Command::new("npx")
            .arg("--version")
            .output()
            .is_err()
        {
            panic!(
                "npx is not available in PATH. \
                 MCP tests require Node.js/npm to install the mock server."
            );
        }
    }

    #[tokio::test]
    async fn test_mcp_manager_mock_server() {
        ensure_npx_available();

        let mut config = HashMap::new();
        config.insert(
            "mock".to_string(),
            McpServerConfig {
                server_type: McpServerType::Local,
                enabled: true,
                command: Some(mock_server_command()),
                url: None,
                headers: None,
            },
        );

        let manager = McpManager::from_config(&config).await.unwrap();

        // 验证工具列表（server-everything 提供了 12 个工具）
        let tools = manager.tools_list().await;
        assert!(
            tools.len() >= 2,
            "expected at least 2 tools, got: {:?}",
            tools
        );
        assert!(tools.iter().any(|(n, _)| n == "mcp:mock/echo"));
        assert!(tools.iter().any(|(n, _)| n == "mcp:mock/get-sum"));

        // 验证 tool_schema
        let schema = manager.tool_schema("mcp:mock/echo").await;
        assert!(schema.is_some());
        let schema = schema.unwrap();
        assert_eq!(schema.name, "echo");
        assert!(schema.input_schema.is_some());

        // 验证工具调用：echo
        let result = manager
            .tool_call("mcp:mock/echo", serde_json::json!({"message": "hello"}))
            .await
            .unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "Echo: hello");

        // 验证工具调用：get-sum
        let result = manager
            .tool_call("mcp:mock/get-sum", serde_json::json!({"a": 3, "b": 5}))
            .await
            .unwrap();
        assert_eq!(result.content[0].text, "The sum of 3 and 5 is 8.");
    }

    #[tokio::test]
    async fn test_mcp_manager_disabled_server() {
        let mut config = HashMap::new();
        config.insert(
            "disabled".to_string(),
            McpServerConfig {
                server_type: McpServerType::Local,
                enabled: false,
                command: Some(mock_server_command()),
                url: None,
                headers: None,
            },
        );

        let manager = McpManager::from_config(&config).await.unwrap();
        assert!(manager.tools_list().await.is_empty());
    }
}
