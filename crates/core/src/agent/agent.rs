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
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================
// 本模块定义了对话中使用的核心数据结构与 agent 循环：
// - `LoopState`：agent 循环的运行时状态
// - `run_one_turn` / `agent_loop`：单轮/多轮对话驱动逻辑
//
// 消息类型（Message / Part / Role / ImageSource）已从本模块迁移到
// 独立的 `message` 模块，供 session、provider、tools 等多个模块共享。
//
// NOTE: `AgentRunner`（位于 `crate::agent::runner`）是新的可配置 Agent 循环抽象。
// `run_one_turn` 和 `agent_loop` 保留用于向后兼容现有调用方（如 main.rs）。
// 新代码应优先使用 `AgentRunner`。

use anyhow::Result;

use crate::agent::PromptBuilder;
use crate::log_block;
use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_trace;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason, TokenUsage};
use crate::provider::execute_tool_calls;
use crate::provider::Chunk;
use crate::session::message::{Message, Part, Role};
use crate::skills::get_registry;
use crate::tools::tool_schema;

// =============================================================================
// 对话循环状态（LoopState）
// =============================================================================

/// 对话循环状态，保存消息历史、当前轮数以及状态迁移原因。
#[derive(Debug)]
pub struct LoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition_reason: Option<String>,
    /// 累计 Token 使用量
    pub token_usage: TokenUsage,
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
            token_usage: TokenUsage::default(),
        }
    }
}

#[cfg(debug_assertions)]
static PROMPT_LOGGED_ONCE: std::sync::Once = std::sync::Once::new();

// =============================================================================
// 异步函数：运行一轮对话
// =============================================================================

/// 运行单轮对话：
/// 1. 通过 `stream_message` 发起流式请求，并传入闭包实时消费 Chunk；
/// 2. 在闭包内部将同类型的文本/思考增量聚合为完整的内容块；
/// 3. tool_use 由客户端拼装完整后，以 `ChunkContent::ToolUse` 形式传入闭包；
/// 4. 将 assistant 回复追加到状态；
/// 5. 若停止原因为 `ToolUse`，则执行工具调用并将结果以 user 身份回传；
/// 6. 返回 `true` 表示需要继续下一轮，`false` 表示本轮结束。
/// 辅助函数：处理流式 chunk，将内容聚合到 content_blocks
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
            log_debug!(
                "LLM usage | prompt={} | completion={} | total_prompt={} | total_completion={}",
                usage.prompt_tokens,
                usage.completion_tokens,
                token_usage.prompt_tokens,
                token_usage.completion_tokens
            );
        }
        ChunkContent::Finish(ref reason) => {
            log_debug!("LLM finish_reason={:?}", reason);
            *finish_reason = Some(reason.clone());
        }
    }
}

/// 收集所有需要两步发现的 MCP 工具的 schema 文本。
async fn collect_mcp_schema_texts(content_blocks: &[Part]) -> Vec<String> {
    let mut schema_texts = Vec::new();
    for part in content_blocks {
        let Part::ToolUse { name, .. } = part else {
            continue;
        };
        if !name.starts_with("mcp:") {
            continue;
        }
        let Some(mcp) = crate::tools::get_mcp_manager() else {
            continue;
        };
        let Some(schema) = mcp.tool_schema(name).await else {
            continue;
        };
        let Some(input_schema) = schema.input_schema else {
            continue;
        };
        schema_texts.push(format!(
            "工具 `{}` 的完整参数格式：\n```json\n{}\n```",
            name,
            serde_json::to_string_pretty(&input_schema).unwrap_or_default()
        ));
    }
    schema_texts
}

pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;
    let mut turn_usage = TokenUsage::default();

    let registry = get_registry();
    let schema = tool_schema().await;
    let system_prompt = PromptBuilder::new().build(&schema, registry);

    #[cfg(debug_assertions)]
    PROMPT_LOGGED_ONCE.call_once(|| {
        log_block!(
            crate::utils::log::LogLevel::Debug,
            "SYSTEM PROMPT (first)",
            &system_prompt
        );
    });
    log_block!(
        crate::utils::log::LogLevel::Trace,
        "SYSTEM PROMPT",
        &system_prompt
    );

    log_info!("[Agent] run_one_turn start | turn={} | messages={}", state.turn_count, state.messages.len());

    for (idx, msg) in state.messages.iter().enumerate() {
        let preview: String = msg
            .parts
            .iter()
            .map(|p| format!("{:?}", p))
            .collect::<String>()
            .chars()
            .take(150)
            .collect();
        log_debug!(
            "message[{}] | role={:?} | preview={}",
            idx,
            msg.role,
            preview
        );
        log_trace!(
            "message[{}] | role={:?} | parts={:?}",
            idx,
            msg.role,
            msg.parts
        );
    }

    let schema = tool_schema().await;
    log_trace!(
        "tools_schema | {}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );

    // 消息历史截断：超过 30 条时只保留最近 30 条，防止长对话导致 Prompt 无限膨胀
    const MAX_CONTEXT_MESSAGES: usize = 30;
    let messages_for_llm: &[Message] = if state.messages.len() > MAX_CONTEXT_MESSAGES {
        let start = state.messages.len().saturating_sub(MAX_CONTEXT_MESSAGES);
        &state.messages[start..]
    } else {
        &state.messages[..]
    };
    log_debug!("[Agent] context truncated | total={} | sent={}", state.messages.len(), messages_for_llm.len());

    client
        .stream_message(&system_prompt, messages_for_llm, &schema, &mut |chunk| {
            // 实时转发文本内容，实现真流式
            match &chunk.content {
                ChunkContent::Text(text) | ChunkContent::Think(text) => {
                    if let Some(ref mut cb) = on_text {
                        cb(text);
                    }
                }
                _ => {}
            }
            process_chunk(chunk, &mut content_blocks, &mut finish_reason, &mut turn_usage)
        })
        .await?;

    // 从当前消息历史中继承 session_id，确保工具结果消息与对话属于同一会话
    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    log_debug!("[Agent] assistant message appended | blocks={}", content_blocks.len());

    // 将 Assistant 的完整回复追加到状态
    for (idx, block) in content_blocks.iter().enumerate() {
        let preview = format!("{:?}", block).chars().take(200).collect::<String>();
        log_debug!("assistant block[{}] | {}", idx, preview);
        log_trace!("assistant block[{}] | {:?}", idx, block);
    }
    state.messages.push(Message::new(
        session_id.clone(),
        Role::Assistant,
        content_blocks.clone(),
    ));

    // 累加本轮 Token 使用量到全局状态
    state.token_usage.prompt_tokens += turn_usage.prompt_tokens;
    state.token_usage.completion_tokens += turn_usage.completion_tokens;

    // 判断停止原因：只有明确为 ToolUse 时才继续执行工具调用回合
    if finish_reason != Some(FinishReason::ToolUse) {
        state.transition_reason = None;
        log_info!("[Agent] run_one_turn end | no tool use | finish_reason={:?}", finish_reason);
        return Ok(false);
    }

    // =============================================================================
    // MCP Two-Step-Discovery
    // =============================================================================
    // 如果 LLM 返回的 ToolUse 中包含 mcp: 前缀且参数为空，
    // 则获取完整 input_schema，构造补充消息，重新调用 LLM。

    let needs_two_step = content_blocks.iter().any(|p| {
        if let Part::ToolUse {
            name, arguments, ..
        } = p
        {
            name.starts_with("mcp:") && arguments.as_object().map(|o| o.is_empty()).unwrap_or(true)
        } else {
            false
        }
    });

    if needs_two_step {
        let schema_texts = collect_mcp_schema_texts(&content_blocks).await;

        if !schema_texts.is_empty() {
            state.messages.push(Message::new(
                session_id.clone(),
                Role::User,
                vec![Part::Text {
                    text: format!(
                        "请为以下 MCP 工具提供正确的参数：\n\n{}",
                        schema_texts.join("\n\n")
                    ),
                }],
            ));

            // 重新调用 LLM 获取带参数的 ToolUse
            content_blocks.clear();
            finish_reason = None;

            client
                .stream_message(&system_prompt, &state.messages, &schema, &mut |chunk| {
                    process_chunk(chunk, &mut content_blocks, &mut finish_reason, &mut turn_usage)
                })
                .await?;

            state.messages.push(Message::new(
                session_id.clone(),
                Role::Assistant,
                content_blocks.clone(),
            ));
        }
    }

    // 发送 ToolUse 事件
    for block in &content_blocks {
        if let Part::ToolUse { id, name, arguments } = block {
            if let Some(ref mut cb) = on_tool_event {
                let _ = cb(crate::server::transport::sse::SseEvent::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
            }
        }
    }

    // 执行所有工具调用，并收集结果
    let tool_results = execute_tool_calls(&content_blocks, on_tool_event).await;
    if tool_results.is_empty() {
        state.transition_reason = None;
        log_info!("[Agent] run_one_turn end | tool_use finish but no results");
        return Ok(false);
    }

    log_info!("[Agent] pushing tool_results back to LLM | results={}", tool_results.len());
    for (idx, tr) in tool_results.iter().enumerate() {
        log_trace!("tool_result[{}] | {:?}", idx, tr);
    }

    // 客户端直出优化：如果所有工具都成功，且 Turn 1 已有前置文本说明，
    // 则跳过 Turn 2（不再请求 LLM 写总结），直接格式化输出工具结果。
    let all_success = tool_results.iter().all(|p| {
        matches!(p, Part::ToolResult { is_error: false, .. })
    });
    let has_preamble = content_blocks.iter().any(|p| matches!(p, Part::Text { .. }));

    if all_success && has_preamble {
        let summary = format_tool_results(&content_blocks, &tool_results);
        log_info!("[Agent] direct output | summary_len={}", summary.len());

        // 通过 SSE 实时发送总结文本到前端
        if let Some(ref mut cb) = on_text {
            cb(&summary);
        }

        // 将总结追加到最后一条 Assistant 消息中
        if let Some(last) = state.messages.last_mut() {
            if last.role == Role::Assistant {
                last.parts.push(Part::Text { text: summary });
            }
        }

        state.transition_reason = Some("direct_output".to_string());
        log_info!("[Agent] run_one_turn end | direct output, no turn 2");
        return Ok(false);
    }

    // 将工具结果封装为 User 消息回传（符合 OpenAI / Anthropic API 的角色交替要求）
    state
        .messages
        .push(Message::new(session_id, Role::User, tool_results));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());

    log_info!("[Agent] run_one_turn end | will continue next turn | next_turn={}", state.turn_count);
    Ok(true)
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

// =============================================================================
// 异步函数：代理主循环
// =============================================================================

/// Agent 主循环：不断调用 `run_one_turn` 直到对话自然结束。
///
/// `on_text` 为可选的实时文本回调，收到每个 Text/Think chunk 时立即调用。
/// 用于 TUI 真流式渲染：首 token 到达即可显示，无需等待整轮完成。
pub async fn agent_loop<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<()> {
    const MAX_TURNS: usize = 25;
    log_info!("[Agent] agent_loop start | messages={} | turn_count={}", state.messages.len(), state.turn_count);
    while run_one_turn(client, state, on_text, on_tool_event).await? {
        if state.turn_count > MAX_TURNS {
            log_error!("[Agent] agent_loop max turns exceeded | {}", MAX_TURNS);
            return Err(anyhow::anyhow!(
                "Agent exceeded maximum turns ({})",
                MAX_TURNS
            ));
        }
    }
    log_info!("[Agent] agent_loop end | total_turns={} | transition_reason={:?}", state.turn_count, state.transition_reason);
    Ok(())
}
