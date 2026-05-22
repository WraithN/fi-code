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
use crate::log_warn;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason, TokenUsage};
use crate::provider::execute_tool_calls;
use crate::provider::Chunk;
use crate::session::message::{Message, Part, Role, TokenUsage as MsgTokenUsage};
use crate::agent::profile::AgentProfile;
use crate::skills::get_registry;
use crate::tools::tool_schema;
use fi_code_shared::constants::*;
use fi_code_shared::dto::AgentType;

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
    /// 增量压缩摘要，仅在内存中存在，不持久化
    pub compression_summary: Option<Message>,
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
            token_usage: TokenUsage::default(),
            compression_summary: None,
        }
    }
}

// =============================================================================
// 单轮对话状态（TurnState）
// =============================================================================

/// 单轮对话的运行时状态。
///
/// 封装 `run_one_turn` 内部的所有整轮生命周期变量，
/// 避免函数内局部变量散乱，提升可读性和可维护性。
#[derive(Debug)]
pub struct TurnState {
    /// 聚合 LLM 流式输出的所有内容块（Text / Think / ToolUse）
    pub content_blocks: Vec<Part>,
    /// LLM 停止原因
    pub finish_reason: Option<FinishReason>,
    /// 本轮 Token 使用量
    pub turn_usage: TokenUsage,
    /// 当前会话 ID
    pub session_id: String,
    /// WaveMarker：标记本轮的 Git 快照、时间戳、Token delta 等元信息
    pub wave_marker: Part,
    /// WaveMarker delta 计算的 Token 基线
    pub token_baseline: TokenUsage,
    /// Assistant 消息在 messages 中的索引（用于后续更新 WaveMarker）
    pub assistant_idx: usize,
}

impl TurnState {
    /// 创建新的 TurnState。
    ///
    /// `wave_step` 是 WaveMarker 的 step 值（通常等于当前轮数 + 1）。
    /// `token_baseline` 是计算 delta 的基线（通常为上一轮结束时的累计 Token）。
    pub fn new(session_id: String, wave_step: u32, token_baseline: TokenUsage) -> Self {
        let snapshot = crate::tools::basic_tools::BasicTool::git_write_tree().ok();
        let wave_marker = Part::WaveMarker {
            step: wave_step,
            total: None,
            git_snapshot: snapshot,
            timestamp: crate::session::message::current_timestamp_ms(),
            delta_tokens: MsgTokenUsage::default(),
        };
        Self {
            content_blocks: Vec::new(),
            finish_reason: None,
            turn_usage: TokenUsage::default(),
            session_id,
            wave_marker,
            token_baseline,
            assistant_idx: 0,
        }
    }

    /// 处理单个流式 chunk，更新 `content_blocks`、`finish_reason`、`turn_usage`。
    pub fn process_chunk(&mut self, chunk: Chunk) {
        match chunk.content {
            ChunkContent::Text(text) => {
                if let Some(Part::Text { text: last }) = self.content_blocks.last_mut() {
                    last.push_str(&text);
                } else {
                    self.content_blocks.push(Part::Text { text });
                }
            }
            ChunkContent::Think(text) => {
                if let Some(Part::Reasoning { thinking: last, .. }) = self.content_blocks.last_mut()
                {
                    last.push_str(&text);
                } else {
                    self.content_blocks.push(Part::Reasoning {
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
                    // 按 id 去重：已有同 id 的 ToolUse 则更新，避免 SSE 增量 flush 时重复 push
                    if let Some(existing) = self.content_blocks.iter_mut().find_map(|p| {
                        if let Part::ToolUse { id: eid, .. } = p {
                            if eid == id {
                                Some(p)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }) {
                        *existing = tool.clone();
                        return;
                    }
                }
                self.content_blocks.push(tool.clone());
            }
            ChunkContent::Usage(usage) => {
                self.turn_usage.prompt_tokens += usage.prompt_tokens;
                self.turn_usage.completion_tokens += usage.completion_tokens;
                log_debug!(
                    "LLM usage | prompt={} | completion={} | total_prompt={} | total_completion={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    self.turn_usage.prompt_tokens,
                    self.turn_usage.completion_tokens
                );
            }
            ChunkContent::Finish(ref reason) => {
                log_debug!("LLM finish_reason={:?}", reason);
                self.finish_reason = Some(reason.clone());
            }
            ChunkContent::Notification(_) => {
                // 通知类消息不需要聚合到 content_blocks，已在闭包中转发给客户端
            }
        }
    }

    /// 将 WaveMarker + content_blocks 组装为 Assistant 消息并追加到 messages。
    /// 同时记录 `assistant_idx`。
    pub fn append_assistant_message(&mut self, messages: &mut Vec<Message>) {
        let mut assistant_parts = vec![self.wave_marker.clone()];
        assistant_parts.extend(self.content_blocks.clone());
        self.assistant_idx = messages.len();
        messages.push(Message::new(
            self.session_id.clone(),
            Role::Assistant,
            assistant_parts,
        ));
    }

    /// 更新 messages 中 `assistant_idx` 对应消息的 WaveMarker。
    ///
    /// `total` 为总轮数（用于 WaveMarker 的 total 字段）。
    /// `current_usage` 为当前累计 Token 使用量（用于计算 delta）。
    pub fn update_wave_marker(
        &self,
        messages: &mut [Message],
        total: Option<u32>,
        current_usage: &TokenUsage,
    ) {
        if let Some(Part::WaveMarker {
            delta_tokens,
            total: t,
            ..
        }) = messages[self.assistant_idx].parts.first_mut()
        {
            *delta_tokens = MsgTokenUsage {
                prompt_tokens: current_usage
                    .prompt_tokens
                    .saturating_sub(self.token_baseline.prompt_tokens),
                completion_tokens: current_usage
                    .completion_tokens
                    .saturating_sub(self.token_baseline.completion_tokens),
            };
            *t = total;
        }
    }

    /// 判断本轮的 ToolUse 是否需要 MCP 两步发现
    ///（即存在 mcp: 前缀且参数为空的工具调用）。
    pub fn needs_mcp_two_step(&self) -> bool {
        self.content_blocks.iter().any(|p| {
            if let Part::ToolUse {
                name, arguments, ..
            } = p
            {
                name.starts_with("mcp:")
                    && arguments.as_object().map(|o| o.is_empty()).unwrap_or(true)
            } else {
                false
            }
        })
    }

    /// 累加本轮 Token 使用量到 LoopState。
    pub fn accumulate_token_usage(&self, state: &mut LoopState) {
        state.token_usage.prompt_tokens += self.turn_usage.prompt_tokens;
        state.token_usage.completion_tokens += self.turn_usage.completion_tokens;
    }
}

#[cfg(debug_assertions)]
static PROMPT_LOGGED_ONCE: std::sync::Once = std::sync::Once::new();

// =============================================================================
// 异步函数：运行一轮对话
// =============================================================================

/// 运行单轮对话：
/// 1. 通过 `stream_message` 发起流式请求，并传入闭包实时消费 Chunk；
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
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    sse_sender: Option<&crate::server::transport::sse::SseSender>,
    // 父级 trace context（来自 ChatSpan）：用于 OTel parent-span 传播
    parent_cx: Option<&opentelemetry::Context>,
) -> Result<bool> {
    use crate::observability::otel;

    // 启动本轮 TurnSpan，所有子 span（LLM / Tool / Compression）都以此为父
    let turn_span = otel::start_turn_span(parent_cx, state.turn_count);
    let turn_cx = turn_span.context();

    // 从当前消息历史中继承 session_id，确保工具结果消息与对话属于同一会话
    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    let mut turn = TurnState::new(
        session_id.clone(),
        state.turn_count as u32 + 1,
        state.token_usage.clone(),
    );

    // 发送 WaveMarker SSE 事件
    if let Some(ref mut cb) = on_tool_event {
        let _ = cb(crate::server::transport::sse::SseEvent::Part {
            part: turn.wave_marker.clone(),
        });
    }

    // === 上下文压缩检查 ===
    if crate::agent::compression::should_compress(&state.messages) {
        let needs_compress = state.compression_summary.is_none()
            || crate::agent::compression::find_compression_range(&state.messages).is_some();

        if needs_compress {
            log_info!(
                "[Compression] Triggered | messages={} | turn={}",
                state.messages.len(),
                state.turn_count
            );

            match crate::agent::compression::compress_history(state, client, sse_sender, Some(&turn_cx)).await {
                Ok(summary) => {
                    state.compression_summary = Some(summary);
                    log_info!("[Compression] Completed successfully");
                }
                Err(e) => {
                    log_error!("[Compression] Failed: {}", e);
                }
            }
        }
    }

    let registry = get_registry();
    let schema = tool_schema().await;
    let profile = AgentProfile::for_type(agent_type);
    let filtered_schema = profile.tool_filter.apply(&schema);
    let system_prompt = PromptBuilder::new().build_with_profile(&filtered_schema, registry, profile);

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

    log_info!(
        "[Agent] run_one_turn start | turn={} | messages={}",
        state.turn_count,
        state.messages.len()
    );

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

    let llm_messages = crate::agent::compression::build_llm_messages(state);
    log_debug!(
        "[Agent] context built | total={} | sent={}",
        state.messages.len(),
        llm_messages.len()
    );

    // 启动 LlmGeneration span：以 turn_cx 为父，作为 Langfuse generation observation
    // 通过 AIClient trait 上报真实 model_name / provider_kind，支撑 Langfuse 按模型/厂商聚合
    // 关闭可观测性时跳过 JSON 序列化（避免对每轮历史做无意义的 50KB+ 序列化）
    let llm_messages_json = if crate::observability::is_enabled() {
        serde_json::to_string(&llm_messages).unwrap_or_default()
    } else {
        String::new()
    };
    let llm_gen = otel::start_llm_generation(
        Some(&turn_cx),
        client.model_name(),
        client.provider_kind(),
        &llm_messages_json,
    );

    if let Err(e) = client
        .stream_message(&system_prompt, &llm_messages, &schema, &mut |chunk| {
            // 实时转发文本内容和系统通知，实现真流式
            match &chunk.content {
                ChunkContent::Text(text) | ChunkContent::Think(text) => {
                    if let Some(ref mut cb) = on_text {
                        cb(text);
                    }
                }
                ChunkContent::Notification(msg) => {
                    if let Some(ref mut cb) = on_text {
                        cb(msg);
                    }
                }
                _ => {}
            }
            turn.process_chunk(chunk);
        })
        .await
    {
        // 记录错误到 LLM generation span（drop 时自动结束）
        log_error!("[Agent] stream_message failed | turn={} | err={}", state.turn_count, e);
        return Err(e);
    }

    // LLM 调用完成：上报 finish_reason / token_usage 到 generation span
    if let Some(ref fr) = turn.finish_reason {
        llm_gen.record_finish_reason(&format!("{:?}", fr));
    }
    let total_tok = turn.turn_usage.prompt_tokens + turn.turn_usage.completion_tokens;
    llm_gen.record_usage(
        turn.turn_usage.prompt_tokens,
        turn.turn_usage.completion_tokens,
        total_tok,
    );
    // 聚合 assistant 文本作为输出（脱敏在 facade 内部完成）
    let completion_text: String = turn
        .content_blocks
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    llm_gen.record_output(&completion_text);

    log_info!(
        "[Agent] LLM stream complete | turn={} | blocks={} | turn_usage={:?}",
        state.turn_count,
        turn.content_blocks.len(),
        turn.turn_usage
    );

    log_debug!(
        "[Agent] assistant message appended | blocks={}",
        turn.content_blocks.len()
    );

    // 将 Assistant 的完整回复追加到状态（包含 WaveMarker）
    for (idx, block) in turn.content_blocks.iter().enumerate() {
        let preview = format!("{:?}", block).chars().take(200).collect::<String>();
        log_debug!("assistant block[{}] | {}", idx, preview);
        log_trace!("assistant block[{}] | {:?}", idx, block);
    }
    turn.append_assistant_message(&mut state.messages);

    // 累加本轮 Token 使用量到全局状态
    turn.accumulate_token_usage(state);

    // 判断停止原因：只有明确为 ToolUse 时才继续执行工具调用回合
    if turn.finish_reason != Some(FinishReason::ToolUse) {
        turn.update_wave_marker(
            &mut state.messages,
            Some(state.turn_count as u32 + 1),
            &state.token_usage,
        );
        state.transition_reason = None;
        log_info!(
            "[Agent] run_one_turn end | no tool use | finish_reason={:?}",
            turn.finish_reason
        );
        // OTel：TurnSpan 在函数返回时 drop，自动 end
        return Ok(false);
    }

    // =============================================================================
    // MCP Two-Step-Discovery
    // =============================================================================
    if turn.needs_mcp_two_step() {
        let schema_texts = collect_mcp_schema_texts(&turn.content_blocks).await;

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
            turn.content_blocks.clear();
            turn.finish_reason = None;

            if let Err(e) = client
                .stream_message(&system_prompt, &llm_messages, &schema, &mut |chunk| {
                    turn.process_chunk(chunk);
                })
                .await
            {
                log_error!("[Agent] MCP two-step stream failed | err={}", e);
                return Err(e);
            }

            state.messages.push(Message::new(
                session_id.clone(),
                Role::Assistant,
                turn.content_blocks.clone(),
            ));
        }
    }

    // 发送 ToolUse 事件
    for block in &turn.content_blocks {
        if let Part::ToolUse {
            id,
            name,
            arguments,
        } = block
        {
            if let Some(ref mut cb) = on_tool_event {
                let _ = cb(crate::server::transport::sse::SseEvent::Part {
                    part: crate::session::message::Part::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    },
                });
            }
        }
    }

    let is_aggressive = crate::agent::compression::should_compress(&state.messages);
    // 执行所有工具调用，并收集结果。传入 turn_cx 让每个工具的 ToolSpan 都挂在本 Turn 下
    let tool_results = execute_tool_calls(
        &turn.content_blocks,
        agent_type,
        on_tool_event,
        is_aggressive,
        Some(&turn_cx),
    )
    .await;
    if tool_results.is_empty() {
        turn.update_wave_marker(
            &mut state.messages,
            Some(state.turn_count as u32 + 1),
            &state.token_usage,
        );
        state.transition_reason = None;
        log_info!("[Agent] run_one_turn end | tool_use finish but no results");
        return Ok(false);
    }

    log_info!(
        "[Agent] pushing tool_results back to LLM | results={}",
        tool_results.len()
    );
    for (idx, tr) in tool_results.iter().enumerate() {
        log_trace!("tool_result[{}] | {:?}", idx, tr);
    }

    // 客户端直出优化：如果所有工具都成功，且 Turn 1 已有前置文本说明，
    // 则跳过 Turn 2（不再请求 LLM 写总结），直接格式化输出工具结果。
    let all_success = tool_results
        .iter()
        .all(|p| matches!(p, Part::ToolResult { .. }));
    let has_preamble = turn
        .content_blocks
        .iter()
        .any(|p| matches!(p, Part::Text { .. }));

    if all_success && has_preamble {
        turn.update_wave_marker(
            &mut state.messages,
            Some(state.turn_count as u32 + 1),
            &state.token_usage,
        );
        let summary = format_tool_results(&turn.content_blocks, &tool_results);
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
        // 在 TurnSpan 上记录迁移原因，便于 Langfuse 查看
        turn_span.set_transition_reason("direct_output");
        log_info!("[Agent] run_one_turn end | direct output, no turn 2");
        return Ok(false);
    }

    // 记录日志必须在 tool_results 被 move 进 messages 之前
    // OTel：tool_results 的每条已被各自 ToolSpan 记录（execute_tool_calls 内部），此处无需再写

    // 将工具结果封装为 User 消息回传（符合 OpenAI / Anthropic API 的角色交替要求）
    state
        .messages
        .push(Message::new(session_id, Role::User, tool_results));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());
    turn_span.set_transition_reason("tool_result");

    turn.update_wave_marker(&mut state.messages, None, &state.token_usage);

    log_info!(
        "[Agent] run_one_turn end | will continue next turn | next_turn={}",
        state.turn_count
    );
    Ok(true)
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
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    sse_sender: Option<&crate::server::transport::sse::SseSender>,
    // 父级 trace context（通常来自 ChatSpan）：所有 Turn 都以此为父
    parent_cx: Option<&opentelemetry::Context>,
) -> Result<()> {
    let mut retry_count = 0u32;

    log_info!(
        "[Agent] agent_loop start | messages={} | turn_count={}",
        state.messages.len(),
        state.turn_count
    );

    loop {
        // 保存当前状态，以便重试时恢复
        let messages_len_before = state.messages.len();
        let turn_count_before = state.turn_count;
        let token_usage_before = state.token_usage;
        let transition_reason_before = state.transition_reason.clone();

        match run_one_turn(client, state, agent_type, on_text, on_tool_event, sse_sender, parent_cx).await {
            Ok(should_continue) => {
                retry_count = 0;
                if !should_continue {
                    break;
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                // 判断是否为可重试的网络/流错误
                let is_retryable = err_str.contains("interrupted")
                    || err_str.contains("timeout")
                    || err_str.contains("connection")
                    || err_str.contains("reset")
                    || err_str.contains("broken pipe");

                if is_retryable && retry_count < MAX_RUN_ONE_TURN_RETRIES {
                    retry_count += 1;
                    // 恢复状态到 run_one_turn 之前
                    state.messages.truncate(messages_len_before);
                    state.turn_count = turn_count_before;
                    state.token_usage = token_usage_before;
                    state.transition_reason = transition_reason_before;

                    let retry_msg = format!(
                        "⚠️ 对话流中断，正在自动重试 ({}/{})",
                        retry_count, MAX_RUN_ONE_TURN_RETRIES
                    );
                    log_warn!(
                        "[Agent] run_one_turn failed with retryable error, retrying {}/{} | {}",
                        retry_count,
                        MAX_RUN_ONE_TURN_RETRIES,
                        e
                    );
                    if let Some(ref mut cb) = on_text {
                        cb(&retry_msg);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }

                log_error!("[Agent] agent_loop end with error | {}", e);
                return Err(e);
            }
        }

        if state.turn_count > MAX_TURNS {
            log_error!("[Agent] agent_loop max turns exceeded | {}", MAX_TURNS);
            return Err(anyhow::anyhow!(
                "Agent exceeded maximum turns ({})",
                MAX_TURNS
            ));
        }
    }

    log_info!(
        "[Agent] agent_loop end | total_turns={} | transition_reason={:?}",
        state.turn_count,
        state.transition_reason
    );

    // 在 ChatSpan 上记录最终消息快照，便于 Langfuse 检视完整对话
    // 必须经 redact_and_truncate 处理：清洗敏感字段（API Key / Bearer Token）并截断至 50KB，
    // 避免泄露密钥或撑爆 OTel 后端
    if crate::observability::is_enabled() {
        if let Some(cx) = parent_cx {
            use opentelemetry::trace::{Span, TraceContextExt};
            let raw = serde_json::to_string(&state.messages).unwrap_or_default();
            let snapshot = crate::observability::redact::redact_and_truncate(&raw);
            cx.span().set_attribute(opentelemetry::KeyValue::new(
                crate::observability::attrs::FI_MESSAGES_SNAPSHOT,
                snapshot,
            ));
        }
    }

    Ok(())
}
