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
// TUI 主线流程 E2E 测试
// =============================================================================
// 使用 MockAIClient 模拟 LLM 响应，测试完整的端到端流程：
// 1. 用户输入 → HTTP API → Agent → SSE 流 → 前端事件
// 2. 工具调用（write、handle_task_plan）→ 文件系统变更 → SSE 事件
//
// 注意：这些测试启动真实的服务器，通过 HTTP API 交互，验证 SSE 事件流。

use std::net::TcpListener;
use std::sync::{Arc, RwLock};

use tokio_stream::StreamExt;
use serde_json::json;

/// 获取一个随机可用端口
fn get_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

/// 启动测试服务器（使用 Mock Provider）
async fn start_test_server(port: u16) -> tokio::task::JoinHandle<()> {
    let config = Arc::new(RwLock::new(fi_code_core::config::Config::default()));
    let provider = Arc::new(RwLock::new(fi_code_core::provider::Provider::new_mock()));

    let server = fi_code_core::server::Server::new(provider, config, Some(port));
    tokio::spawn(async move {
        server.run().await;
    })
}

/// SSE 事件收集器
#[derive(Debug, Clone)]
struct SseEvent {
    event_type: String,
    content: Option<String>,
    tool_name: Option<String>,
    plan_id: Option<String>,
    task_count: Option<usize>,
}

/// 发送对话消息并收集所有 SSE 事件
async fn chat_and_collect_events(port: u16, message: &str) -> Vec<SseEvent> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap();

    let url = format!("http://127.0.0.1:{}/chat", port);
    let req_body = json!({
        "session_id": null,
        "message": message
    });

    let response = client
        .post(&url)
        .json(&req_body)
        .send()
        .await
        .expect("Failed to send chat request");

    assert_eq!(response.status(), 200);

    let mut events = Vec::new();
    let mut buffer = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("SSE stream error");
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer.drain(..=pos).collect::<String>();
            let line = line.trim_end();

            if !line.starts_with("data: ") {
                continue;
            }

            let json_str = &line[6..];
            if let Ok(event) = serde_json::from_str::<fi_code_core::server::transport::sse::SseEvent>(json_str) {
                use fi_code_core::server::transport::sse::SseEvent as Ev;
                let sse_event = match &event {
                    Ev::Message { content } => SseEvent {
                        event_type: "Message".to_string(),
                        content: Some(content.clone()),
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::ToolUse { name, .. } => SseEvent {
                        event_type: "ToolUse".to_string(),
                        content: None,
                        tool_name: Some(name.clone()),
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::ToolResult { tool_use_id, .. } => SseEvent {
                        event_type: "ToolResult".to_string(),
                        content: Some(tool_use_id.clone()),
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::TaskProgress { plan_id, tasks } => SseEvent {
                        event_type: "TaskProgress".to_string(),
                        content: None,
                        tool_name: None,
                        plan_id: Some(plan_id.clone()),
                        task_count: Some(tasks.len()),
                    },
                    Ev::Done { .. } => SseEvent {
                        event_type: "Done".to_string(),
                        content: None,
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::Error { message } => SseEvent {
                        event_type: "Error".to_string(),
                        content: Some(message.clone()),
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::Usage { .. } => SseEvent {
                        event_type: "Usage".to_string(),
                        content: None,
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                    Ev::MessageDetails { blocks } => SseEvent {
                        event_type: "MessageDetails".to_string(),
                        content: Some(format!("blocks={}", blocks.len())),
                        tool_name: None,
                        plan_id: None,
                        task_count: None,
                    },
                };
                let is_done = matches!(event, Ev::Done { .. });
                events.push(sse_event);
                if is_done {
                    return events;
                }
            }
        }
    }

    events
}

/// 设置并获取测试工作目录
fn setup_test_workspace() -> std::path::PathBuf {
    let workspace = std::env::temp_dir().join("fi-code-tui-flow-test");
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).unwrap();
    fi_code_core::utils::workspace::set_workspace(workspace.clone());
    workspace
}

/// 清理测试输出目录
fn cleanup_test_output() {
    let workspace = std::env::temp_dir().join("fi-code-tui-flow-test");
    let _ = std::fs::remove_dir_all(&workspace);
}

mod e2e_tui_flow {
    use super::*;

    // =============================================================================
    // 场景 1：简单对话
    // =============================================================================

    #[tokio::test]
    async fn test_simple_greeting_flow() {
        let workspace = setup_test_workspace();
        let port = get_available_port();
        let server_handle = start_test_server(port).await;

        // 等待服务器启动
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let events = chat_and_collect_events(port, "你好，你是谁").await;

        // 验证收到了消息事件
        let message_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "Message")
            .collect();
        assert!(
            !message_events.is_empty(),
            "Should receive Message events"
        );

        // 验证消息内容包含问候语
        let all_text: String = message_events
            .iter()
            .filter_map(|e| e.content.clone())
            .collect();
        assert!(
            all_text.contains("FiCode") || all_text.contains("编程"),
            "Expected greeting text, got: {}",
            all_text
        );

        // 验证收到了 Done 事件
        assert!(
            events.iter().any(|e| e.event_type == "Done"),
            "Should receive Done event"
        );

        // 验证没有 Error 事件
        assert!(
            !events.iter().any(|e| e.event_type == "Error"),
            "Should not receive Error events"
        );

        server_handle.abort();
        cleanup_test_output();
    }

    // =============================================================================
    // 场景 2：代码书写任务
    // =============================================================================

    #[tokio::test]
    async fn test_code_writing_flow() {
        let workspace = setup_test_workspace();
        let port = get_available_port();
        let server_handle = start_test_server(port).await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let events = chat_and_collect_events(port, "帮我写一段代码，创建一个 hello.rs 文件").await;

        // 验证收到了 ToolUse 事件（write 工具）
        let tool_use_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "ToolUse")
            .collect();
        assert!(
            !tool_use_events.is_empty(),
            "Should receive ToolUse events"
        );

        // 验证工具名是 write
        assert!(
            tool_use_events.iter().any(|e| e.tool_name.as_deref() == Some("write")),
            "Should use write tool"
        );

        // 验证收到了 ToolResult 事件
        assert!(
            events.iter().any(|e| e.event_type == "ToolResult"),
            "Should receive ToolResult event"
        );

        // 验证文件实际已写入
        let file_path = workspace.join("test_output/hello.rs");
        assert!(
            file_path.exists(),
            "File should be written to {:?}",
            file_path
        );

        let content = std::fs::read_to_string(&file_path).expect("Failed to read file");
        assert!(
            content.contains("fn main()") && content.contains("Hello, World"),
            "File content should contain Rust hello world code, got: {}",
            content
        );

        // 验证收到了 Done 事件
        assert!(
            events.iter().any(|e| e.event_type == "Done"),
            "Should receive Done event"
        );

        server_handle.abort();
        cleanup_test_output();
    }

    // =============================================================================
    // 场景 3：复杂任务拆分
    // =============================================================================

    #[tokio::test]
    async fn test_task_splitting_flow() {
        let workspace = setup_test_workspace();
        let port = get_available_port();
        let server_handle = start_test_server(port).await;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let events = chat_and_collect_events(port, "我有一个复杂任务，请帮我拆分任务并执行").await;

        // 验证收到了 ToolUse 事件（handle_task_plan 工具）
        let tool_use_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "ToolUse")
            .collect();
        assert!(
            !tool_use_events.is_empty(),
            "Should receive ToolUse events"
        );

        // 验证工具名是 handle_task_plan
        assert!(
            tool_use_events.iter().any(|e| e.tool_name.as_deref() == Some("handle_task_plan")),
            "Should use handle_task_plan tool"
        );

        // 验证收到了 ToolResult 事件
        assert!(
            events.iter().any(|e| e.event_type == "ToolResult"),
            "Should receive ToolResult event"
        );

        // 验证收到了 TaskProgress 事件（任务状态更新）
        // 注意：TaskProgress 是通过独立线程发送的，可能有延迟
        let progress_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "TaskProgress")
            .collect();

        // TaskProgress 事件数量可能为 0 或多个，取决于执行时序
        // 如果有，验证任务数量
        for ev in &progress_events {
            if let Some(count) = ev.task_count {
                assert_eq!(count, 3, "Task plan should have 3 tasks");
            }
        }

        // 验证收到了 Done 事件
        assert!(
            events.iter().any(|e| e.event_type == "Done"),
            "Should receive Done event"
        );

        server_handle.abort();
        cleanup_test_output();
    }
}
