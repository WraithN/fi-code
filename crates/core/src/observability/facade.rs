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

//! Facade：业务层与 OTel 之间的唯一接口。
//!
//! 设计要点：
//! - opentelemetry 0.27 的 `BoxedSpan` 不实现 Clone，因此 guard 结构只持有
//!   `Context`（Clone-friendly），所有 span 操作走 `cx.span()` 拿 `SpanRef`。
//! - 每种 guard 在 Drop 时自动调用 `span.end()`，业务层不需要手动结束。
//! - 输入/输出文本统一经过 `redact::redact_and_truncate` 脱敏 + 截断。
//! - parent context 通过 `Tracer::build_with_context` 传递，确保 trace_id 传播。

use opentelemetry::global::BoxedSpan;
use opentelemetry::trace::{Span, SpanBuilder, SpanKind, Status, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use serde_json::json;

use fi_code_shared::dto::AgentType;

use crate::observability::attrs::{
    FI_AGENT_TYPE, FI_COMPRESSION_AFTER, FI_COMPRESSION_BEFORE, FI_TOOL_CALL_ID, FI_TOOL_NAME,
    FI_TRANSITION_REASON, FI_TURN_INDEX, GEN_AI_REQUEST_MODEL, GEN_AI_RESPONSE_FINISH_REASONS,
    GEN_AI_SYSTEM, GEN_AI_USAGE_INPUT_TOKENS, GEN_AI_USAGE_OUTPUT_TOKENS,
    GEN_AI_USAGE_TOTAL_TOKENS, LANGFUSE_OBS_INPUT, LANGFUSE_OBS_LEVEL, LANGFUSE_OBS_MODEL_NAME,
    LANGFUSE_OBS_OUTPUT, LANGFUSE_OBS_TYPE, LANGFUSE_OBS_USAGE_DETAILS, LANGFUSE_SESSION_ID,
    LANGFUSE_TRACE_NAME, LANGFUSE_TRACE_TAGS, LANGFUSE_USER_ID, LEVEL_DEFAULT, LEVEL_ERROR,
    OBS_TYPE_GENERATION, OBS_TYPE_SPAN,
};
use crate::observability::redact::redact_and_truncate;

// ===== 常量集中：禁止魔法值（AGENTS.md §6.11）=====

/// instrumentation scope 名称（用于 `global::tracer(name)`）。
const INSTRUMENTATION_NAME: &str = "fi-code";

/// 当前固定 user.id 为 "local"，单机版无多租户概念。
const DEFAULT_USER_ID: &str = "local";

// Span 名称
const SPAN_NAME_CHAT: &str = "chat.request";
const SPAN_NAME_TURN: &str = "agent.turn";
const SPAN_NAME_LLM: &str = "llm.generation";
const SPAN_NAME_COMPRESSION: &str = "agent.compression";

// Event 名称
const EVENT_NAME_PERMISSION_ASK: &str = "permission_ask";

// Permission event 属性键
const PERM_EVT_ACTION: &str = "action";
const PERM_EVT_APPROVED: &str = "approved";
const PERM_EVT_DURATION_MS: &str = "duration_ms";

// Usage details JSON 字段
const USAGE_KEY_INPUT: &str = "input";
const USAGE_KEY_OUTPUT: &str = "output";
const USAGE_KEY_TOTAL: &str = "total";

// 工具 span 名称前缀，最终拼成 `tool.<name>`
const TOOL_SPAN_PREFIX: &str = "tool.";

/// 拿到全局 tracer。
fn tracer() -> opentelemetry::global::BoxedTracer {
    global::tracer(INSTRUMENTATION_NAME)
}

/// 统一脱敏入口，缩短调用点写法。
fn redacted(s: &str) -> String {
    redact_and_truncate(s)
}

/// 根据可选 parent 构造 BoxedSpan，统一处理"有/无父上下文"两种分支。
fn start_span_with_parent(builder: SpanBuilder, parent: Option<&Context>) -> BoxedSpan {
    let tr = tracer();
    match parent {
        Some(cx) => tr.build_with_context(builder, cx),
        None => tr.build(builder),
    }
}

// =====================================================================
// ChatSpan：一次完整请求的根 span（trace 入口）。
// =====================================================================

/// 顶层 ChatSpan guard：持有 Context，Drop 时自动 end。
pub struct ChatSpan {
    cx: Context,
}

impl ChatSpan {
    /// 返回内部 Context（用于子 span 父节点传递）。
    pub fn context(&self) -> Context {
        self.cx.clone()
    }

    /// 返回当前 span 的 trace_id 字符串（用于日志关联）。
    pub fn trace_id(&self) -> String {
        self.cx.span().span_context().trace_id().to_string()
    }

    /// 设置最终输出（脱敏 + 截断）。
    pub fn set_output(&self, text: &str) {
        self.cx
            .span()
            .set_attribute(KeyValue::new(LANGFUSE_OBS_OUTPUT, redacted(text)));
    }

    /// 设置 trace 级标签。Langfuse 期望逗号分隔字符串。
    pub fn set_tags(&self, tags: &[&str]) {
        let joined = tags.join(",");
        self.cx
            .span()
            .set_attribute(KeyValue::new(LANGFUSE_TRACE_TAGS, joined));
    }

    /// 标记错误：写 status + level=ERROR。
    pub fn record_error(&self, msg: &str) {
        self.cx.span().set_status(Status::error(msg.to_string()));
        self.cx
            .span()
            .set_attribute(KeyValue::new(LANGFUSE_OBS_LEVEL, LEVEL_ERROR));
    }
}

impl Drop for ChatSpan {
    fn drop(&mut self) {
        self.cx.span().end();
    }
}

/// 启动 ChatSpan，写入 user/session/trace.name/input 等 Langfuse 关键属性。
pub fn start_chat_span(session_id: &str, user_message: &str, agent_type: AgentType) -> ChatSpan {
    let tr = tracer();
    let span = tr
        .span_builder(SPAN_NAME_CHAT)
        .with_kind(SpanKind::Server)
        .with_attributes(vec![
            KeyValue::new(LANGFUSE_USER_ID, DEFAULT_USER_ID),
            KeyValue::new(LANGFUSE_SESSION_ID, session_id.to_string()),
            KeyValue::new(LANGFUSE_TRACE_NAME, SPAN_NAME_CHAT),
            KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN),
            KeyValue::new(LANGFUSE_OBS_INPUT, redacted(user_message)),
            KeyValue::new(FI_AGENT_TYPE, format!("{:?}", agent_type)),
        ])
        .start(&tr);
    ChatSpan {
        cx: Context::current_with_span(span),
    }
}

// =====================================================================
// TurnSpan：每个 agent 回合（含 LLM + 工具调用序列）。
// =====================================================================

/// 单回合 guard。
pub struct TurnSpan {
    cx: Context,
}

impl TurnSpan {
    /// 返回内部 Context（用于子 span 父节点传递）。
    pub fn context(&self) -> Context {
        self.cx.clone()
    }

    /// 记录本回合的状态迁移原因（如 "tool_call" / "compression" / "final"）。
    pub fn set_transition_reason(&self, reason: &str) {
        self.cx
            .span()
            .set_attribute(KeyValue::new(FI_TRANSITION_REASON, reason.to_string()));
    }
}

impl Drop for TurnSpan {
    fn drop(&mut self) {
        self.cx.span().end();
    }
}

/// 启动 TurnSpan；必须传 parent（chat span 或上一轮 turn）。
pub fn start_turn_span(parent: Option<&Context>, turn_index: usize) -> TurnSpan {
    let builder = tracer().span_builder(SPAN_NAME_TURN).with_attributes(vec![
        KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN),
        KeyValue::new(FI_TURN_INDEX, turn_index as i64),
    ]);
    let span = start_span_with_parent(builder, parent);
    TurnSpan {
        cx: Context::current_with_span(span),
    }
}

// =====================================================================
// LlmGeneration：一次 LLM 请求-响应（Langfuse "generation" observation）。
// =====================================================================

/// LLM 调用 guard。
pub struct LlmGeneration {
    cx: Context,
}

impl LlmGeneration {
    /// 返回内部 Context。
    pub fn context(&self) -> Context {
        self.cx.clone()
    }

    /// 记录补全输出（脱敏 + 截断）。
    pub fn record_output(&self, completion: &str) {
        self.cx
            .span()
            .set_attribute(KeyValue::new(LANGFUSE_OBS_OUTPUT, redacted(completion)));
    }

    /// 记录 token 用量：拆分写 OTel GenAI semconv 三键 + Langfuse usage_details JSON。
    pub fn record_usage(&self, in_tok: u32, out_tok: u32, total_tok: u32) {
        let sp = self.cx.span();
        sp.set_attribute(KeyValue::new(GEN_AI_USAGE_INPUT_TOKENS, in_tok as i64));
        sp.set_attribute(KeyValue::new(GEN_AI_USAGE_OUTPUT_TOKENS, out_tok as i64));
        sp.set_attribute(KeyValue::new(GEN_AI_USAGE_TOTAL_TOKENS, total_tok as i64));
        // Langfuse 期望 usage_details 是 JSON 字符串。
        let details = json!({
            USAGE_KEY_INPUT: in_tok,
            USAGE_KEY_OUTPUT: out_tok,
            USAGE_KEY_TOTAL: total_tok,
        })
        .to_string();
        sp.set_attribute(KeyValue::new(LANGFUSE_OBS_USAGE_DETAILS, details));
    }

    /// 记录 finish_reason（如 "stop" / "tool_calls" / "length"）。
    pub fn record_finish_reason(&self, reason: &str) {
        self.cx.span().set_attribute(KeyValue::new(
            GEN_AI_RESPONSE_FINISH_REASONS,
            reason.to_string(),
        ));
    }
}

impl Drop for LlmGeneration {
    fn drop(&mut self) {
        self.cx.span().end();
    }
}

/// 启动 LlmGeneration；observation.type = generation 让 Langfuse 识别为模型调用。
pub fn start_llm_generation(
    parent: Option<&Context>,
    model: &str,
    provider: &str,
    messages_json: &str,
) -> LlmGeneration {
    let builder = tracer().span_builder(SPAN_NAME_LLM).with_attributes(vec![
        KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_GENERATION),
        KeyValue::new(GEN_AI_SYSTEM, provider.to_string()),
        KeyValue::new(GEN_AI_REQUEST_MODEL, model.to_string()),
        KeyValue::new(LANGFUSE_OBS_INPUT, redacted(messages_json)),
        KeyValue::new(LANGFUSE_OBS_MODEL_NAME, model.to_string()),
    ]);
    let span = start_span_with_parent(builder, parent);
    LlmGeneration {
        cx: Context::current_with_span(span),
    }
}

// =====================================================================
// ToolSpan：一次工具调用。
// =====================================================================

/// 工具调用 guard。
pub struct ToolSpan {
    cx: Context,
}

impl ToolSpan {
    /// 返回内部 Context。
    pub fn context(&self) -> Context {
        self.cx.clone()
    }

    /// 记录结果：写 output（脱敏）+ level（成功 DEFAULT / 失败 ERROR）。
    pub fn record_result(&self, output: &str, is_error: bool) {
        let sp = self.cx.span();
        sp.set_attribute(KeyValue::new(LANGFUSE_OBS_OUTPUT, redacted(output)));
        let level = if is_error { LEVEL_ERROR } else { LEVEL_DEFAULT };
        sp.set_attribute(KeyValue::new(LANGFUSE_OBS_LEVEL, level));
    }

    /// 追加 permission_ask 事件：记录权限询问的动作、是否同意、耗时。
    pub fn add_permission_event(&self, action: &str, approved: bool, duration_ms: u64) {
        self.cx.span().add_event(
            EVENT_NAME_PERMISSION_ASK,
            vec![
                KeyValue::new(PERM_EVT_ACTION, action.to_string()),
                KeyValue::new(PERM_EVT_APPROVED, approved),
                KeyValue::new(PERM_EVT_DURATION_MS, duration_ms as i64),
            ],
        );
    }
}

impl Drop for ToolSpan {
    fn drop(&mut self) {
        self.cx.span().end();
    }
}

/// 启动 ToolSpan；span 名为 `tool.<tool_name>`。
pub fn start_tool_span(
    parent: Option<&Context>,
    tool_name: &str,
    tool_call_id: &str,
    args_json: &str,
) -> ToolSpan {
    let name = format!("{}{}", TOOL_SPAN_PREFIX, tool_name);
    let builder = tracer().span_builder(name).with_attributes(vec![
        KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN),
        KeyValue::new(FI_TOOL_NAME, tool_name.to_string()),
        KeyValue::new(FI_TOOL_CALL_ID, tool_call_id.to_string()),
        KeyValue::new(LANGFUSE_OBS_INPUT, redacted(args_json)),
    ]);
    let span = start_span_with_parent(builder, parent);
    ToolSpan {
        cx: Context::current_with_span(span),
    }
}

// =====================================================================
// CompressionSpan：上下文压缩动作。
// =====================================================================

/// 压缩 guard。
pub struct CompressionSpan {
    cx: Context,
}

impl CompressionSpan {
    /// 返回内部 Context。
    pub fn context(&self) -> Context {
        self.cx.clone()
    }

    /// 记录压缩前后的 token 数。
    pub fn record_ratio(&self, before_tokens: u32, after_tokens: u32) {
        let sp = self.cx.span();
        sp.set_attribute(KeyValue::new(FI_COMPRESSION_BEFORE, before_tokens as i64));
        sp.set_attribute(KeyValue::new(FI_COMPRESSION_AFTER, after_tokens as i64));
    }
}

impl Drop for CompressionSpan {
    fn drop(&mut self) {
        self.cx.span().end();
    }
}

/// 启动 CompressionSpan。
pub fn start_compression_span(parent: Option<&Context>) -> CompressionSpan {
    let builder = tracer()
        .span_builder(SPAN_NAME_COMPRESSION)
        .with_attributes(vec![KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN)]);
    let span = start_span_with_parent(builder, parent);
    CompressionSpan {
        cx: Context::current_with_span(span),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建并 drop ChatSpan：覆盖最常见生命周期，不允许 panic。
    #[test]
    fn test_chat_span_drop_does_not_panic() {
        let span = start_chat_span("test-session-1", "hello", AgentType::Build);
        span.set_output("hi there");
        // span 在作用域结束时 drop，触发 end()。
        drop(span);
    }

    /// 含敏感凭据的 input 应能正常完成 redact + 设置，不允许 panic。
    /// 真实的属性内容验证留给集成测试（需要 in-memory exporter）。
    #[test]
    fn test_redaction_applied_in_chat_input() {
        let user_msg = "my key is sk-test1234567890abcdefghij please redact";
        let span = start_chat_span("sess-redact", user_msg, AgentType::Plan);
        // 触发 set_output 路径也走 redact。
        span.set_output("API: sk-test1234567890abcdefghij");
        // 简单 sanity check：trace_id 是 32 位十六进制字符串。
        let tid = span.trace_id();
        assert_eq!(tid.len(), 32, "trace_id 应为 32 位 hex，实际：{}", tid);
    }

    /// 子 span 通过 Context::current_with_span 派生 → trace_id 必须一致。
    #[test]
    fn test_child_span_parent_propagation() {
        let chat = start_chat_span("sess-prop", "msg", AgentType::Build);
        let chat_tid = chat.trace_id();
        let chat_cx = chat.context();

        let turn = start_turn_span(Some(&chat_cx), 0);
        let turn_tid = turn.context().span().span_context().trace_id().to_string();

        assert_eq!(
            chat_tid, turn_tid,
            "子 turn span 必须复用父 chat 的 trace_id"
        );
    }
}
