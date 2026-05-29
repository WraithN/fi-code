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
// 本模块已从 fi-code-shared crate 重新导出所有类型，
// 保留此文件是为了维持现有代码的导入路径向后兼容。

pub use fi_code_shared::dto::{
    current_timestamp_ms, ImageSource, Message, MessageBuilder, Part, Role, TokenUsage,
};

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
            for_context_only: false,
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
                ..
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
            prompt_tokens: 1024,
            completion_tokens: 512,
            latency_ms: 1200,
            cost: Some(0.003),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"usage\""));
        assert!(json.contains("\"prompt_tokens\":1024"));
        assert!(json.contains("\"completion_tokens\":512"));
        assert!(json.contains("\"latency_ms\":1200"));
        assert!(json.contains("\"cost\":0.003"));

        let deserialized: Part = serde_json::from_str(&json).unwrap();
        match deserialized {
            Part::Usage {
                prompt_tokens,
                completion_tokens,
                latency_ms,
                cost,
            } => {
                assert_eq!(prompt_tokens, 1024);
                assert_eq!(completion_tokens, 512);
                assert_eq!(latency_ms, 1200);
                assert_eq!(cost, Some(0.003));
            }
            _ => panic!("expected Usage variant"),
        }
    }
}
