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
// runner 模块：AgentRunner 抽象
// =============================================================================
// 本模块将 Agent 的运行逻辑封装为一个可复用的结构体，
// 便于上层（如 TaskManager）按需驱动多轮对话，而无需直接依赖全局状态。

use anyhow::Result;

use crate::log_debug;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason, TokenUsage};
use crate::provider::execute_tool_calls;
use crate::provider::Chunk;
use crate::session::message::{Message, Part, Role};

// =============================================================================
// Agent 运行结果
// =============================================================================

/// 保存一次 Agent 运行结束后的完整状态。
#[derive(Debug)]
pub struct AgentRunResult {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub finish_reason: Option<FinishReason>,
    pub token_usage: TokenUsage,
}

// =============================================================================
// AgentRunner：封装客户端与运行配置
// =============================================================================

/// Agent 运行器，持有对话所需的全部依赖。
pub struct AgentRunner {
    client: Box<dyn AIClient>,
    system_prompt: String,
    tools_schema: serde_json::Value,
    max_turns: usize,
}

impl AgentRunner {
    /// 构造一个 `AgentRunner`。
    pub fn new(
        client: Box<dyn AIClient>,
        system_prompt: impl Into<String>,
        tools_schema: serde_json::Value,
    ) -> Self {
        Self {
            client,
            system_prompt: system_prompt.into(),
            tools_schema,
            max_turns: 25,
        }
    }

    /// 设置最大对话轮数。
    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = max;
        self
    }

    /// 运行 Agent 循环，直到对话自然结束或达到最大轮数。
    pub async fn run(&self, initial_messages: Vec<Message>) -> Result<AgentRunResult> {
        self.run_with_sink(initial_messages, &mut None).await
    }

    /// 运行 Agent 循环，支持实时文本回调。
    ///
    /// `on_text` 收到每个 Text/Think chunk 时立即调用，用于真流式渲染。
    pub async fn run_with_sink(
        &self,
        initial_messages: Vec<Message>,
        on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    ) -> Result<AgentRunResult> {
        let mut messages = initial_messages;
        let mut turn_count = 0usize;
        let mut last_finish_reason = None;
        let mut token_usage = TokenUsage::default();

        while turn_count < self.max_turns {
            turn_count += 1;
            log_debug!("AgentRunner::run | turn={}/{} ", turn_count, self.max_turns);

            let (should_continue, finish_reason, turn_usage) =
                self.run_one_turn(&mut messages, on_text, &mut None).await?;
            last_finish_reason = finish_reason;
            token_usage.prompt_tokens += turn_usage.prompt_tokens;
            token_usage.completion_tokens += turn_usage.completion_tokens;
            if !should_continue {
                break;
            }
        }

        Ok(AgentRunResult {
            messages,
            turn_count,
            finish_reason: last_finish_reason,
            token_usage,
        })
    }

    /// 单轮对话逻辑：
    /// 1. 通过 `stream_message` 发起流式请求；
    /// 2. 聚合 chunk 为完整的 Assistant 消息；
    /// 3. 若停止原因为 `ToolUse`，执行工具调用并将结果回传；
    /// 4. 返回 `true` 表示需要继续下一轮，`false` 表示本轮结束。
    async fn run_one_turn(
        &self,
        messages: &mut Vec<Message>,
        on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
        on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    ) -> Result<(bool, Option<FinishReason>, TokenUsage)> {
        let mut content_blocks = Vec::new();
        let mut finish_reason = None;
        let mut turn_usage = TokenUsage::default();

        // 消息历史截断：超过 30 条时只保留最近 30 条
        const MAX_CONTEXT_MESSAGES: usize = 30;
        let messages_for_llm: &[Message] = if messages.len() > MAX_CONTEXT_MESSAGES {
            let start = messages.len().saturating_sub(MAX_CONTEXT_MESSAGES);
            &messages[start..]
        } else {
            &messages[..]
        };

        self.client
            .stream_message(
                &self.system_prompt,
                messages_for_llm,
                &self.tools_schema,
                &mut |chunk| {
                    match &chunk.content {
                        ChunkContent::Text(text) | ChunkContent::Think(text) => {
                            if let Some(ref mut cb) = on_text {
                                cb(text);
                            }
                        }
                        _ => {}
                    }
                    Self::process_chunk(chunk, &mut content_blocks, &mut finish_reason, &mut turn_usage)
                },
            )
            .await?;

        // 从当前消息历史中继承 session_id
        let session_id = messages
            .last()
            .map(|m| m.session_id.clone())
            .unwrap_or_default();

        // 将 Assistant 的完整回复追加到消息历史
        messages.push(Message::new(
            session_id.clone(),
            Role::Assistant,
            content_blocks.clone(),
        ));

        // 非 ToolUse 则结束循环
        if finish_reason != Some(FinishReason::ToolUse) {
            log_debug!(
                "AgentRunner::run_one_turn | finish_reason={:?}, stopping",
                finish_reason
            );
            return Ok((false, finish_reason, turn_usage));
        }

        // 执行所有工具调用并收集结果
        let tool_results = execute_tool_calls(&content_blocks, on_tool_event).await;
        if tool_results.is_empty() {
            log_debug!("AgentRunner::run_one_turn | tool_use but no results, stopping");
            return Ok((false, finish_reason, turn_usage));
        }

        log_debug!(
            "AgentRunner::run_one_turn | pushing {} tool_result(s) back to LLM",
            tool_results.len()
        );

        // 客户端直出优化：如果所有工具都成功，且 Turn 1 已有前置文本说明，
        // 则跳过 Turn 2，直接格式化输出工具结果。
        let all_success = tool_results.iter().all(|p| {
            matches!(p, Part::ToolResult { is_error: false, .. })
        });
        let has_preamble = content_blocks.iter().any(|p| matches!(p, Part::Text { .. }));

        if all_success && has_preamble {
            let summary = format_tool_results(&content_blocks, &tool_results);
            log_debug!("AgentRunner::direct output | summary_len={}", summary.len());

            if let Some(ref mut cb) = on_text {
                cb(&summary);
            }

            if let Some(last) = messages.last_mut() {
                if last.role == Role::Assistant {
                    last.parts.push(Part::Text { text: summary });
                }
            }

            return Ok((false, finish_reason, turn_usage));
        }

        // 将工具结果封装为 User 消息回传
        messages.push(Message::new(session_id, Role::User, tool_results));

        Ok((true, finish_reason, turn_usage))
    }

    /// 处理流式 chunk，将内容聚合到 `content_blocks`。
    fn process_chunk(
        chunk: Chunk,
        content_blocks: &mut Vec<Part>,
        finish_reason: &mut Option<FinishReason>,
        token_usage: &mut TokenUsage,
    ) {
        match chunk.content {
            ChunkContent::Text(text) => {
                if let Some(Part::Text { text: last }) = content_blocks.last_mut() {
                    last.push_str(&text);
                } else {
                    content_blocks.push(Part::Text { text });
                }
            }
            ChunkContent::Think(text) => {
                if let Some(Part::Reasoning { thinking: last, .. }) = content_blocks.last_mut() {
                    last.push_str(&text);
                } else {
                    content_blocks.push(Part::Reasoning {
                        thinking: text,
                        signature: None,
                    });
                }
            }
            ChunkContent::ToolUse(ref tool) => {
                if let Part::ToolUse {
                    id,
                    name,
                    arguments,
                } = tool
                {
                    log_debug!(
                        "LLM tool_use | id={} | name={} | args={}",
                        id,
                        name,
                        arguments
                    );
                }
                content_blocks.push(tool.clone());
            }
            ChunkContent::Usage(usage) => {
                token_usage.prompt_tokens += usage.prompt_tokens;
                token_usage.completion_tokens += usage.completion_tokens;
            }
            ChunkContent::Finish(ref reason) => {
                log_debug!("LLM finish_reason={:?}", reason);
                *finish_reason = Some(reason.clone());
            }
        }
    }
}

/// 将工具执行结果格式化为直出文本。
fn format_tool_results(content_blocks: &[Part], tool_results: &[Part]) -> String {
    let mut lines = Vec::new();
    for result in tool_results {
        if let Part::ToolResult {
            tool_call_id,
            content,
            is_error,
        } = result
        {
            let emoji = if *is_error { "❌" } else { "✅" };
            let tool_name = content_blocks.iter().find_map(|p| {
                if let Part::ToolUse { id, name, .. } = p {
                    if id == tool_call_id {
                        Some(name.as_str())
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            if let Some(name) = tool_name {
                lines.push(format!("{} {} | {}", emoji, name, content));
            } else {
                lines.push(format!("{} {}", emoji, content));
            }
        }
    }
    lines.join("\n")
}
