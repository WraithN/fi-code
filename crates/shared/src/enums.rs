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

// =============================================================================
// 共享枚举：前后端及跨模块共享的基础枚举类型
// =============================================================================
// 本模块集中管理所有需要跨 crate 或前后端序列化传输的枚举，
// 避免定义分散导致的版本不一致问题。

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// 对话角色枚举
// -----------------------------------------------------------------------------

/// 对话角色枚举。
/// - `User`：人类用户
/// - `Assistant`：AI 助手
/// - `System`：系统级提示（如环境描述）
/// - `Developer`：开发者消息（部分模型支持，如 Claude Code 风格）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

// -----------------------------------------------------------------------------
// Provider API 类型枚举
// -----------------------------------------------------------------------------

/// Provider API 类型
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ProviderType {
    #[default]
    OpenAiCompatible,
    Anthropic,
}

// -----------------------------------------------------------------------------
// MCP 服务器类型枚举
// -----------------------------------------------------------------------------

/// MCP 服务器类型
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum McpServerType {
    #[default]
    Local,
    Remote,
}

// -----------------------------------------------------------------------------
// 模型生成停止原因枚举
// -----------------------------------------------------------------------------

/// 模型生成停止的统一抽象。
/// OpenAI 使用 `finish_reason` 字段（如 `stop`、`tool_calls`），
/// Anthropic 使用 `stop_reason` 字段（如 `end_turn`、`tool_use`）。
/// 本枚举将两者映射到同一语义层，方便上层业务统一判断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// 自然结束对话回合（OpenAI: `stop`，Anthropic: `end_turn`)
    Stop,
    /// 达到最大 token 限制（OpenAI: `length`，Anthropic: `max_tokens`)
    Length,
    /// 因为需要调用工具而停止（OpenAI: `tool_calls`，Anthropic: `tool_use`)
    ToolUse,
    /// 命中预设的停止序列（Anthropic: `stop_sequence`)
    StopSequence,
    /// 内容被安全过滤拦截（OpenAI: `content_filter`)
    ContentFilter,
    /// 其他未知或未标准化的原因
    Other(String),
}

impl FinishReason {
    /// 从 OpenAI 的 `finish_reason` 字符串转换为统一枚举
    pub fn from_openai(reason: &str) -> Self {
        match reason {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "tool_calls" => FinishReason::ToolUse,
            "content_filter" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_string()),
        }
    }

    /// 从 Anthropic 的 `stop_reason` 字符串转换为统一枚举
    pub fn from_anthropic(reason: &str) -> Self {
        match reason {
            "end_turn" => FinishReason::Stop,
            "max_tokens" => FinishReason::Length,
            "tool_use" => FinishReason::ToolUse,
            "stop_sequence" => FinishReason::StopSequence,
            other => FinishReason::Other(other.to_string()),
        }
    }
}
