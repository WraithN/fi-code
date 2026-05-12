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
// MockAIClient：用于 E2E 测试的模拟 LLM 客户端
// =============================================================================
// 根据输入消息内容返回预定义的响应，不依赖真实 LLM API。
// 支持模拟文本回复、工具调用（write、handle_task_plan 等）。

use async_trait::async_trait;

use crate::provider::base_client::{AIClient, Chunk, ChunkContent, FinishReason};
use crate::session::message::{Message, Part};
use anyhow::Result;

/// 模拟 AI 客户端，用于端到端测试。
///
/// 响应策略基于最后一条用户消息的内容关键词匹配：
/// - 问候语（"你好"/"你是谁"）→ 返回问候文本
/// - 代码任务（"写代码"/"创建文件"）→ 返回 write 工具调用
/// - 复杂任务（"复杂"/"拆分任务"）→ 返回 handle_task_plan 工具调用
/// - 其他/子任务 → 返回简单文本
pub struct MockAIClient;

impl MockAIClient {
    pub fn new() -> Self {
        Self
    }
}

/// 从消息列表中提取最后一条文本内容。
fn extract_last_text(messages: &[Message]) -> String {
    messages
        .last()
        .and_then(|m| m.parts.last())
        .and_then(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
        .to_lowercase()
}

/// 判断消息类型，决定 Mock 响应策略。
enum MessageType {
    Greeting,
    CodeTask,
    ReadTask,
    BashTask,
    EditTask,
    SkillTask,
    InitTask,
    ComplexTask,
    Subtask,
}

fn classify_message(text: &str) -> MessageType {
    if text.contains("你好") || text.contains("你是谁") || text.contains("hi") {
        MessageType::Greeting
    } else if text.contains("复杂") || text.contains("拆分任务") || text.contains("任务计划")
        || text.contains("设计并实现") || text.contains("注定失败")
    {
        MessageType::ComplexTask
    } else if text.contains("写") && (text.contains("代码") || text.contains("文件")) {
        MessageType::CodeTask
    } else if text.contains("读取") || text.contains("read") {
        MessageType::ReadTask
    } else if text.contains("运行") || text.contains("bash") || text.contains("命令") || text.contains("ls ") {
        MessageType::BashTask
    } else if text.contains("编辑") || text.contains("修改") || text.contains("edit") || text.contains("添加") {
        MessageType::EditTask
    } else if text.contains("技能") || text.contains("skill") || text.contains("提交") || text.contains("审查") {
        MessageType::SkillTask
    } else if text.contains("/init") {
        MessageType::InitTask
    } else {
        MessageType::Subtask
    }
}

/// 辅助函数：发送文本 chunk。
fn send_text(on_chunk: &mut (dyn FnMut(Chunk) + Send), text: &str) {
    // 模拟流式输出：将文本按字符分批发送
    for chunk in text.chars().collect::<Vec<_>>().chunks(10) {
        let s: String = chunk.iter().collect();
        on_chunk(Chunk {
            content: ChunkContent::Text(s),
        });
    }
}

/// 辅助函数：发送 ToolUse chunk。
fn send_tool_use(
    on_chunk: &mut (dyn FnMut(Chunk) + Send),
    id: &str,
    name: &str,
    arguments: serde_json::Value,
) {
    on_chunk(Chunk {
        content: ChunkContent::ToolUse(Part::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
        }),
    });
}

/// 辅助函数：发送 Finish chunk。
fn send_finish(on_chunk: &mut (dyn FnMut(Chunk) + Send), reason: FinishReason) {
    on_chunk(Chunk {
        content: ChunkContent::Finish(reason),
    });
}

#[async_trait]
impl AIClient for MockAIClient {
    async fn stream_message(
        &self,
        _system_prompt: &str,
        messages: &[Message],
        _tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()> {
        let last_text = extract_last_text(messages);

        match classify_message(&last_text) {
            MessageType::Greeting => {
                send_text(
                    on_chunk,
                    "你好！我是 FiCode，一个智能编程助手。我可以帮你写代码、调试程序、回答问题等。",
                );
                send_finish(on_chunk, FinishReason::Stop);
            }

            MessageType::CodeTask => {
                send_text(
                    on_chunk,
                    "好的，文件写入成功。",
                );
                send_tool_use(
                    on_chunk,
                    "write_1",
                    "write",
                    serde_json::json!({
                        "path": "test_output/hello.rs",
                        "content": "fn main() {\n    println!(\"Hello, World!\");\n}\n"
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::ComplexTask => {
                send_text(
                    on_chunk,
                    "我来帮你拆分这个复杂任务，制定执行计划。",
                );
                send_tool_use(
                    on_chunk,
                    "plan_1",
                    "handle_task_plan",
                    serde_json::json!({
                        "tasks": [
                            {"name": "分析需求", "description": "理解用户的核心需求"},
                            {"name": "设计方案", "description": "设计实现方案"},
                            {"name": "编写代码", "description": "编写核心代码"}
                        ]
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::ReadTask => {
                send_text(on_chunk, "我来读取文件内容。");
                send_tool_use(
                    on_chunk,
                    "read_1",
                    "read",
                    serde_json::json!({
                        "path": "test.txt"
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::BashTask => {
                send_text(on_chunk, "我来执行命令。");
                send_tool_use(
                    on_chunk,
                    "bash_1",
                    "bash",
                    serde_json::json!({
                        "command": "ls -la"
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::EditTask => {
                send_text(on_chunk, "我来修改文件。");
                send_tool_use(
                    on_chunk,
                    "edit_1",
                    "edit",
                    serde_json::json!({
                        "path": "main.rs",
                        "old_text": "fn main() {}",
                        "new_text": "fn main() {\n    println!(\"Hello!\");\n}"
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::SkillTask => {
                let skill_name = if last_text.contains("审查") || last_text.contains("code-review") {
                    "code-review"
                } else {
                    "commit"
                };
                send_text(on_chunk, "我来加载技能辅助完成，生成提交信息。");
                send_tool_use(
                    on_chunk,
                    "skill_1",
                    "use_skill",
                    serde_json::json!({
                        "name": skill_name
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::InitTask => {
                send_text(on_chunk, "好的，我来初始化项目，创建 AGENTS.md 文件。");
                send_tool_use(
                    on_chunk,
                    "write_1",
                    "write",
                    serde_json::json!({
                        "path": "AGENTS.md",
                        "content": "# Project Agents\n\nThis project uses fi-code agent.\n"
                    }),
                );
                send_finish(on_chunk, FinishReason::ToolUse);
            }

            MessageType::Subtask => {
                send_text(on_chunk, "子任务已完成，结果符合预期。");
                send_finish(on_chunk, FinishReason::Stop);
            }
        }

        Ok(())
    }
}
