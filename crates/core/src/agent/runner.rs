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

use crate::agent::profile::AgentProfile;
use crate::agent::PromptBuilder;
use crate::agent::TurnState;
use crate::log_debug;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason, TokenUsage};
use crate::provider::execute_tool_calls;
use crate::session::message::{Message, Part, Role};
use fi_code_shared::dto::AgentType;

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
    pub agent_type: AgentType,
    max_turns: usize,
}

impl AgentRunner {
    /// 构造一个 `AgentRunner`。
    pub fn new(client: Box<dyn AIClient>, agent_type: AgentType) -> Self {
        Self {
            client,
            agent_type,
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
        // 从当前消息历史中继承 session_id
        let session_id = messages
            .last()
            .map(|m| m.session_id.clone())
            .unwrap_or_default();

        let assistant_count = messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .count() as u32;
        let mut turn = TurnState::new(
            session_id.clone(),
            assistant_count + 1,
            TokenUsage::default(),
        );

        // 发送 WaveMarker SSE 事件
        if let Some(ref mut cb) = on_tool_event {
            let _ = cb(crate::server::transport::sse::SseEvent::Part {
                part: turn.wave_marker.clone(),
            });
        }

        // 获取 Profile 并过滤工具
        let profile = AgentProfile::for_type(self.agent_type);
        let all_schema = crate::tools::tool_schema().await;
        let tools_schema = profile.tool_filter.apply(&all_schema);

        // 构建带 Profile 后缀的系统提示词
        let registry = crate::skills::get_registry();
        let system_prompt =
            PromptBuilder::new().build_with_profile(&tools_schema, &registry, profile);

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
                &system_prompt,
                messages_for_llm,
                &tools_schema,
                &mut |chunk| {
                    match &chunk.content {
                        ChunkContent::Text(text) => {
                            if let Some(ref mut cb) = on_text {
                                cb(text);
                            }
                        }
                        ChunkContent::Think(text) => {
                            if let Some(ref mut cb) = on_tool_event {
                                let _ = cb(crate::server::transport::sse::SseEvent::Part {
                                    part: Part::Reasoning {
                                        thinking: text.clone(),
                                        signature: None,
                                    },
                                });
                            }
                        }
                        ChunkContent::ToolUse(tool) => {
                            // ask_for_question 通过 QuestionAsk SSE 事件与前端交互，不展示原始参数 JSON
                            if !matches!(tool, Part::ToolUse { name, .. } if name == "ask_for_question") {
                                if let Some(ref mut cb) = on_tool_event {
                                    let _ = cb(crate::server::transport::sse::SseEvent::Part {
                                        part: tool.clone(),
                                    });
                                }
                            }
                        }
                        ChunkContent::Notification(msg) => {
                            if let Some(ref mut cb) = on_tool_event {
                                let _ = cb(crate::server::transport::sse::SseEvent::Message {
                                    content: msg.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                    turn.process_chunk(chunk);
                },
            )
            .await?;

        // 组装 Assistant 消息：WaveMarker + content_blocks
        turn.append_assistant_message(messages);

        // 非 ToolUse 则结束循环
        if turn.finish_reason != Some(FinishReason::ToolUse) {
            log_debug!(
                "AgentRunner::run_one_turn | finish_reason={:?}, stopping",
                turn.finish_reason
            );
            let assistant_count = messages
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .count() as u32;
            turn.update_wave_marker(messages, Some(assistant_count), &turn.turn_usage);
            return Ok((false, turn.finish_reason, turn.turn_usage));
        }

        let is_aggressive = crate::agent::compression::should_compress(messages);
        // 执行所有工具调用并收集结果（runner 暂无 chat 父 span，传 None）
        let tool_results = execute_tool_calls(
            &turn.content_blocks,
            self.agent_type,
            on_tool_event,
            is_aggressive,
            None,
        )
        .await;
        if tool_results.is_empty() {
            log_debug!("AgentRunner::run_one_turn | tool_use but no results, stopping");
            let assistant_count = messages
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .count() as u32;
            turn.update_wave_marker(messages, Some(assistant_count), &turn.turn_usage);
            return Ok((false, turn.finish_reason, turn.turn_usage));
        }

        log_debug!(
            "AgentRunner::run_one_turn | pushing {} tool_result(s) back to LLM",
            tool_results.len()
        );

        // 客户端直出优化：如果所有工具都成功，且 Turn 1 已有前置文本说明，
        // 则跳过 Turn 2，直接格式化输出工具结果。
        let all_success = tool_results
            .iter()
            .all(|p| matches!(p, Part::ToolResult { .. }));
        let has_preamble = turn
            .content_blocks
            .iter()
            .any(|p| matches!(p, Part::Text { .. }));

        if all_success && has_preamble {
            let assistant_count = messages
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .count() as u32;
            turn.update_wave_marker(messages, Some(assistant_count), &turn.turn_usage);
            let summary = format_tool_results(&turn.content_blocks, &tool_results);
            log_debug!("AgentRunner::direct output | summary_len={}", summary.len());

            if let Some(ref mut cb) = on_text {
                cb(&summary);
            }

            if let Some(last) = messages.last_mut() {
                if last.role == Role::Assistant {
                    last.parts.push(Part::Text { text: summary });
                }
            }

            return Ok((false, turn.finish_reason, turn.turn_usage));
        }

        // 将工具结果封装为 User 消息回传
        messages.push(Message::new(session_id, Role::User, tool_results));

        turn.update_wave_marker(messages, None, &turn.turn_usage);

        Ok((true, turn.finish_reason, turn.turn_usage))
    }
}

/// 将工具执行结果格式化为直出文本。
fn format_tool_results(content_blocks: &[Part], tool_results: &[Part]) -> String {
    let mut lines = Vec::new();
    for result in tool_results {
        let (tool_call_id, content, is_error) = match result {
            Part::ToolResult {
                tool_call_id,
                content,
                ..
            } => (tool_call_id, content, false),
            Part::ToolError {
                tool_call_id,
                content,
                ..
            } => (tool_call_id, content, true),
            _ => continue,
        };
        let emoji = if is_error { "❌" } else { "✅" };
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
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fi_code_shared::dto::AgentType;

    #[test]
    fn test_agent_runner_new_with_plan() {
        let runner = AgentRunner::new(
            Box::new(crate::provider::mock_client::MockAIClient::new()),
            AgentType::Plan,
        );
        assert_eq!(runner.agent_type, AgentType::Plan);
    }
}
