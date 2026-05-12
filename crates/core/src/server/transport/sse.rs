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

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// 消息详情块，用于展示模型的思考过程和工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetailBlock {
    Text {
        text: String,
    },
    Reasoning {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// 任务进度项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressItem {
    pub id: String,
    pub name: String,
    pub status: crate::tools::task::TaskStatus,
}

/// SSE 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "message")]
    Message { content: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        arguments: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        diff: Option<String>,
        is_new_file: bool,
    },
    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
    #[serde(rename = "details")]
    MessageDetails { blocks: Vec<DetailBlock> },
    #[serde(rename = "usage")]
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "done")]
    Done { session_id: String },
}

/// SSE 发送端，供 agent_loop 写入事件
#[derive(Clone)]
pub struct SseSender {
    tx: mpsc::Sender<SseEvent>,
}

impl SseSender {
    pub fn new(tx: mpsc::Sender<SseEvent>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, event: SseEvent) -> Result<(), String> {
        self.tx.send(event).await.map_err(|e| e.to_string())
    }

    /// 同步尝试发送事件（不阻塞，channel 满时返回错误）。
    pub fn try_send(&self, event: SseEvent) -> Result<(), String> {
        self.tx.try_send(event).map_err(|e| e.to_string())
    }
}

/// 创建 SSE 流对 (sender, stream)
pub fn create_sse_channel(buffer: usize) -> (SseSender, ReceiverStream<SseEvent>) {
    let (tx, rx) = mpsc::channel::<SseEvent>(buffer);
    (SseSender::new(tx), ReceiverStream::new(rx))
}

/// 将 SseEvent 序列化为 SSE data 行
pub fn format_sse_event(event: SseEvent) -> String {
    let data = serde_json::to_string(&event).unwrap_or_default();
    format!("data: {}\n\n", data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_message_serde() {
        let event = SseEvent::Message {
            content: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("\"content\":\"hello\""));

        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::Message { content } => assert_eq!(content, "hello"),
            _ => panic!("expected Message variant"),
        }
    }

    #[test]
    fn test_sse_event_tool_use_serde() {
        let event = SseEvent::ToolUse {
            id: "tool-1".to_string(),
            name: "write".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::ToolUse { id, name, arguments } => {
                assert_eq!(id, "tool-1");
                assert_eq!(name, "write");
                assert_eq!(arguments["path"], "/tmp/test.txt");
            }
            _ => panic!("expected ToolUse variant"),
        }
    }

    #[test]
    fn test_sse_event_error_serde() {
        let event = SseEvent::Error {
            message: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::Error { message } => assert_eq!(message, "something went wrong"),
            _ => panic!("expected Error variant"),
        }
    }

    #[test]
    fn test_sse_event_done_serde() {
        let event = SseEvent::Done {
            session_id: "sess-123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::Done { session_id } => assert_eq!(session_id, "sess-123"),
            _ => panic!("expected Done variant"),
        }
    }

    #[test]
    fn test_sse_event_usage_serde() {
        let event = SseEvent::Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SseEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SseEvent::Usage {
                prompt_tokens,
                completion_tokens,
            } => {
                assert_eq!(prompt_tokens, 100);
                assert_eq!(completion_tokens, 50);
            }
            _ => panic!("expected Usage variant"),
        }
    }

    #[test]
    fn test_format_sse_event() {
        let event = SseEvent::Message {
            content: "hi".to_string(),
        };
        let formatted = format_sse_event(event);
        assert!(formatted.starts_with("data: "));
        assert!(formatted.ends_with("\n\n"));
        assert!(formatted.contains("\"type\":\"message\""));
    }

    #[tokio::test]
    async fn test_sse_sender_send() {
        use futures::StreamExt;
        let (sender, mut stream) = create_sse_channel(10);
        let event = SseEvent::Message {
            content: "test".to_string(),
        };
        sender.send(event.clone()).await.unwrap();

        let received: Option<SseEvent> = stream.next().await;
        match received.unwrap() {
            SseEvent::Message { content } => assert_eq!(content, "test"),
            _ => panic!("expected Message"),
        }
    }

    #[tokio::test]
    async fn test_sse_sender_try_send() {
        use futures::StreamExt;
        let (sender, mut stream) = create_sse_channel(1);
        let event1 = SseEvent::Message {
            content: "first".to_string(),
        };
        let event2 = SseEvent::Message {
            content: "second".to_string(),
        };

        // 第一次 try_send 应该成功
        assert!(sender.try_send(event1).is_ok());

        // 第二次 try_send 可能成功（channel 容量为 1，但尚未被消费）
        // 或者失败（如果 channel 满了）
        // 这里不做强断言，只测试不 panic
        let _ = sender.try_send(event2);

        // 消费至少一个事件
        let received: Option<SseEvent> = stream.next().await;
        assert!(received.is_some());
    }

    #[test]
    fn test_detail_block_serde() {
        let block = DetailBlock::ToolUse {
            id: "t1".to_string(),
            name: "read".to_string(),
            arguments: "{}".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let decoded: DetailBlock = serde_json::from_str(&json).unwrap();
        match decoded {
            DetailBlock::ToolUse { id, name, arguments } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "read");
                assert_eq!(arguments, "{}");
            }
            _ => panic!("expected ToolUse"),
        }
    }
}
