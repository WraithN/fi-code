// =============================================================================
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================
// 本模块定义了对话中使用的核心数据结构与 agent 循环：
// - `LoopState`：agent 循环的运行时状态
// - `run_one_turn` / `agent_loop`：单轮/多轮对话驱动逻辑
//
// 消息类型（Message / Part / Role / ImageSource）已从本模块迁移到
// 独立的 `message` 模块，供 session、provider、tools 等多个模块共享。

use anyhow::Result;

use crate::agent::PromptBuilder;
use crate::log_block;
use crate::log_debug;
use crate::log_trace;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
use crate::provider::execute_tool_calls;
use crate::session::message::{Message, Part, Role};
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
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
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
pub async fn run_one_turn<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    let system_prompt = PromptBuilder::new().build(&tool_schema());

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

    log_debug!(
        "run_one_turn start | turn={} | messages={}",
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

    log_trace!(
        "tools_schema | {}",
        serde_json::to_string_pretty(&tool_schema()).unwrap_or_default()
    );

    client
        .stream_message(
            &system_prompt,
            &state.messages,
            &tool_schema(),
            &mut |chunk| {
                match chunk.content {
                    // 文本增量：与最后一个 Text 块合并，避免历史记录碎片化
                    ChunkContent::Text(text) => {
                        if let Some(Part::Text { text: last }) = content_blocks.last_mut() {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Text { text });
                        }
                    }
                    // 思考增量：与最后一个 Reasoning 块合并
                    ChunkContent::Think(text) => {
                        if let Some(Part::Reasoning { thinking: last, .. }) =
                            content_blocks.last_mut()
                        {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Reasoning {
                                thinking: text,
                                signature: None,
                            });
                        }
                    }
                    // 完整的工具调用块（客户端已拼装完毕）
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
                    // 流结束标志
                    ChunkContent::Finish(ref reason) => {
                        log_debug!("LLM finish_reason={:?}", reason);
                        finish_reason = Some(reason.clone());
                    }
                }
            },
        )
        .await?;

    // 从当前消息历史中继承 session_id，确保工具结果消息与对话属于同一会话
    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    log_debug!(
        "assistant message appended | blocks={}",
        content_blocks.len()
    );

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

    // 判断停止原因：只有明确为 ToolUse 时才继续执行工具调用回合
    if finish_reason != Some(FinishReason::ToolUse) {
        state.transition_reason = None;
        log_debug!("run_one_turn end | no tool use");
        return Ok(false);
    }

    // 执行所有工具调用，并收集结果
    let tool_results = execute_tool_calls(&content_blocks);
    if tool_results.is_empty() {
        state.transition_reason = None;
        log_debug!("run_one_turn end | tool_use finish but no results");
        return Ok(false);
    }

    log_debug!(
        "pushing tool_results back to LLM | results={}",
        tool_results.len()
    );
    for (idx, tr) in tool_results.iter().enumerate() {
        log_trace!("tool_result[{}] | {:?}", idx, tr);
    }

    // 将工具结果封装为 User 消息回传（符合 OpenAI / Anthropic API 的角色交替要求）
    state
        .messages
        .push(Message::new(session_id, Role::User, tool_results));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());

    log_debug!("run_one_turn end | will continue next turn");
    Ok(true)
}

// =============================================================================
// 异步函数：代理主循环
// =============================================================================

/// Agent 主循环：不断调用 `run_one_turn` 直到对话自然结束。
pub async fn agent_loop<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<()> {
    while run_one_turn(client, state).await? {}
    Ok(())
}
