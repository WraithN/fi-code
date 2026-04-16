use anyhow::{Context, Result};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::json;

use crate::log_debug;
use crate::log_trace;
use crate::provider::base_client::{
    send_with_retry, AIClient, Chunk, ChunkContent, FinishReason, RetryConfig,
};
use crate::session::message::{Message, Part, Role};

// =============================================================================
// OpenAI API 兼容客户端
// =============================================================================

pub struct OpenAiClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model_name: String,
    retry_config: RetryConfig,
}

impl OpenAiClient {
    /// 构造 OpenAI 兼容客户端。
    pub(crate) fn new(api_key: String, base_url: String, model_name: String) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
            model_name,
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

        // 将内部 Message/Part 模型转换为 OpenAI 兼容的消息格式
        let openai_messages = build_messages(system_prompt, messages);

        // 显式开启流式模式
        let body = json!({
            "model": self.model_name,
            "messages": openai_messages,
            "tools": convert_tools_schema(tools_schema),
            "max_tokens": 8000,
            "stream": true
        });

        let url = format!("{}/v1/chat/completions", self.base_url);
        log_debug!(
            "OpenAI request | url={} | model={} | messages={} | tools_count={}",
            url,
            self.model_name,
            openai_messages.len(),
            body.get("tools")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        );
        log_trace!(
            "OpenAI request body | {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
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

            if line.starts_with("data:") {
                let data = line[5..].trim();
                if data == "[DONE]" {
                    log_debug!("OpenAI SSE [DONE]");
                    continue;
                }

                log_trace!(
                    "OpenAI SSE raw | {}",
                    data.chars().take(300).collect::<String>()
                );

                let json: serde_json::Value = serde_json::from_str(data)
                    .with_context(|| format!("Failed to parse OpenAI SSE data: {}", data))?;

                // OpenAI 的 choices 数组通常只有一个元素
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    for choice in choices {
                        let finish_reason = choice.get("finish_reason").and_then(|v| v.as_str());

                        if let Some(delta) = choice.get("delta") {
                            // 文本增量：直接回传
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

                            // 工具调用增量：仅更新内存状态，暂不回传
                            if let Some(tools) = delta.get("tool_calls").and_then(|v| v.as_array())
                            {
                                for tool in tools {
                                    let index =
                                        tool.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                            as usize;
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
                                        index, id, name, args
                                    );

                                    let entry = index_to_tool.entry(index).or_insert((
                                        None,
                                        None,
                                        String::new(),
                                    ));
                                    if let Some(id) = id {
                                        entry.0 = Some(id);
                                    }
                                    if let Some(name) = name {
                                        entry.1 = Some(name);
                                    }
                                    entry.2.push_str(&args);
                                }
                            }
                        }

                        // 当收到 finish_reason 时，说明所有增量已结束
                        if let Some(finish) = finish_reason {
                            // 若因工具调用结束，先将拼好的完整 tool_use 回传
                            if finish == "tool_calls" {
                                let mut indices: Vec<usize> =
                                    index_to_tool.keys().cloned().collect();
                                indices.sort();
                                for idx in indices {
                                    if let Some((Some(id), Some(name), args)) =
                                        index_to_tool.remove(&idx)
                                    {
                                        let arguments =
                                            serde_json::from_str(&args).unwrap_or(json!({}));
                                        log_debug!(
                                            "OpenAI assembled tool_call | id={} | name={} | args={}",
                                            id, name, arguments
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
                            }

                            let reason = FinishReason::from_openai(finish);
                            log_debug!("OpenAI finish_reason={:?}", reason);
                            on_chunk(Chunk {
                                content: ChunkContent::Finish(reason),
                            });
                        }
                    }
                }
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
            Role::User => {
                let mut text_parts = Vec::new();
                let mut tool_results = Vec::new();

                for part in &msg.parts {
                    match part {
                        Part::Text { text } => {
                            text_parts.push(text.clone());
                        }
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
                        Part::Image { .. } => {
                            // OpenAI vision 在当前简化路径中以占位符表示
                            text_parts.push("[image]".to_string());
                        }
                        _ => {}
                    }
                }

                if !text_parts.is_empty() {
                    result.push(OpenAiMessage {
                        role: "user".to_string(),
                        content: Some(text_parts.join("\n")),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }

                for tr in tool_results {
                    result.push(tr);
                }
            }
            Role::Assistant => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();

                for part in &msg.parts {
                    match part {
                        Part::Text { text } => {
                            text_parts.push(text.clone());
                        }
                        Part::ToolUse {
                            id,
                            name,
                            arguments,
                        } => {
                            tool_calls.push(OpenAiToolCall {
                                id: id.clone(),
                                call_type: "function".to_string(),
                                function: OpenAiFunctionCall {
                                    name: name.clone(),
                                    arguments: arguments.to_string(),
                                },
                            });
                        }
                        Part::Reasoning { thinking, .. } => {
                            text_parts.push(thinking.clone());
                        }
                        _ => {}
                    }
                }

                let content_text = if text_parts.is_empty() {
                    None
                } else {
                    Some(text_parts.join("\n"))
                };

                result.push(OpenAiMessage {
                    role: "assistant".to_string(),
                    content: content_text,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                });
            }
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
