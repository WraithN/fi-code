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

// OpenTelemetry / Langfuse / fi-code 自定义属性键常量集合
// 命名空间约定：
// - langfuse.*      Langfuse 平台专有键
// - gen_ai.*        OTel GenAI semconv 标准键
// - fi_code.*       本项目自定义键

// ===== Langfuse trace 级别 =====
pub const LANGFUSE_USER_ID: &str = "langfuse.user.id";
pub const LANGFUSE_SESSION_ID: &str = "langfuse.session.id";
pub const LANGFUSE_TRACE_NAME: &str = "langfuse.trace.name";
pub const LANGFUSE_TRACE_TAGS: &str = "langfuse.trace.tags";
pub const LANGFUSE_TRACE_INPUT: &str = "langfuse.trace.input";
pub const LANGFUSE_TRACE_OUTPUT: &str = "langfuse.trace.output";
pub const LANGFUSE_TRACE_METADATA_PREFIX: &str = "langfuse.trace.metadata.";
pub const LANGFUSE_RELEASE: &str = "langfuse.release";
pub const LANGFUSE_ENVIRONMENT: &str = "langfuse.environment";

// ===== Langfuse observation 级别 =====
pub const LANGFUSE_OBS_TYPE: &str = "langfuse.observation.type";
pub const LANGFUSE_OBS_INPUT: &str = "langfuse.observation.input";
pub const LANGFUSE_OBS_OUTPUT: &str = "langfuse.observation.output";
pub const LANGFUSE_OBS_LEVEL: &str = "langfuse.observation.level";
pub const LANGFUSE_OBS_STATUS_MESSAGE: &str = "langfuse.observation.status_message";
pub const LANGFUSE_OBS_USAGE_DETAILS: &str = "langfuse.observation.usage_details";
pub const LANGFUSE_OBS_MODEL_NAME: &str = "langfuse.observation.model.name";

// ===== OTel GenAI 标准键 =====
pub const GEN_AI_SYSTEM: &str = "gen_ai.system";
pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";
pub const GEN_AI_RESPONSE_MODEL: &str = "gen_ai.response.model";
pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
pub const GEN_AI_USAGE_TOTAL_TOKENS: &str = "gen_ai.usage.total_tokens";
pub const GEN_AI_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";

// ===== fi-code 自定义键（统一以 fi_code. 为前缀，避免与 OTel/Langfuse 冲突）=====
pub const FI_TURN_INDEX: &str = "fi_code.turn.index";
pub const FI_TOOL_NAME: &str = "fi_code.tool.name";
pub const FI_TOOL_CALL_ID: &str = "fi_code.tool.call_id";
pub const FI_MESSAGES_SNAPSHOT: &str = "fi_code.messages_snapshot";
pub const FI_AGENT_TYPE: &str = "fi_code.agent.type";
pub const FI_TRANSITION_REASON: &str = "fi_code.transition_reason";
pub const FI_COMPRESSION_BEFORE: &str = "fi_code.compression.before_tokens";
pub const FI_COMPRESSION_AFTER: &str = "fi_code.compression.after_tokens";

// ===== Observation type 取值 =====
pub const OBS_TYPE_SPAN: &str = "span";
pub const OBS_TYPE_GENERATION: &str = "generation";
pub const OBS_TYPE_EVENT: &str = "event";

// ===== Observation level 取值 =====
pub const LEVEL_DEFAULT: &str = "DEFAULT";
pub const LEVEL_ERROR: &str = "ERROR";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_langfuse_key_naming_consistency() {
        // Langfuse trace 级常量必须以 langfuse.trace 或 langfuse.user / langfuse.session 等开头
        assert_eq!(LANGFUSE_USER_ID, "langfuse.user.id");
        assert_eq!(LANGFUSE_SESSION_ID, "langfuse.session.id");
        assert_eq!(LANGFUSE_TRACE_NAME, "langfuse.trace.name");
        assert_eq!(LANGFUSE_OBS_TYPE, "langfuse.observation.type");
        assert_eq!(
            LANGFUSE_OBS_USAGE_DETAILS,
            "langfuse.observation.usage_details"
        );
        // metadata 前缀末尾必须带点，便于业务侧拼接子键
        assert!(LANGFUSE_TRACE_METADATA_PREFIX.ends_with('.'));
    }

    #[test]
    fn test_gen_ai_semconv_keys() {
        // 严格遵循 OTel GenAI semconv 命名（snake_case + 分组前缀）
        assert_eq!(GEN_AI_SYSTEM, "gen_ai.system");
        assert_eq!(GEN_AI_REQUEST_MODEL, "gen_ai.request.model");
        assert_eq!(GEN_AI_RESPONSE_MODEL, "gen_ai.response.model");
        assert_eq!(GEN_AI_USAGE_INPUT_TOKENS, "gen_ai.usage.input_tokens");
        assert_eq!(GEN_AI_USAGE_OUTPUT_TOKENS, "gen_ai.usage.output_tokens");
        assert_eq!(GEN_AI_USAGE_TOTAL_TOKENS, "gen_ai.usage.total_tokens");
        assert_eq!(
            GEN_AI_RESPONSE_FINISH_REASONS,
            "gen_ai.response.finish_reasons"
        );
    }

    #[test]
    fn test_fi_code_namespace_prefix() {
        // 所有 fi-code 自定义键必须以 fi_code. 开头，避免命名空间污染
        let fi_keys = [
            FI_TURN_INDEX,
            FI_TOOL_NAME,
            FI_TOOL_CALL_ID,
            FI_MESSAGES_SNAPSHOT,
            FI_AGENT_TYPE,
            FI_TRANSITION_REASON,
            FI_COMPRESSION_BEFORE,
            FI_COMPRESSION_AFTER,
        ];
        for k in fi_keys.iter() {
            assert!(
                k.starts_with("fi_code."),
                "fi-code 自定义键 {} 必须以 fi_code. 为前缀",
                k
            );
        }
        // observation type / level 取值常量
        assert_eq!(OBS_TYPE_SPAN, "span");
        assert_eq!(OBS_TYPE_GENERATION, "generation");
        assert_eq!(OBS_TYPE_EVENT, "event");
        assert_eq!(LEVEL_DEFAULT, "DEFAULT");
        assert_eq!(LEVEL_ERROR, "ERROR");
    }
}
