use anyhow::{Context, Result};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;

use crate::log_debug;
use crate::log_trace;
use crate::provider::base_client::{
    send_with_retry, AIClient, Chunk, ChunkContent, FinishReason, RetryConfig,
};
use crate::session::message::{ImageSource, Message, Part, Role};

// =============================================================================
// Anthropic API 客户端
// =============================================================================

/// Anthropic API 客户端。
pub struct AnthropicClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model_name: String,
    retry_config: RetryConfig,
}

impl AnthropicClient {
    /// 构造 Anthropic 客户端。
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
impl AIClient for AnthropicClient {
    async fn stream_message(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()> {
        // 构造请求头
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key)?);
        headers.insert("anthropic-version", HeaderValue::from_static("2025-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // 构造 Anthropic 兼容的请求消息
        // 由于 `Message` 已升级为 `Vec<Part>` 结构，需要手动映射到 Anthropic 的 content block 格式
        let anthropic_messages = build_messages(messages);

        // 构造请求体，显式开启流式模式
        let body = json!({
            "model": self.model_name,
            "system": system_prompt,
            "messages": anthropic_messages,
            "tools": *tools_schema,
            "max_tokens": 8000,
            "stream": true
        });

        let url = format!("{}/v1/messages", self.base_url);
        log_debug!(
            "Anthropic request | url={} | model={} | messages={} | tools_count={}",
            url,
            self.model_name,
            anthropic_messages.len(),
            body.get("tools")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        );
        log_trace!(
            "Anthropic request body | {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
        let request = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .build()?;
        let resp = send_with_retry(&self.client, request, &self.retry_config).await?;

        // 检查 HTTP 状态码
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Anthropic API error ({}): {}",
                status,
                text
            ));
        }

        // 直接读取 SSE 字节流并调用闭包
        let byte_stream = resp.bytes_stream();
        parse_anthropic_sse(byte_stream, on_chunk).await
    }
}

// =============================================================================
// 消息格式转换：内部 Message -> Anthropic 请求格式
// =============================================================================

/// 将内部 `Message` 列表转换为 Anthropic 兼容的消息数组。
///
/// 映射规则：
/// - `Role` -> 小写字符串（user/assistant/system/developer）
/// - `Part::Text` -> `{"type": "text", "text": ...}`
/// - `Part::Image` -> 根据 `ImageSource` 生成 base64 或 url 类型的 image block
/// - `Part::ToolUse` -> `{"type": "tool_use", "id", "name", "input"}`
/// - `Part::ToolResult` -> `{"type": "tool_result", "tool_use_id", "content", "is_error"}`
/// - `Part::Reasoning` -> 暂映射为 text block 以保留内容
fn build_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut result = Vec::new();
    for msg in messages {
        let role_str = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Developer => "developer",
        };

        let mut content = Vec::new();
        for part in &msg.parts {
            let value = match part {
                Part::Text { text } => {
                    json!({"type": "text", "text": text})
                }
                Part::Image { source } => match source {
                    ImageSource::Base64 { media_type, data } => {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": media_type,
                                "data": data
                            }
                        })
                    }
                    ImageSource::Path { path } => {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": format!("file://{}", path)
                            }
                        })
                    }
                    ImageSource::Url { url } => {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "url",
                                "url": url
                            }
                        })
                    }
                },
                Part::ToolUse {
                    id,
                    name,
                    arguments,
                } => {
                    json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": arguments
                    })
                }
                Part::ToolResult {
                    tool_call_id,
                    content: c,
                    is_error,
                } => {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": c,
                        "is_error": is_error
                    })
                }
                Part::Reasoning { thinking, .. } => {
                    // Anthropic extended thinking 可能使用不同的 block 类型；
                    // 当前为了保留内容，先映射为普通文本块
                    json!({"type": "text", "text": thinking})
                }
            };
            content.push(value);
        }

        if !content.is_empty() {
            result.push(json!({"role": role_str, "content": content}));
        }
    }
    result
}

// =============================================================================
// Anthropic SSE 解析：将原生 Server-Sent Events 通过闭包实时回传
// =============================================================================

/// 解析 Anthropic 的 SSE 字节流，并在解析过程中直接调用 `on_chunk`。
///
/// 关键事件映射：
/// - `content_block_delta` + `text_delta`       => `ChunkContent::Text`
/// - `content_block_delta` + `thinking_delta`   => `ChunkContent::Think`
/// - `content_block_start` + `tool_use`         => 记录工具调用元数据
/// - `content_block_delta` + `input_json_delta` => 累积工具参数 JSON
/// - `content_block_stop`                       => 拼装完整 ToolUse 并回传
/// - `message_delta` 中的 `stop_reason`         => `ChunkContent::Finish`
async fn parse_anthropic_sse<S>(
    byte_stream: S,
    on_chunk: &mut (dyn FnMut(Chunk) + Send),
) -> Result<()>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let mut buffer = String::new();
    // 维护 index -> (tool_id, tool_name, args_json_string)
    let mut index_to_tool: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();
    let mut current_event_type: Option<String> = None;

    tokio::pin!(byte_stream);
    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // 按行处理 SSE 数据
        while let Some(pos) = buffer.find('\n') {
            let line = buffer.drain(..=pos).collect::<String>();
            let line = line.trim_end();

            if line.starts_with("event:") {
                current_event_type = Some(line[6..].trim().to_string());
            } else if line.starts_with("data:") {
                let data = line[5..].trim();
                if data == "[DONE]" {
                    log_debug!("Anthropic SSE [DONE]");
                    continue;
                }

                let event_type = current_event_type.take().unwrap_or_default();
                log_trace!(
                    "Anthropic SSE raw | event={} | {}",
                    event_type,
                    data.chars().take(300).collect::<String>()
                );

                let json: serde_json::Value = serde_json::from_str(data)
                    .with_context(|| format!("Failed to parse Anthropic SSE data: {}", data))?;

                match event_type.as_str() {
                    "content_block_start" => {
                        if let Some(block) = json.get("content_block") {
                            let block_type =
                                block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if block_type == "tool_use" {
                                let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                    as usize;
                                let id = block
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                log_debug!(
                                    "Anthropic SSE tool_use_start | index={} | id={} | name={}",
                                    index,
                                    id,
                                    name
                                );
                                index_to_tool.insert(index, (id, name, String::new()));
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = json.get("delta") {
                            let delta_type =
                                delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match delta_type {
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        log_trace!(
                                            "Anthropic SSE text_delta | len={} | preview={}",
                                            text.len(),
                                            text.chars().take(80).collect::<String>()
                                        );
                                        on_chunk(Chunk {
                                            content: ChunkContent::Text(text.to_string()),
                                        });
                                    }
                                }
                                "thinking_delta" => {
                                    if let Some(text) =
                                        delta.get("thinking").and_then(|v| v.as_str())
                                    {
                                        log_trace!(
                                            "Anthropic SSE thinking_delta | len={} | preview={}",
                                            text.len(),
                                            text.chars().take(80).collect::<String>()
                                        );
                                        on_chunk(Chunk {
                                            content: ChunkContent::Think(text.to_string()),
                                        });
                                    }
                                }
                                "input_json_delta" => {
                                    let index =
                                        json.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                            as usize;
                                    if let Some((_, _, args)) = index_to_tool.get_mut(&index) {
                                        if let Some(partial) =
                                            delta.get("partial_json").and_then(|v| v.as_str())
                                        {
                                            log_trace!("Anthropic SSE input_json_delta | index={} | partial={}", index, partial);
                                            args.push_str(partial);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_stop" => {
                        let index =
                            json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        if let Some((id, name, args)) = index_to_tool.remove(&index) {
                            let arguments = serde_json::from_str(&args).unwrap_or(json!({}));
                            log_debug!(
                                "Anthropic assembled tool_call | id={} | name={} | args={}",
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
                    "message_delta" => {
                        if let Some(stop) = json
                            .get("delta")
                            .and_then(|d| d.get("stop_reason"))
                            .and_then(|v| v.as_str())
                        {
                            let reason = FinishReason::from_anthropic(stop);
                            log_debug!("Anthropic finish_reason={:?}", reason);
                            on_chunk(Chunk {
                                content: ChunkContent::Finish(reason),
                            });
                        }
                    }
                    _ => {}
                }
            } else if line.is_empty() {
                // SSE 空行表示一个事件结束，重置 event 类型
                current_event_type = None;
            }
        }
    }

    Ok(())
}
