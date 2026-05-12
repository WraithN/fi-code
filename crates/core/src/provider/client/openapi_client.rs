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

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::json;

use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_trace;
use crate::provider::base_client::{
    send_with_retry, AIClient, Chunk, ChunkContent, FinishReason, RetryConfig,
};
use crate::session::message::{Message, Part, Role};

// =============================================================================
// OpenAI API 兼容客户端
// =============================================================================

use std::collections::HashMap;

pub struct OpenAiClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model_name: String,
    headers: Option<HashMap<String, String>>,
    retry_config: RetryConfig,
}

impl OpenAiClient {
    /// 构造 OpenAI 兼容客户端。
    pub(crate) fn new(
        client: reqwest::Client,
        api_key: String,
        base_url: String,
        model_name: String,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        Ok(Self {
            client,
            api_key,
            base_url,
            model_name,
            headers,
            retry_config: RetryConfig::default(),
        })
    }
}

#[async_trait::async_trait]
impl AIClient for OpenAiClient {
    async fn stream_message(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // 追加用户配置的自定义请求头
        if let Some(ref custom_headers) = self.headers {
            for (key, value) in custom_headers {
                if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                    if let Ok(header_value) = HeaderValue::from_str(value) {
                        headers.insert(header_name, header_value);
                    }
                }
            }
        }

        // 将内部 Message/Part 模型转换为 OpenAI 兼容的消息格式
        let openai_messages = build_messages(system_prompt, messages);

        // 显式开启流式模式
        let mut body = json!({
            "model": self.model_name,
            "messages": openai_messages,
            "max_tokens": 8000,
            "stream": true,
            "stream_options": { "include_usage": true }
        });
        // 如果 tools_schema 非空则附加（部分平台不支持 tools，会导致 404）
        let tools = convert_tools_schema(tools_schema);
        if let Some(arr) = tools.as_array() {
            if !arr.is_empty() {
                body["tools"] = tools;
            }
        }

        // 兼容两种 base_url 写法：
        // - 已包含版本号（如 https://api.openai.com/v1） → 直接附加 /chat/completions
        // - 未包含版本号（如 http://localhost:11434） → 附加 /v1/chat/completions
        let url = if self.base_url.ends_with("/v1")
            || self.base_url.ends_with("/v2")
            || self.base_url.ends_with("/v3")
            || self.base_url.ends_with("/v4")
        {
            format!("{}/chat/completions", self.base_url)
        } else {
            format!("{}/v1/chat/completions", self.base_url)
        };
        log_info!("[Server] OpenAI request | url={} | model={} | messages={} | tools_count={}",
            url,
            self.model_name,
            openai_messages.len(),
            body.get("tools")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        );
        let body_str = serde_json::to_string_pretty(&body).unwrap_or_default();
        let truncated_body = if body_str.len() > 2000 {
            format!("{}... [{} bytes total]", &body_str[..2000], body_str.len())
        } else {
            body_str
        };
        log_debug!("[Server] OpenAI request body | {}", truncated_body);
        let request = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .build()?;
        let resp = send_with_retry(&self.client, request, &self.retry_config).await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            log_error!("[Server] OpenAI API error | url={} | status={} | response={}",
                url,
                status,
                text
            );
            return Err(anyhow::anyhow!("OpenAI API error ({}): {}", status, text));
        }

        let byte_stream = resp.bytes_stream();
        parse_openai_sse(byte_stream, on_chunk).await
    }
}

// =============================================================================
// OpenAI SSE 解析：将原生 Server-Sent Events 通过闭包实时回传
// =============================================================================

/// 解析 OpenAI 的 SSE 字节流，并在解析过程中直接调用 `on_chunk`。
///
/// 解析逻辑：
/// - `delta.content` 存在 => 直接回传 `ChunkContent::Text`
/// - `delta.tool_calls` 存在 => 在内存中累积每个 index 的 (id, name, arguments)
/// - `finish_reason` 存在 => 若因 tool_calls 结束，先将所有拼好的 tool_use 回传，
///   最后统一回传 `ChunkContent::Finish`
fn update_openai_tool_call_delta(
    tool: &serde_json::Value,
    index_to_tool: &mut std::collections::HashMap<usize, (Option<String>, Option<String>, String)>,
) {
    let index = tool.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let id = tool
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let name = tool
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let args = tool
        .get("function")
        .and_then(|f| f.get("arguments"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    log_trace!(
        "OpenAI SSE tool_call_delta | index={} | id={:?} | name={:?} | args={}",
        index,
        id,
        name,
        args
    );

    let entry = index_to_tool
        .entry(index)
        .or_insert((None, None, String::new()));
    if let Some(id) = id {
        entry.0 = Some(id);
    }
    if let Some(name) = name {
        entry.1 = Some(name);
    }
    entry.2.push_str(&args);
}

fn flush_openai_tool_calls(
    index_to_tool: &mut std::collections::HashMap<usize, (Option<String>, Option<String>, String)>,
    on_chunk: &mut dyn FnMut(Chunk),
) {
    let mut indices: Vec<usize> = index_to_tool.keys().cloned().collect();
    indices.sort();
    for idx in indices {
        let Some((Some(id), Some(name), args)) = index_to_tool.remove(&idx) else {
            continue;
        };
        let arguments = serde_json::from_str(&args).unwrap_or(json!({}));
        log_debug!(
            "[Server] OpenAI assembled tool_call | id={} | name={} | args={}",
            id,
            name,
            arguments
        );
        on_chunk(Chunk {
            content: ChunkContent::ToolUse(Part::ToolUse {
                id,
                name,
                arguments,
            }),
        });
    }
}

fn handle_openai_delta(
    delta: &serde_json::Value,
    index_to_tool: &mut std::collections::HashMap<usize, (Option<String>, Option<String>, String)>,
    on_chunk: &mut dyn FnMut(Chunk),
) {
    if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            log_trace!(
                "OpenAI SSE text_delta | len={} | preview={}",
                text.len(),
                text.chars().take(80).collect::<String>()
            );
            on_chunk(Chunk {
                content: ChunkContent::Text(text.to_string()),
            });
        }
    }
    if let Some(tools) = delta.get("tool_calls").and_then(|v| v.as_array()) {
        for tool in tools {
            update_openai_tool_call_delta(tool, index_to_tool);
        }
    }
}

fn handle_openai_finish(
    finish: &str,
    index_to_tool: &mut std::collections::HashMap<usize, (Option<String>, Option<String>, String)>,
    on_chunk: &mut dyn FnMut(Chunk),
) {
    if finish == "tool_calls" {
        flush_openai_tool_calls(index_to_tool, on_chunk);
    }
    let reason = FinishReason::from_openai(finish);
    log_debug!("[Server] OpenAI finish_reason={:?}", reason);
    on_chunk(Chunk {
        content: ChunkContent::Finish(reason),
    });
}

fn process_openai_choice(
    choice: &serde_json::Value,
    index_to_tool: &mut std::collections::HashMap<usize, (Option<String>, Option<String>, String)>,
    on_chunk: &mut dyn FnMut(Chunk),
) {
    let finish_reason = choice.get("finish_reason").and_then(|v| v.as_str());
    if let Some(delta) = choice.get("delta") {
        handle_openai_delta(delta, index_to_tool, on_chunk);
    }
    if let Some(finish) = finish_reason {
        handle_openai_finish(finish, index_to_tool, on_chunk);
    }
}

async fn parse_openai_sse<S>(byte_stream: S, on_chunk: &mut (dyn FnMut(Chunk) + Send)) -> Result<()>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let mut buffer = String::new();
    // OpenAI 的 tool_calls 增量只带 `index`，需要维护 index -> (id, name, args_buffer)
    let mut index_to_tool: std::collections::HashMap<
        usize,
        (Option<String>, Option<String>, String),
    > = std::collections::HashMap::new();

    tokio::pin!(byte_stream);
    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer.drain(..=pos).collect::<String>();
            let line = line.trim_end();

            if !line.starts_with("data:") {
                continue;
            }

            let data = line[5..].trim();
            if data == "[DONE]" {
                log_debug!("[Server] OpenAI SSE [DONE]");
                continue;
            }

            log_trace!(
                "[Server] OpenAI SSE raw | {}",
                data.chars().take(300).collect::<String>()
            );

            let json: serde_json::Value = serde_json::from_str(data)
                .with_context(|| format!("Failed to parse OpenAI SSE data: {}", data))?;

            // 解析 usage（通常出现在最后一个 chunk，此时 choices 可能为空）
            if let Some(usage) = json.get("usage") {
                let prompt_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let completion_tokens = usage
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                if prompt_tokens > 0 || completion_tokens > 0 {
                    log_debug!(
                        "[Server] OpenAI SSE usage | prompt={} | completion={}",
                        prompt_tokens,
                        completion_tokens
                    );
                    on_chunk(Chunk {
                        content: ChunkContent::Usage(crate::provider::base_client::TokenUsage {
                            prompt_tokens,
                            completion_tokens,
                        }),
                    });
                }
            }

            let Some(choices) = json.get("choices").and_then(|v| v.as_array()) else {
                continue;
            };
            for choice in choices {
                process_openai_choice(choice, &mut index_to_tool, on_chunk);
            }
        }
    }

    Ok(())
}

// =============================================================================
// 请求/响应结构体（仅用于序列化请求体）
// =============================================================================

/// OpenAI 消息格式，用于序列化请求体。
#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// 辅助函数：转换消息格式
// =============================================================================

/// 将内部 `Message` 列表转换为 OpenAI 兼容的消息格式。
///
/// 映射规则：
/// - `Role::User`：将其中的 `Part::Text` 合并为 user 消息；
///   `Part::ToolResult` 映射为独立的 `role: "tool"` 消息
/// - `Role::Assistant`：将其中的 `Part::Text` 和 `Part::Reasoning` 合并为 content；
///   `Part::ToolUse` 映射为 `tool_calls` 数组
/// - `Role::System`：在循环前单独插入 system 消息
fn build_user_messages(msg: &Message) -> Vec<OpenAiMessage> {
    let mut text_parts = Vec::new();
    let mut tool_results = Vec::new();

    for part in &msg.parts {
        match part {
            Part::Text { text } => text_parts.push(text.clone()),
            Part::ToolResult {
                tool_call_id,
                content: c,
                ..
            } => {
                tool_results.push(OpenAiMessage {
                    role: "tool".to_string(),
                    content: Some(c.clone()),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id.clone()),
                });
            }
            Part::Image { .. } => text_parts.push("[image]".to_string()),
            _ => {}
        }
    }

    let mut result = Vec::new();
    if !text_parts.is_empty() {
        result.push(OpenAiMessage {
            role: "user".to_string(),
            content: Some(text_parts.join("\n")),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    result.extend(tool_results);
    result
}

/// 压缩工具参数，避免 Turn 2 的 Prompt 因重复传大段代码而膨胀。
fn compact_tool_arguments(name: &str, arguments: &serde_json::Value) -> serde_json::Value {
    let mut compact = arguments.clone();
    match name {
        "write" | "edit" => {
            if let Some(obj) = compact.as_object_mut() {
                if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                    obj.insert(
                        "content".to_string(),
                        json!(format!("[{} bytes omitted]", content.len())),
                    );
                }
            }
        }
        "bash" => {
            if let Some(obj) = compact.as_object_mut() {
                if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
                    let truncated = if cmd.len() > 100 {
                        format!("{}... [{} chars total]", &cmd[..100], cmd.len())
                    } else {
                        cmd.to_string()
                    };
                    obj.insert("command".to_string(), json!(truncated));
                }
            }
        }
        _ => {}
    }
    compact
}

fn build_assistant_message(msg: &Message) -> OpenAiMessage {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for part in &msg.parts {
        match part {
            Part::Text { text } => text_parts.push(text.clone()),
            Part::ToolUse {
                id,
                name,
                arguments,
            } => {
                // 上下文压缩：截断大参数，避免 Turn 2 Prompt 膨胀
                let compact = compact_tool_arguments(name, arguments);
                tool_calls.push(OpenAiToolCall {
                    id: id.clone(),
                    call_type: "function".to_string(),
                    function: OpenAiFunctionCall {
                        name: name.clone(),
                        arguments: compact.to_string(),
                    },
                });
            }
            Part::Reasoning { thinking, .. } => text_parts.push(thinking.clone()),
            _ => {}
        }
    }

    let content_text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    };

    OpenAiMessage {
        role: "assistant".to_string(),
        content: content_text,
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        tool_call_id: None,
    }
}

fn build_messages(system_prompt: &str, messages: &[Message]) -> Vec<OpenAiMessage> {
    let mut result = Vec::new();

    result.push(OpenAiMessage {
        role: "system".to_string(),
        content: Some(system_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    });

    for msg in messages {
        match msg.role {
            Role::User => result.extend(build_user_messages(msg)),
            Role::Assistant => result.push(build_assistant_message(msg)),
            _ => {}
        }
    }

    result
}

// =============================================================================
// 辅助函数：转换工具 schema 和响应
// =============================================================================

/// 将内部工具注册表生成的 schema 转换为 OpenAI 要求的 `tools` 格式。
fn convert_tools_schema(tools_schema: &serde_json::Value) -> serde_json::Value {
    if let Some(arr) = tools_schema.as_array() {
        let converted: Vec<serde_json::Value> = arr
            .iter()
            .map(|tool| {
                let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let description = tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let parameters = tool.get("input_schema").cloned().unwrap_or(json!({}));
                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters
                    }
                })
            })
            .collect();
        serde_json::Value::Array(converted)
    } else {
        json!([])
    }
}
