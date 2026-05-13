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
// message 模块：定义对话核心数据结构与消息构造器
// =============================================================================
// 本模块抽离了与消息内容相关的通用类型：
// - `Role`：消息发送者角色
// - `Part`：单条消息的内容块（文本、图片、工具调用、工具结果、推理）
// - `ImageSource`：图片数据来源
// - `Message`：包含完整元数据的消息结构
// - `MessageBuilder`：用于从持久化记录流式重建 Message 的构造器

use serde::{Deserialize, Serialize};

// =============================================================================
// 角色枚举
// =============================================================================

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

// =============================================================================
// Token 使用量统计
// =============================================================================

/// Token 使用量统计，用于 WaveMarker 等场景。
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

// =============================================================================
// 内容块枚举（Part）：消息的原子组成单元
// =============================================================================

/// 内容块枚举：一条 `Message` 由多个 `Part` 按顺序组成。
///
/// 这种设计与 Anthropic / OpenAI 的最新内容块 API 对齐，
/// 支持纯文本、多模态图片、工具调用、工具结果、推理过程、
/// 波浪标记以及用量统计。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// 纯文本内容
    Text { text: String },
    /// 图片内容，支持本地路径、Base64 数据或远程 URL
    Image { source: ImageSource },
    /// 工具调用请求（由 Assistant 发起）
    ToolUse {
        id: String,
        name: String,
        /// 工具参数，使用 `serde_json::Value` 保持灵活性
        arguments: serde_json::Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    /// 工具执行错误（由 User 角色消息携带，回传给模型）
    ToolError {
        tool_call_id: String,
        content: String,
        error_message: String,
    },
    /// 推理/思考过程（如 Claude Extended Thinking）
    Reasoning {
        thinking: String,
        /// 可选的签名，用于验证推理内容未被篡改
        signature: Option<String>,
    },
    /// 波浪标记，用于标识 Agent 执行步骤
    WaveMarker {
        step: u32,
        total: Option<u32>,
        git_snapshot: Option<String>,
        timestamp: u64,
        delta_tokens: TokenUsage,
    },
    /// 用量统计
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
        cost: Option<f64>,
    },
}

/// 图片来源枚举，对应 Part::Image 的 source 字段。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// 本地文件系统路径
    Path { path: String },
    /// Base64 编码的图片数据
    Base64 { media_type: String, data: String },
    /// 远程图片 URL
    Url { url: String },
}

// =============================================================================
// 消息结构体（Message）
// =============================================================================

/// 对话消息结构体，用于在多轮对话中保存角色与内容块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Message {
    /// 便捷构造方法，自动生成 ULID id 与当前时间戳。
    pub fn new(session_id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            session_id: session_id.into(),
            role,
            created_at: current_timestamp_ms(),
            parts,
            token_count: None,
            cost: None,
        }
    }
}

/// 获取当前 Unix 时间戳（毫秒）。
/// 使用 `std::time::SystemTime` 避免引入额外依赖（如 chrono）。
pub fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// =============================================================================
// MessageBuilder：用于从持久化记录流式重建 Message
// =============================================================================

/// 消息构造器，在 `load_session` 过程中暂存一个 Message 的中间状态。
pub struct MessageBuilder {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
}

impl MessageBuilder {
    pub fn new(
        id: impl Into<String>,
        session_id: impl Into<String>,
        role: Role,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            session_id: session_id.into(),
            role,
            created_at,
            parts: Vec::new(),
        }
    }

    /// 向当前消息追加一个 Part。
    pub fn add_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    /// 完成消息构造，合并可选的 token_count 和 cost。
    pub fn finalize(self, token_count: Option<u64>, cost: Option<f64>) -> Message {
        Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            created_at: self.created_at,
            parts: self.parts,
            token_count,
            cost,
        }
    }
}

// =============================================================================
// 单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 WaveMarker 的序列化与反序列化
    #[test]
    fn test_wave_marker_serde() {
        let part = Part::WaveMarker {
            step: 1,
            total: Some(3),
            git_snapshot: Some("abc123".to_string()),
            timestamp: 1715600000000,
            delta_tokens: TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
            },
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"wave_marker\""));
        assert!(json.contains("\"step\":1"));
        assert!(json.contains("\"total\":3"));
        assert!(json.contains("\"git_snapshot\":\"abc123\""));
        assert!(json.contains("\"timestamp\":1715600000000"));
        assert!(json.contains("\"prompt_tokens\":100"));
        assert!(json.contains("\"completion_tokens\":50"));

        let deserialized: Part = serde_json::from_str(&json).unwrap();
        match deserialized {
            Part::WaveMarker {
                step,
                total,
                git_snapshot,
                timestamp,
                delta_tokens,
            } => {
                assert_eq!(step, 1);
                assert_eq!(total, Some(3));
                assert_eq!(git_snapshot, Some("abc123".to_string()));
                assert_eq!(timestamp, 1715600000000);
                assert_eq!(delta_tokens.prompt_tokens, 100);
                assert_eq!(delta_tokens.completion_tokens, 50);
            }
            _ => panic!("expected WaveMarker variant"),
        }
    }

    /// 测试 ToolError 的序列化与反序列化
    #[test]
    fn test_tool_error_serde() {
        let part = Part::ToolError {
            tool_call_id: "call_123".to_string(),
            content: "some output".to_string(),
            error_message: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"tool_error\""));
        assert!(json.contains("\"tool_call_id\":\"call_123\""));
        assert!(json.contains("\"content\":\"some output\""));
        assert!(json.contains("\"error_message\":\"something went wrong\""));

        let deserialized: Part = serde_json::from_str(&json).unwrap();
        match deserialized {
            Part::ToolError {
                tool_call_id,
                content,
                error_message,
            } => {
                assert_eq!(tool_call_id, "call_123");
                assert_eq!(content, "some output");
                assert_eq!(error_message, "something went wrong");
            }
            _ => panic!("expected ToolError variant"),
        }
    }

    /// 测试 Usage 的序列化与反序列化
    #[test]
    fn test_usage_serde() {
        let part = Part::Usage {
            input_tokens: 1024,
            output_tokens: 512,
            latency_ms: 1200,
            cost: Some(0.003),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"usage\""));
        assert!(json.contains("\"input_tokens\":1024"));
        assert!(json.contains("\"output_tokens\":512"));
        assert!(json.contains("\"latency_ms\":1200"));
        assert!(json.contains("\"cost\":0.003"));

        let deserialized: Part = serde_json::from_str(&json).unwrap();
        match deserialized {
            Part::Usage {
                input_tokens,
                output_tokens,
                latency_ms,
                cost,
            } => {
                assert_eq!(input_tokens, 1024);
                assert_eq!(output_tokens, 512);
                assert_eq!(latency_ms, 1200);
                assert_eq!(cost, Some(0.003));
            }
            _ => panic!("expected Usage variant"),
        }
    }
}
