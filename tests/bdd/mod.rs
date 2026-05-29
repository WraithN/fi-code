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

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cucumber::World;
use serde_json::json;
use tokio_stream::StreamExt;

pub mod steps;

// =============================================================================
// Cucumber World：共享测试状态
// =============================================================================

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct AgentWorld {
    pub port: u16,
    pub server_handle: Option<tokio::task::JoinHandle<()>>,
    pub workspace: Option<PathBuf>,
    pub events: Vec<SseEvent>,
    pub last_response: String,
    pub current_file: Option<String>,
    pub current_file_content: Option<String>,
    pub registered_skills: Vec<String>,
    pub tui_log_visible: bool,
    pub is_connected: bool,
}

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<serde_json::Value>,
    pub plan_id: Option<String>,
    pub task_count: Option<usize>,
}

impl AgentWorld {
    pub async fn new() -> Self {
        Self {
            port: 0,
            server_handle: None,
            workspace: None,
            events: Vec::new(),
            last_response: String::new(),
            current_file: None,
            current_file_content: None,
            registered_skills: Vec::new(),
            tui_log_visible: false,
            is_connected: true,
        }
    }

    /// 获取一个随机可用端口
    pub fn get_available_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().unwrap().port()
    }

    /// 启动 Mock Provider 测试服务器
    pub async fn start_mock_server(&mut self) {
        let port = Self::get_available_port();
        self.port = port;

        // 设置测试工作目录
        let workspace = std::env::temp_dir().join(format!("fi-code-bdd-{}", port));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).unwrap();
        fi_code_core::utils::workspace::set_workspace(workspace.clone());
        self.workspace = Some(workspace);

        let config = Arc::new(RwLock::new(fi_code_core::config::Config::default()));
        let provider = Arc::new(RwLock::new(fi_code_core::provider::Provider::new_mock()));

        let server = fi_code_core::server::Server::new(provider, config, Some(port));
        let handle = tokio::spawn(async move {
            server.run().await;
        });

        self.server_handle = Some(handle);
        self.is_connected = true;

        // 等待服务器启动
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    /// 通过 HTTP API 发送消息并收集 SSE 事件
    pub async fn send_chat_message(&mut self, message: &str) {
        self.send_chat_message_with_agent(message, None).await;
    }

    /// 通过 HTTP API 发送消息并收集 SSE 事件，支持指定 Agent 类型
    pub async fn send_chat_message_with_agent(
        &mut self,
        message: &str,
        agent: Option<fi_code_core::agent::AgentType>,
    ) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap();

        let url = format!("http://127.0.0.1:{}/chat", self.port);
        let req_body = json!({
            "session_id": null,
            "message": message,
            "agent": agent
        });

        let response = client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .expect("Failed to send chat request");

        assert_eq!(response.status(), 200);

        let mut buffer = String::new();
        let mut stream = response.bytes_stream();
        self.events.clear();

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
                if let Ok(event) =
                    serde_json::from_str::<fi_code_core::server::transport::sse::SseEvent>(json_str)
                {
                    use fi_code_core::server::transport::sse::SseEvent as Ev;
                    let sse_event = match &event {
                        Ev::Message { content } => SseEvent {
                            event_type: "Message".to_string(),
                            content: Some(content.clone()),
                            tool_name: None,
                            tool_args: None,
                            plan_id: None,
                            task_count: None,
                        },
                        Ev::Part { part } => match part {
                            fi_code_core::session::message::Part::ToolUse {
                                name,
                                arguments,
                                ..
                            } => SseEvent {
                                event_type: "ToolUse".to_string(),
                                content: None,
                                tool_name: Some(name.clone()),
                                tool_args: Some(arguments.clone()),
                                plan_id: None,
                                task_count: None,
                            },
                            fi_code_core::session::message::Part::ToolResult {
                                content, ..
                            } => SseEvent {
                                event_type: "ToolResult".to_string(),
                                content: Some(content.clone()),
                                tool_name: None,
                                tool_args: None,
                                plan_id: None,
                                task_count: None,
                            },
                            fi_code_core::session::message::Part::ToolError { content, .. } => {
                                SseEvent {
                                    event_type: "ToolError".to_string(),
                                    content: Some(content.clone()),
                                    tool_name: None,
                                    tool_args: None,
                                    plan_id: None,
                                    task_count: None,
                                }
                            }
                            _ => SseEvent {
                                event_type: "Other".to_string(),
                                content: None,
                                tool_name: None,
                                tool_args: None,
                                plan_id: None,
                                task_count: None,
                            },
                        },
                        Ev::TaskProgress { plan_id, tasks } => SseEvent {
                            event_type: "TaskProgress".to_string(),
                            content: None,
                            tool_name: None,
                            tool_args: None,
                            plan_id: Some(plan_id.clone()),
                            task_count: Some(tasks.len()),
                        },
                        Ev::Done { .. } => SseEvent {
                            event_type: "Done".to_string(),
                            content: None,
                            tool_name: None,
                            tool_args: None,
                            plan_id: None,
                            task_count: None,
                        },
                        Ev::Error { message } => SseEvent {
                            event_type: "Error".to_string(),
                            content: Some(message.clone()),
                            tool_name: None,
                            tool_args: None,
                            plan_id: None,
                            task_count: None,
                        },
                        Ev::AgentInfo { .. } => SseEvent {
                            event_type: "AgentInfo".to_string(),
                            content: None,
                            tool_name: None,
                            tool_args: None,
                            plan_id: None,
                            task_count: None,
                        },
                        Ev::CompressionStatus { .. } => SseEvent {
                            event_type: "CompressionStatus".to_string(),
                            content: None,
                            tool_name: None,
                            tool_args: None,
                            plan_id: None,
                            task_count: None,
                        },
                        Ev::PermissionAsk { tool_call_id, .. } => {
                            // BDD 测试中自动确认权限请求
                            let client = reqwest::Client::new();
                            let _ = client
                                .post(format!(
                                    "http://127.0.0.1:{}/api/permission/respond",
                                    self.port
                                ))
                                .json(&json!({
                                    "tool_call_id": tool_call_id,
                                    "approved": true
                                }))
                                .send()
                                .await;
                            SseEvent {
                                event_type: "PermissionAsk".to_string(),
                                content: None,
                                tool_name: None,
                                tool_args: None,
                                plan_id: None,
                                task_count: None,
                            }
                        }
                        Ev::QuestionAsk {
                            tool_call_id,
                            options,
                            recommended,
                            ..
                        } => {
                            // BDD 测试中自动回答 question_ask：选择推荐选项或第一个选项
                            let answer = if let Some(rec_id) = recommended {
                                options.iter().find(|o| o.id == *rec_id).map(|o| json!({"type": "option", "id": o.id.clone(), "label": o.label.clone()}))
                            } else {
                                options.first().map(|o| json!({"type": "option", "id": o.id.clone(), "label": o.label.clone()}))
                            };
                            if let Some(ans) = answer {
                                let client = reqwest::Client::new();
                                let _ = client
                                    .post(format!(
                                        "http://127.0.0.1:{}/api/question/respond",
                                        self.port
                                    ))
                                    .json(&json!({
                                        "tool_call_id": tool_call_id,
                                        "answer": ans
                                    }))
                                    .send()
                                    .await;
                            }
                            SseEvent {
                                event_type: "QuestionAsk".to_string(),
                                content: None,
                                tool_name: None,
                                tool_args: None,
                                plan_id: None,
                                task_count: None,
                            }
                        }
                    };
                    let is_done = matches!(event, Ev::Done { .. });
                    self.events.push(sse_event);
                    if is_done {
                        return;
                    }
                }
            }
        }
    }

    /// 获取所有消息文本的拼接
    pub fn all_message_text(&self) -> String {
        self.events
            .iter()
            .filter(|e| e.event_type == "Message")
            .filter_map(|e| e.content.clone())
            .collect::<Vec<_>>()
            .join("")
    }

    /// 获取所有 ToolUse 事件
    pub fn tool_use_events(&self) -> Vec<&SseEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == "ToolUse")
            .collect()
    }

    /// 获取所有 ToolError 事件
    pub fn tool_error_events(&self) -> Vec<&SseEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == "ToolError")
            .collect()
    }

    /// 获取所有 ToolResult/ToolError 事件的内容拼接
    pub fn all_tool_result_text(&self) -> String {
        self.events
            .iter()
            .filter(|e| e.event_type == "ToolResult" || e.event_type == "ToolError")
            .filter_map(|e| e.content.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 清理资源
    pub fn cleanup(&mut self) {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        if let Some(ref workspace) = self.workspace {
            let _ = std::fs::remove_dir_all(workspace);
        }
    }
}
