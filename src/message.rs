// =============================================================================
// message 模块：定义对话核心数据结构
// =============================================================================
// 本模块抽离了与消息内容相关的通用类型：
// - `Role`：消息发送者角色
// - `Part`：单条消息的内容块（文本、图片、工具调用、工具结果、推理）
// - `ImageSource`：图片数据来源
// - `Message`：包含完整元数据的消息结构
//
// 设计演进：这些类型原本位于 `agent/mod.rs` 中，但随着 `session` 持久化、
// `provider` 序列化、`tools` 工具结果等多个模块的依赖增加，
// 将其独立为顶层模块可降低耦合、避免循环依赖。

use serde::{Deserialize, Serialize};

// =============================================================================
// 角色枚举
// =============================================================================

/// 对话角色枚举。
/// - `User`：人类用户
/// - `Assistant`：AI 助手
/// - `System`：系统级提示（如环境描述）
/// - `Developer`：开发者消息（部分模型支持，如 Claude Code 风格）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

// =============================================================================
// 内容块枚举（Part）：消息的原子组成单元
// =============================================================================

/// 内容块枚举：一条 `Message` 由多个 `Part` 按顺序组成。
///
/// 这种设计与 Anthropic / OpenAI 的最新内容块 API 对齐，
/// 支持纯文本、多模态图片、工具调用、工具结果以及推理过程。
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
        arguments: serde_json::Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
    /// 推理/思考过程（如 Claude Extended Thinking）
    Reasoning {
        thinking: String,
        /// 可选的签名，用于验证推理内容未被篡改
        signature: Option<String>,
    },
}

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

// =============================================================================
// 消息结构体（Message）
// =============================================================================

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
