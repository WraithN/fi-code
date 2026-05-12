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

// provider 模块：封装与 AI Provider 相关的功能

pub mod base_client;
pub mod client;
pub mod mock_client;
pub mod provider;

// 重新导出常用类型，简化外部使用
pub use base_client::{
    extract_text, send_with_retry, AIClient, ApiResponse, Chunk, ChunkContent, FinishReason,
    RetryConfig,
};
pub use client::{AnthropicClient, OpenAiClient};
pub use mock_client::MockAIClient;
pub use provider::Provider;

// 从 tools 模块重新导出工具调用函数
#[allow(unused_imports)]
pub use crate::tools::{execute_tool_calls, tool_call};
