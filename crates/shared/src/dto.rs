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
// 共享 DTO（Data Transfer Objects）：前后端 API 传输用的结构体
// =============================================================================
// 本模块集中管理所有在 HTTP API / SSE 流 / JSON-RPC 中序列化传输的结构体，
// 确保前后端对同一数据结构的定义完全一致。

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::enums::Role;

// -----------------------------------------------------------------------------
// Token 使用量统计
// -----------------------------------------------------------------------------

/// Token 使用量统计，用于 WaveMarker 等场景。
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

// -----------------------------------------------------------------------------
// 图片数据来源
// -----------------------------------------------------------------------------

/// 图片来源枚举，对应 Part::Image 的 source 字段。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// 本地文件系统路径
    Path { path: String },
    /// Base64 编码的图片数据
    Base64 { media_type: String, data: String },
    /// 远程图片 URL
    Url { url: String },
}

// -----------------------------------------------------------------------------
// 消息内容块（Part）：消息的原子组成单元
// -----------------------------------------------------------------------------

/// 内容块枚举：一条 `Message` 由多个 `Part` 按顺序组成。
///
/// 这种设计与 Anthropic / OpenAI 的最新内容块 API 对齐，
/// 支持纯文本、多模态图片、工具调用、工具结果、推理过程、
/// 波浪标记以及用量统计。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// 纯文本内容
    Text { text: String },
    /// 图片内容，支持本地路径、Base64 数据或远程 URL
    Image { source: ImageSource },
    /// 工具调用请求（由 Assistant 发起）
    ToolUse {
        id: String,
        name: String,
        /// 工具参数，使用 `serde_json::Value` 保持灵活性
        arguments: Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
        /// 工具执行耗时（毫秒），用于 TUI 展示性能信息
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        /// 工具结果元数据，用于前端展示额外信息（压缩状态、行数等）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        /// 仅用于上下文，不展示给用户（避免重复显示）
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        for_context_only: bool,
    },
    /// 工具执行错误（由 User 角色消息携带，回传给模型）
    ToolError {
        tool_call_id: String,
        content: String,
        error_message: String,
        /// 仅用于上下文，不展示给用户（避免重复显示）
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        for_context_only: bool,
    },
    /// 推理/思考过程（如 Claude Extended Thinking）
    Reasoning {
        thinking: String,
        /// 可选的签名，用于验证推理内容未被篡改
        signature: Option<String>,
    },
    /// 波浪标记，用于标识 Agent 执行步骤
    WaveMarker {
        step: u32,
        total: Option<u32>,
        git_snapshot: Option<String>,
        timestamp: u64,
        delta_tokens: TokenUsage,
    },
    /// 用量统计
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        latency_ms: u32,
        cost: Option<f64>,
    },
    /// 代码块内容，用于展示文件内容和 diff，支持语法高亮
    CodeBlock {
        language: String,
        code: String,
        /// 仅用于上下文，不展示给用户（避免重复显示）
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        for_context_only: bool,
    },
    /// 系统通知（如压缩完成、Agent 切换等）
    #[serde(rename = "system_notice")]
    SystemNotice { kind: String, content: String },
}

// -----------------------------------------------------------------------------
// 消息结构体（Message）
// -----------------------------------------------------------------------------

/// 对话消息结构体，用于在多轮对话中保存角色与内容块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Message {
    /// 便捷构造方法，自动生成 ULID id 与当前时间戳。
    pub fn new(session_id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            session_id: session_id.into(),
            role,
            created_at: current_timestamp_ms(),
            parts,
            token_count: None,
            cost: None,
        }
    }
}

/// 获取当前 Unix 时间戳（毫秒）。
/// 使用 `std::time::SystemTime` 避免引入额外依赖（如 chrono）。
pub fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ------------------------------------------------------------------------------
// Agent 类型枚举
// ------------------------------------------------------------------------------

/// Agent 类型：Build（全功能）和 Plan（只读规划）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AgentType {
    #[default]
    Build,
    Plan,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AgentType {
    /// 返回用于界面显示的名称（首字母大写），如 `"Build"`、`"Plan"`。
    /// 注意：这与序列化时的 wire format（snake_case）不同。
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Build => "Build",
            AgentType::Plan => "Plan",
        }
    }
}

// -----------------------------------------------------------------------------
// MessageBuilder：用于从持久化记录流式重建 Message
// -----------------------------------------------------------------------------

/// 消息构造器，在 `load_session` 过程中暂存一个 Message 的中间状态。
pub struct MessageBuilder {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
}

impl MessageBuilder {
    pub fn new(
        id: impl Into<String>,
        session_id: impl Into<String>,
        role: Role,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            session_id: session_id.into(),
            role,
            created_at,
            parts: Vec::new(),
        }
    }

    /// 向当前消息追加一个 Part。
    pub fn add_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    /// 完成消息构造，合并可选的 token_count 和 cost。
    pub fn finalize(self, token_count: Option<u64>, cost: Option<f64>) -> Message {
        Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            created_at: self.created_at,
            parts: self.parts,
            token_count,
            cost,
        }
    }
}

// -----------------------------------------------------------------------------
// SSE 事件类型
// -----------------------------------------------------------------------------

/// SSE 事件枚举，服务端通过 SSE 流向前端推送的各类事件。
/// 注意：序列化标签为 `"type"`（而非 `"event"`），与前端解析逻辑保持一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "message")]
    Message { content: String },
    #[serde(rename = "part")]
    Part { part: Part },
    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
    #[serde(rename = "error")]
    Error { message: String },
    /// Agent 信息变更事件，携带当前 Agent 类型与名称。
    /// 序列化标签为 `"agent_info"`，前端通过此事件更新界面状态。
    #[serde(rename = "agent_info")]
    AgentInfo {
        agent_type: AgentType,
        agent_name: String,
    },
    #[serde(rename = "done")]
    Done { session_id: String },
    /// 压缩状态更新事件
    #[serde(rename = "compression_status")]
    CompressionStatus {
        is_compressing: bool,
        progress: u8,
        context_ratio: u8,
        summary: Option<String>,
    },
    /// 权限确认请求事件（Ask 级别操作需要用户确认）
    #[serde(rename = "permission_ask")]
    PermissionAsk {
        tool_call_id: String,
        tool_name: String,
        risk: String,
        reason: String,
    },
    /// 用户问题询问事件（ask_for_question 工具触发）
    #[serde(rename = "question_ask")]
    QuestionAsk {
        tool_call_id: String,
        question: String,
        options: Vec<crate::tui_event::QuestionOption>,
        recommended: Option<String>,
        allow_custom: bool,
    },
}

/// 任务计划中的单个任务项。
/// 注意：`status` 使用 `String` 而非枚举，避免 shared crate 依赖 tools 模块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressItem {
    pub id: String,
    pub name: String,
    pub status: String,
}

// -----------------------------------------------------------------------------
// JSON-RPC 请求/响应
// -----------------------------------------------------------------------------

/// JSON-RPC 2.0 请求体。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 响应体。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 错误对象。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(code: i32, message: impl Into<String>, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

// -----------------------------------------------------------------------------
// 通用 API 响应包装
// -----------------------------------------------------------------------------

/// 通用 API 响应包装器，所有 JSON API 返回统一使用此结构。
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            code: None,
        }
    }

    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            code: Some(code.into()),
        }
    }
}

// -----------------------------------------------------------------------------
// TUI 共享类型
// -----------------------------------------------------------------------------

/// 主题预设，与 UI 框架无关的可序列化主题配置。
///
/// 颜色值使用 u32 存储（0xRRGGBB 格式），便于在不同模块间传递和通过 HTTP 序列化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePreset {
    pub name: String,
    pub description: String,
    pub bg_base: u32,
    pub bg_surface: u32,
    pub bg_overlay: u32,
    pub border: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_placeholder: u32,
    pub brand: u32,
    pub user: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
    pub selection_bg: u32,
    pub selection_fg: u32,
    pub accent_hover: u32,
}

/// 预设主题配置，编译时从 JSON 文件嵌入二进制。
const PRESET_THEMES_JSON: &str = include_str!("preset_themes.json");

impl ThemePreset {
    /// 返回所有内置主题预设。
    pub fn all_presets() -> Vec<Self> {
        serde_json::from_str(PRESET_THEMES_JSON)
            .expect("preset_themes.json 格式错误，必须是有效的 ThemePreset 数组")
    }
}

/// 文件树节点。
#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize, // 缩进深度，用于层级可视化
}

/// 命令元数据，用于 TUI 命令列表展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMeta {
    pub name: String,
    pub description: String,
    pub args_hint: Option<String>,
}

// -----------------------------------------------------------------------------
// Chat API 请求/响应 DTO
// -----------------------------------------------------------------------------

/// Chat 端点请求体。
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
    pub agent: Option<AgentType>,
}

/// 模型切换请求体。
#[derive(Deserialize)]
pub struct SwitchModelRequest {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
}

/// 模型切换响应。
#[derive(Serialize)]
pub struct SwitchModelResponse {
    pub provider: String,
    pub model: String,
}

// -----------------------------------------------------------------------------
// Session API DTO
// -----------------------------------------------------------------------------

/// Session 信息传输对象。
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool,
}

/// 创建 Session 请求。
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    #[serde(default)]
    pub template: String,
}

/// 重命名 Session 请求。
#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_default_is_build() {
        assert_eq!(AgentType::default(), AgentType::Build);
    }

    #[test]
    fn test_agent_type_serde_roundtrip() {
        let build = AgentType::Build;
        let json = serde_json::to_string(&build).unwrap();
        assert_eq!(json, "\"build\"");
        let decoded: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AgentType::Build);

        let plan = AgentType::Plan;
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(json, "\"plan\"");
        let decoded: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AgentType::Plan);
    }

    #[test]
    fn test_agent_type_as_str() {
        assert_eq!(AgentType::Build.as_str(), "Build");
        assert_eq!(AgentType::Plan.as_str(), "Plan");
    }

    #[test]
    fn test_agent_type_deserialize_invalid() {
        let result: Result<AgentType, _> = serde_json::from_str("\"unknown\"");
        assert!(result.is_err());
    }
}
