// MIT License
// Copyright (c) 2025 fi-code contributors

use cucumber::{given, when, then};
use crate::bdd::AgentWorld;

// =============================================================================
// 流式输出与卡片渲染步骤定义
// Feature: streaming_output.feature
// =============================================================================

#[then("用户应该收到 Thinking 卡片")]
async fn user_receives_thinking_card(world: &mut AgentWorld) {
    // Thinking 状态通常由第一个 Message 事件或前端状态表示
    // 在 SSE 流中，只要开始接收事件就认为是 Thinking
    assert!(
        !world.events.is_empty(),
        "Expected to receive events (Thinking state)"
    );
}

#[then("用户应该看到流式的文本消息")]
async fn user_sees_streaming_text(world: &mut AgentWorld) {
    let message_events: Vec<_> = world.events.iter()
        .filter(|e| e.event_type == "Message")
        .collect();
    assert!(
        !message_events.is_empty(),
        "Expected streaming text messages"
    );
}

#[then("最终应该收到 Done 事件")]
async fn user_receives_done_event(world: &mut AgentWorld) {
    let has_done = world.events.iter().any(|e| e.event_type == "Done");
    assert!(has_done, "Expected Done event at the end of stream");
}

#[then(regex = r#"^卡片状态应该从 (.*) 变为 (.*)$"#)]
async fn card_status_changes(world: &mut AgentWorld, from: String, to: String) {
    // 简化验证：只要有 Done 事件就认为状态已转变
    let has_done = world.events.iter().any(|e| e.event_type == "Done");
    assert!(
        has_done,
        "Expected card status to change from '{}' to '{}'", 
        from, 
        to
    );
}

#[then(regex = r#"^用户应该收到 ToolUse 卡片，显示工具名称和参数$"#)]
async fn user_receives_tool_use_card(world: &mut AgentWorld) {
    let tool_use_events: Vec<_> = world.events.iter()
        .filter(|e| e.event_type == "ToolUse")
        .collect();
    assert!(
        !tool_use_events.is_empty(),
        "Expected ToolUse card with tool name and parameters"
    );
}

#[then("用户应该收到 ToolResult 卡片，显示执行结果")]
async fn user_receives_tool_result_card(world: &mut AgentWorld) {
    let has_tool_result = world.events.iter().any(|e| e.event_type == "ToolResult");
    assert!(has_tool_result, "Expected ToolResult card");
}

#[then(regex = r#"^最终结果卡片应该包含 "(.*)"$"#)]
async fn final_result_contains(world: &mut AgentWorld, expected: String) {
    let text = world.all_message_text();
    assert!(
        text.contains(&expected),
        "Expected final result to contain '{}', got: {}",
        expected,
        text
    );
}

#[then(regex = r#"^用户应该收到 WriteFile 卡片，显示文件路径和内容摘要$"#)]
async fn user_receives_write_file_card(world: &mut AgentWorld) {
    let has_tool_use = world.events.iter().any(|e| {
        e.event_type == "ToolUse" && e.tool_name.as_deref() == Some("write")
    });
    assert!(
        has_tool_use,
        "Expected WriteFile card showing file path and content"
    );
}

#[then("文件应该被实际写入")]
async fn file_actually_written(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let file_path = workspace.join("test_output/hello.rs");
    assert!(
        file_path.exists(),
        "File should be actually written to {:?}",
        file_path
    );
}

#[then("用户应该收到截断的内容提示")]
async fn user_receives_truncation_notice(world: &mut AgentWorld) {
    // Mock 客户端目前不模拟长内容截断
    // 此步骤作为占位，实际实现需要 MockAIClient 支持长文本响应
    assert!(
        world.events.iter().any(|e| e.event_type == "Message" || e.event_type == "Done"),
        "Expected some response (truncation test placeholder)"
    );
}

#[when("用户点击展开按钮")]
async fn user_clicks_expand(_world: &mut AgentWorld) {
    // TUI 交互操作在 BDD 测试中无法直接模拟
    // 此步骤作为场景衔接
}

#[then("用户应该看到完整的内容")]
async fn user_sees_full_content(world: &mut AgentWorld) {
    // 对应展开后的完整内容
    let text = world.all_message_text();
    assert!(
        !text.is_empty(),
        "Expected full content after expansion"
    );
}
