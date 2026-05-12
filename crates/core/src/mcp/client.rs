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

use anyhow::Result;
use async_trait::async_trait;

use super::types::{CallToolResult, InitializeResult, ListToolsResult};

// =============================================================================
// McpClient Trait
// =============================================================================
// 所有 MCP 客户端（stdio / HTTP）都必须实现此 trait。
// `Send + Sync` 确保客户端可以安全地跨线程共享。

#[async_trait]
pub trait McpClient: Send + Sync {
    /// 初始化握手。必须在首次使用客户端前调用。
    async fn initialize(&mut self) -> Result<InitializeResult>;

    /// 获取服务器提供的所有工具列表。
    async fn list_tools(&self) -> Result<ListToolsResult>;

    /// 调用指定工具。
    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult>;
}
