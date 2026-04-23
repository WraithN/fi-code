// =============================================================================
// runner 模块：AgentRunner 抽象
// =============================================================================
// 本模块将 Agent 的运行逻辑封装为一个可复用的结构体，
// 便于上层（如 TaskManager）按需驱动多轮对话，而无需直接依赖全局状态。

use anyhow::Result;

use crate::log_debug;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
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
        let mut messages = initial_messages;
        let mut turn_count = 0usize;
        let mut last_finish_reason = None;

        while turn_count < self.max_turns {
            turn_count += 1;
            log_debug!("AgentRunner::run | turn={}/{} ", turn_count, self.max_turns);

            let (should_continue, finish_reason) = self.run_one_turn(&mut messages).await?;
            last_finish_reason = finish_reason;
            if !should_continue {
                break;
            }
        }

        Ok(AgentRunResult {
            messages,
            turn_count,
            finish_reason: last_finish_reason,
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
    ) -> Result<(bool, Option<FinishReason>)> {
        let mut content_blocks = Vec::new();
        let mut finish_reason = None;

        self.client
            .stream_message(
                &self.system_prompt,
                messages,
                &self.tools_schema,
                &mut |chunk| Self::process_chunk(chunk, &mut content_blocks, &mut finish_reason),
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
            log_debug!("AgentRunner::run_one_turn | finish_reason={:?}, stopping", finish_reason);
            return Ok((false, finish_reason));
        }

        // 执行所有工具调用并收集结果
        let tool_results = execute_tool_calls(&content_blocks).await;
        if tool_results.is_empty() {
            log_debug!("AgentRunner::run_one_turn | tool_use but no results, stopping");
            return Ok((false, finish_reason));
        }

        log_debug!(
            "AgentRunner::run_one_turn | pushing {} tool_result(s) back to LLM",
            tool_results.len()
        );

        // 将工具结果封装为 User 消息回传
        messages.push(Message::new(session_id, Role::User, tool_results));

        Ok((true, finish_reason))
    }

    /// 处理流式 chunk，将内容聚合到 `content_blocks`。
    fn process_chunk(
        chunk: Chunk,
        content_blocks: &mut Vec<Part>,
        finish_reason: &mut Option<FinishReason>,
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
                if let Part::ToolUse { id, name, arguments } = tool {
                    log_debug!(
                        "LLM tool_use | id={} | name={} | args={}",
                        id,
                        name,
                        arguments
                    );
                }
                content_blocks.push(tool.clone());
            }
            ChunkContent::Finish(ref reason) => {
                log_debug!("LLM finish_reason={:?}", reason);
                *finish_reason = Some(reason.clone());
            }
        }
    }
}
