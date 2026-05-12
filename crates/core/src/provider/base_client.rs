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

use crate::log_info;
use crate::log_trace;
use crate::log_warn;
use crate::session::message::Part;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

// =============================================================================
// 统一的停止原因枚举：兼容 OpenAI 与 Anthropic 的不同标准
// =============================================================================

/// 模型生成停止的统一抽象。
/// OpenAI 使用 `finish_reason` 字段（如 `stop`、`tool_calls`），
/// Anthropic 使用 `stop_reason` 字段（如 `end_turn`、`tool_use`）。
/// 本枚举将两者映射到同一语义层，方便上层业务统一判断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// 自然结束对话回合（OpenAI: `stop`，Anthropic: `end_turn`)
    Stop,
    /// 达到最大 token 限制（OpenAI: `length`，Anthropic: `max_tokens`)
    Length,
    /// 因为需要调用工具而停止（OpenAI: `tool_calls`，Anthropic: `tool_use`)
    ToolUse,
    /// 命中预设的停止序列（Anthropic: `stop_sequence`)
    StopSequence,
    /// 内容被安全过滤拦截（OpenAI: `content_filter`)
    ContentFilter,
    /// 其他未知或未标准化的原因
    Other(String),
}

impl FinishReason {
    /// 从 OpenAI 的 `finish_reason` 字符串转换为统一枚举
    pub fn from_openai(reason: &str) -> Self {
        match reason {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "tool_calls" => FinishReason::ToolUse,
            "content_filter" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_string()),
        }
    }

    /// 从 Anthropic 的 `stop_reason` 字符串转换为统一枚举
    pub fn from_anthropic(reason: &str) -> Self {
        match reason {
            "end_turn" => FinishReason::Stop,
            "max_tokens" => FinishReason::Length,
            "tool_use" => FinishReason::ToolUse,
            "stop_sequence" => FinishReason::StopSequence,
            other => FinishReason::Other(other.to_string()),
        }
    }
}

// =============================================================================
// 流式回调单元：客户端通过闭包将解析后的消息片段实时回传上层
// =============================================================================

/// 闭包接收到的单个消息单元内容。
/// - `Text` / `Think`：增量片段，由上层自行聚合。
/// - `ToolUse`：客户端已在内部拼装为完整的工具调用块，直接可用。
///   类型已随 Session 设计升级为 `Part::ToolUse`。
/// - `Finish`：流结束，携带统一的停止原因。
#[derive(Debug, Clone)]
pub enum ChunkContent {
    Text(String),
    Think(String),
    ToolUse(Part),
    Usage(TokenUsage),
    Finish(FinishReason),
}

/// Token 使用量统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// 闭包参数结构体，包装单次回调的内容。
#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: ChunkContent,
}

// =============================================================================
// 聚合后的完整响应结构
// =============================================================================

/// 将流式消息聚合后得到的完整响应。
/// `content` 为按顺序排列的内容块（文本 + 思考 + 工具调用），
/// `finish_reason` 为模型停止的统一原因。
#[derive(Debug)]
pub struct ApiResponse {
    pub content: Vec<Part>,
    pub finish_reason: Option<FinishReason>,
}

/// 从内容块列表中提取所有文本并按换行拼接。
/// Reasoning 块默认不暴露给终端输出，因此不被提取。
pub fn extract_text(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|block| match block {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// =============================================================================
// HTTP 重试装饰器：指数退避 + Full Jitter
// =============================================================================

/// 重试策略配置。
/// 当遇到可重试的网络错误或 HTTP 状态码时，
/// 按照 `base_delay * 2^attempt` 计算退避时间，并叠加 [0, delay) 范围内的随机 jitter。
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数（不包含首次请求）
    pub max_retries: u32,
    /// 首次重试的基础延迟
    pub base_delay: Duration,
    /// 延迟上限，防止无限增长
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

/// 判断 HTTP 状态码是否属于可重试场景。
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 502 | 503 | 504)
}

/// 判断 reqwest 错误是否属于可重试场景。
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout()
}

/// 计算第 `attempt` 次重试的退避时间（0-based）。
/// 使用 Full Jitter 策略：在 `[0, min(base*2^attempt, max_delay))` 内随机取值。
fn compute_backoff(attempt: u32, base: Duration, max: Duration) -> Duration {
    let exp = std::cmp::min(attempt, 6); // 防止指数溢出，最大 2^6 = 64
    let delay = base.saturating_mul(2_u32.pow(exp));
    let capped = std::cmp::min(delay, max);

    if capped.is_zero() {
        return Duration::ZERO;
    }

    let millis = rand::random::<u64>() % (capped.as_millis() as u64).max(1);
    Duration::from_millis(millis)
}

/// 发送 HTTP 请求并在遇到可重试错误时执行指数退避 + jitter 重试。
///
/// # 注意
/// - `request` 必须支持 `try_clone()`。对于基于 `String` / `Bytes` 的 JSON body，
///   reqwest 通常可以成功克隆；若 body 是不可克隆的流，则会在首次重试时返回错误。
/// - 对于非成功但不可重试的 HTTP 响应，直接原样返回，由调用方处理错误 body。
async fn do_retry_backoff(attempt: u32, config: &RetryConfig, context: &str, detail: &str) {
    let backoff = compute_backoff(attempt, config.base_delay, config.max_delay);
    log_trace!(
        "[Server] send_with_retry | attempt={} | {} | backoff={:?}",
        attempt + 1,
        context,
        backoff
    );
    log_warn!(
        "[Server] HTTP retry | {} (attempt {}/{}), retry in {:?}: {}",
        context,
        attempt + 1,
        config.max_retries,
        backoff,
        detail
    );
    tokio::time::sleep(backoff).await;
}

pub async fn send_with_retry(
    client: &reqwest::Client,
    request: reqwest::Request,
    config: &RetryConfig,
) -> Result<reqwest::Response> {
    let mut attempt = 0u32;

    loop {
        let req = request
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Request body is not cloneable, cannot retry"))?;

        let resp = match client.execute(req).await {
            Ok(r) => r,
            Err(err) => {
                if !is_retryable_error(&err) || attempt >= config.max_retries {
                    return Err(err.into());
                }
                do_retry_backoff(attempt, config, "network error", &err.to_string()).await;
                attempt += 1;
                continue;
            }
        };

        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if !is_retryable_status(status) || attempt >= config.max_retries {
            return Ok(resp);
        }
        let text = resp.text().await.unwrap_or_default();
        do_retry_backoff(attempt, config, &format!("HTTP {}", status), &text).await;
        attempt += 1;
    }
}

// =============================================================================
// AI 客户端统一 Trait
// =============================================================================

#[async_trait]
pub trait AIClient: Send + Sync {
    /// 发起流式对话请求。
    /// 具体实现负责解析厂商原生的 SSE 事件：
    /// - 普通文本/思考增量直接通过 `on_chunk` 回传；
    /// - tool_use 必须在内部拼装完整后再以 `ChunkContent::ToolUse` 形式回传；
    /// - 流结束时通过 `ChunkContent::Finish` 回传停止原因。
    async fn stream_message(
        &self,
        system_prompt: &str,
        messages: &[crate::session::message::Message],
        tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()>;
}
