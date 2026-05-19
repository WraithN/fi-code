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
// 上下文压缩模块
// =============================================================================
// 本模块负责：
// 1. Token 估算（当 LLM 不返回 usage 时使用）
// 2. 压缩阈值检测
// 3. 工具结果动态压缩
// 4. 历史消息范围计算（含 tool_use/tool_result 配对保护）
// 5. 增量压缩执行（通过 subagent summarize）
// 6. 构建供 LLM 使用的压缩消息视图

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;

use fi_code_shared::constants::*;
use fi_code_shared::dto::{Message, Part, Role};

use crate::agent::LoopState;
use crate::provider::base_client::AIClient;

// ---------------------------------------------------------------------------
// 全局上下文限制（由调用方根据配置设置）
// ---------------------------------------------------------------------------

static CONTEXT_LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_CONTEXT_LIMIT);

pub fn set_context_limit(limit: u32) {
    CONTEXT_LIMIT.store(limit, Ordering::Relaxed);
}

pub fn get_context_limit() -> u32 {
    CONTEXT_LIMIT.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Token 估算
// ---------------------------------------------------------------------------

const TOKEN_WEIGHT_ASCII: f64 = 0.25;
const TOKEN_WEIGHT_NON_ASCII: f64 = 0.67;

/// 估算文本的 token 数。
pub fn estimate_tokens(text: &str) -> u32 {
    text.chars()
        .map(|c| if c.is_ascii() { TOKEN_WEIGHT_ASCII } else { TOKEN_WEIGHT_NON_ASCII })
        .sum::<f64>()
        .ceil() as u32
}

/// 估算单条消息的 token 数。
pub fn estimate_message_tokens(msg: &Message) -> u32 {
    msg.parts.iter().map(|part| match part {
        Part::Text { text } => estimate_tokens(text),
        Part::ToolResult { content, .. } => estimate_tokens(content),
        Part::ToolError { content, .. } => estimate_tokens(content),
        _ => 20,
    }).sum()
}

/// 估算消息列表的总 token 数。
pub fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(estimate_message_tokens).sum()
}

// ---------------------------------------------------------------------------
// 阈值检测
// ---------------------------------------------------------------------------

/// 判断是否应该触发上下文压缩。
pub fn should_compress(messages: &[Message]) -> bool {
    let limit = get_context_limit();
    let threshold = (limit as f64 * COMPRESSION_THRESHOLD) as u32;
    let estimated = estimate_total_tokens(messages);
    estimated >= threshold
}

// ---------------------------------------------------------------------------
// 工具结果动态压缩
// ---------------------------------------------------------------------------

/// 压缩工具结果内容。
pub fn compress_tool_result(content: &str, is_aggressive: bool) -> String {
    let threshold = if is_aggressive {
        TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE
    } else {
        TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL
    };

    if content.len() <= threshold {
        return content.to_string();
    }

    let head_end = content
        .char_indices()
        .nth(TOOL_RESULT_COMPRESS_HEAD)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    let tail_start = content.len().saturating_sub(TOOL_RESULT_COMPRESS_TAIL);
    let truncated = content.len() - head_end - (content.len() - tail_start);

    format!(
        "{}\n\n... [{} chars truncated] ...\n\n{}",
        &content[..head_end],
        truncated,
        &content[tail_start..]
    )
}

// ---------------------------------------------------------------------------
// 压缩范围计算
// ---------------------------------------------------------------------------

/// 判断一条消息是否是纯粹的 tool_result 消息（不是用户主动输入）。
fn is_tool_result_message(msg: &Message) -> bool {
    msg.role == Role::User
        && msg.parts.iter().all(|p| matches!(p, Part::ToolResult { .. } | Part::ToolError { .. }))
}

/// 找到可以被压缩的消息范围。
///
/// 返回 `(start_idx, end_idx)` 包含性范围。
/// 保留最近 2 轮完整对话。
pub fn find_compression_range(messages: &[Message]) -> Option<(usize, usize)> {
    if messages.len() < 4 {
        return None;
    }

    let mut rounds_found = 0;
    let mut split_idx = messages.len();

    for (idx, msg) in messages.iter().enumerate().rev() {
        if msg.role == Role::User && !is_tool_result_message(msg) {
            rounds_found += 1;
            if rounds_found == 2 {
                split_idx = idx;
                break;
            }
        }
    }

    if rounds_found < 2 || split_idx == 0 {
        return None;
    }

    let safe_start = find_safe_split_point(messages, split_idx);

    if safe_start == 0 {
        return None;
    }

    Some((0, safe_start - 1))
}

/// 确保分割点不会切断 tool_use/tool_result 配对。
fn find_safe_split_point(messages: &[Message], mut split_idx: usize) -> usize {
    let tool_ids_in_range: HashSet<String> = messages[split_idx..]
        .iter()
        .filter_map(|msg| {
            msg.parts.iter().find_map(|part| match part {
                Part::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
        })
        .collect();

    let mut earliest_tool_result = split_idx;
    for (idx, msg) in messages[..split_idx].iter().enumerate().rev() {
        if let Some(tool_call_id) = msg.parts.iter().find_map(|p| match p {
            Part::ToolResult { tool_call_id, .. } | Part::ToolError { tool_call_id, .. } => {
                Some(tool_call_id.clone())
            }
            _ => None,
        }) {
            if tool_ids_in_range.contains(&tool_call_id) {
                earliest_tool_result = idx;
            }
        }
    }

    if earliest_tool_result < split_idx {
        for (idx, msg) in messages[..earliest_tool_result].iter().enumerate().rev() {
            if msg.parts.iter().any(|p| matches!(p, Part::ToolUse { .. })) {
                split_idx = idx;
                break;
            }
        }
    }

    split_idx
}

// ---------------------------------------------------------------------------
// 压缩 Subagent
// ---------------------------------------------------------------------------

const COMPRESSION_SYSTEM_PROMPT: &str = r#"你是一个对话摘要助手。你的任务是将一段对话历史压缩成简洁的摘要，供后续 AI 助手理解上下文。

摘要规则：
1. 保留所有关键决策、代码修改、文件路径、错误信息
2. 保留用户明确提出的需求和约束条件
3. 删除重复或冗余的中间推理步骤
4. 保留工具调用的关键结果（如 grep 找到了什么、bash 输出是什么）
5. 如果对话涉及多轮代码编辑，保留最终的代码状态描述
6. 摘要长度控制在 2000-4000 token 以内
7. 使用中文输出摘要（因为原始对话是中文）

输出格式：纯文本段落，不要加标题或标记。"#;

async fn run_compression_subagent<C: AIClient + ?Sized>(
    client: &C,
    messages_to_summarize: Vec<Message>,
) -> Result<String> {
    let empty_schema = serde_json::Value::Array(vec![]);
    let mut result_text = String::new();

    client
        .stream_message(
            COMPRESSION_SYSTEM_PROMPT,
            &messages_to_summarize,
            &empty_schema,
            &mut |chunk: crate::provider::base_client::Chunk| {
                if let crate::provider::base_client::ChunkContent::Text(text) = chunk.content {
                    result_text.push_str(&text);
                }
            },
        )
        .await?;

    if result_text.is_empty() {
        return Err(anyhow::anyhow!("Compression subagent returned no text"));
    }

    Ok(result_text)
}

// ---------------------------------------------------------------------------
// 增量压缩执行
// ---------------------------------------------------------------------------

/// 计算当前上下文占用比例（百分比）。
fn calculate_context_ratio(loop_state: &LoopState) -> u8 {
    let limit = get_context_limit();
    let current = estimate_total_tokens(&loop_state.messages);
    if limit == 0 { return 0; }
    ((current as f64 / limit as f64) * 100.0).min(100.0) as u8
}

/// 对会话历史执行增量压缩，返回新的 Summary 消息。
pub async fn compress_history<C: AIClient + ?Sized>(
    loop_state: &LoopState,
    client: &C,
    sse_sender: &crate::server::transport::sse::SseSender,
) -> Result<Message> {
    let range = find_compression_range(&loop_state.messages)
        .ok_or_else(|| anyhow::anyhow!("No compressible range found"))?;

    let original_count = loop_state.messages.len();
    let original_tokens = estimate_total_tokens(&loop_state.messages);

    // 发送压缩开始事件
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::CompressionStatus {
        is_compressing: true,
        progress: 0,
        context_ratio: calculate_context_ratio(loop_state),
        summary: None,
    }).await;

    let (start, end) = range;

    let mut to_compress = Vec::new();

    if let Some(ref summary) = loop_state.compression_summary {
        to_compress.push(summary.clone());
    }

    to_compress.extend(loop_state.messages[start..=end].iter().cloned());

    let summary_text = run_compression_subagent(client, to_compress).await?;

    let token_savings = if original_tokens > 0 {
        let saved = original_tokens.saturating_sub(estimate_tokens(&summary_text));
        ((saved as f64 / original_tokens as f64) * 100.0) as u8
    } else { 0 };

    let display_text = format!(
        "🗜️ 上下文已压缩 | {}条消息 → 1条摘要 | 节省 {}% tokens",
        original_count, token_savings
    );

    // 发送压缩完成事件
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::CompressionStatus {
        is_compressing: false,
        progress: 100,
        context_ratio: calculate_context_ratio(loop_state),
        summary: Some(display_text.clone()),
    }).await;

    // 在聊天流中插入系统通知
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::Part {
        part: Part::SystemNotice {
            kind: "compression_done".to_string(),
            content: display_text,
        },
    }).await;

    let session_id = loop_state
        .messages
        .first()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    Ok(Message::new(
        session_id,
        Role::User,
        vec![Part::Text { text: summary_text }],
    ))
}

/// 构建供 LLM 使用的消息视图。
pub fn build_llm_messages(loop_state: &LoopState) -> Vec<Message> {
    if let Some(ref summary) = loop_state.compression_summary {
        let mut result = Vec::new();
        result.push(summary.clone());

        if let Some((_, end)) = find_compression_range(&loop_state.messages) {
            result.extend(loop_state.messages[end + 1..].iter().cloned());
        }

        result
    } else {
        loop_state.messages.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_ascii() {
        let text = "Hello world";
        let tokens = estimate_tokens(text);
        // 11 ASCII chars * 0.25 = 2.75 -> ceil = 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        let text = "Hello 世界";
        let tokens = estimate_tokens(text);
        // 6 ASCII * 0.25 + 2 CJK * 0.67 = 1.5 + 1.34 = 2.84 -> ceil = 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_should_compress_below_threshold() {
        set_context_limit(100);
        let messages = vec![Message::new(
            "test".to_string(),
            Role::User,
            vec![Part::Text { text: "hi".to_string() }],
        )];
        assert!(!should_compress(&messages));
    }

    #[test]
    fn test_should_compress_above_threshold() {
        set_context_limit(100);
        // threshold = 85, need >= 85 estimated tokens
        // each char ~0.25 for ASCII, so ~400 chars = 100 tokens
        let long_text = "a".repeat(400);
        let messages = vec![Message::new(
            "test".to_string(),
            Role::User,
            vec![Part::Text { text: long_text }],
        )];
        assert!(should_compress(&messages));
    }

    #[test]
    fn test_compress_tool_result_short() {
        let content = "short";
        let result = compress_tool_result(content, false);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_compress_tool_result_normal() {
        let content = "a".repeat(10_000);
        let result = compress_tool_result(&content, false);
        assert!(result.contains("truncated"));
        assert!(result.starts_with("a"));
    }

    #[test]
    fn test_compress_tool_result_aggressive() {
        let content = "b".repeat(5_000);
        let result = compress_tool_result(&content, true);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_find_compression_range_insufficient_messages() {
        let messages = vec![
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u1".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a1".to_string() }]),
        ];
        assert!(find_compression_range(&messages).is_none());
    }

    #[test]
    fn test_find_compression_range_basic() {
        let messages = vec![
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u1".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a1".to_string() }]),
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u2".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a2".to_string() }]),
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u3".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a3".to_string() }]),
        ];
        let range = find_compression_range(&messages);
        assert!(range.is_some());
        let (start, end) = range.unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, 1);
    }

    #[test]
    fn test_find_safe_split_point_tool_pairing() {
        let messages = vec![
            Message::new("s".to_string(), Role::Assistant, vec![
                Part::ToolUse { id: "t1".to_string(), name: "bash".to_string(), arguments: serde_json::Value::Null },
            ]),
            Message::new("s".to_string(), Role::User, vec![
                Part::ToolResult { tool_call_id: "t1".to_string(), content: "result".to_string(), duration_ms: None },
            ]),
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u2".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a2".to_string() }]),
        ];
        let safe = find_safe_split_point(&messages, 2);
        assert_eq!(safe, 2);
    }

    #[test]
    fn test_build_llm_messages_without_summary() {
        let state = LoopState::new(vec![
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u1".to_string() }]),
        ]);
        let msgs = build_llm_messages(&state);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_build_llm_messages_with_summary() {
        let mut state = LoopState::new(vec![
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u1".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a1".to_string() }]),
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u2".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a2".to_string() }]),
            Message::new("s".to_string(), Role::User, vec![Part::Text { text: "u3".to_string() }]),
            Message::new("s".to_string(), Role::Assistant, vec![Part::Text { text: "a3".to_string() }]),
        ]);
        state.compression_summary = Some(Message::new(
            "s".to_string(),
            Role::User,
            vec![Part::Text { text: "summary".to_string() }],
        ));
        let msgs = build_llm_messages(&state);
        assert_eq!(msgs.len(), 5); // summary + u2 + a2 + u3 + a3
    }
}
