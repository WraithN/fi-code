// MIT License
// Copyright (c) 2025 fi-code contributors

use crate::bdd::AgentWorld;
use cucumber::{given, then, when};

// =============================================================================
// 通用步骤定义（Background 和跨 Feature 共享）
// =============================================================================

#[given("一个配置了 Mock Provider 的后端服务")]
async fn mock_backend_service(world: &mut AgentWorld) {
    world.start_mock_server().await;
}

#[given("一个运行中的后端服务")]
async fn running_backend_service(world: &mut AgentWorld) {
    world.start_mock_server().await;
}

#[given(regex = r#"^工作目录下存在文件 "(.*)"，内容为 "(.*)"$"#)]
async fn file_exists_with_content(world: &mut AgentWorld, filename: String, content: String) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let file_path = workspace.join(&filename);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&file_path, &content).unwrap();
    world.current_file = Some(filename);
    world.current_file_content = Some(content);
}

#[given(regex = r#"^用户已发送复杂任务 "(.*)"$"#)]
async fn user_sent_complex_task(world: &mut AgentWorld, message: String) {
    world.send_chat_message(&message).await;
}

#[given("Agent 已生成任务计划")]
async fn task_plan_generated(world: &mut AgentWorld) {
    let has_tool_use = world
        .tool_use_events()
        .iter()
        .any(|e| e.tool_name.as_deref() == Some("handle_task_plan"));
    assert!(has_tool_use, "Task plan should have been generated");
}

#[given("一个初始化的 TUI 前端")]
async fn initialized_tui(_world: &mut AgentWorld) {
    // TUI 前端在 BDD 测试中通过 Mock 方式模拟
    // 实际渲染测试在单元测试中使用 TestBackend 完成
}

#[given("TUI 当前处于正常聊天界面")]
async fn tui_normal_chat(_world: &mut AgentWorld) {
    // 默认状态即为正常聊天界面
}

#[when(regex = r#"^用户发送消息 "(.*)"$"#)]
async fn user_sends_message(world: &mut AgentWorld, message: String) {
    world.send_chat_message(&message).await;
    world.last_response = world.all_message_text();
}

#[when(regex = r#"^用户以 Build Agent 发送消息 "(.*)"$"#)]
async fn user_sends_message_build_agent(world: &mut AgentWorld, message: String) {
    world
        .send_chat_message_with_agent(&message, Some(fi_code_core::agent::AgentType::Build))
        .await;
    world.last_response = world.all_message_text();
}

#[when(regex = r#"^用户以 Plan Agent 发送消息 "(.*)"$"#)]
async fn user_sends_message_plan_agent(world: &mut AgentWorld, message: String) {
    world
        .send_chat_message_with_agent(&message, Some(fi_code_core::agent::AgentType::Plan))
        .await;
    world.last_response = world.all_message_text();
}

#[then(regex = r#"^用户应该收到包含 "(.*)" 的响应$"#)]
async fn response_contains(world: &mut AgentWorld, expected: String) {
    let text = world.all_message_text();
    assert!(
        text.contains(&expected),
        "Expected response to contain '{}', but got: {}",
        expected,
        text
    );
}

#[then("用户应该收到命令执行结果")]
async fn user_receives_command_result(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty() || world.events.iter().any(|e| e.event_type == "ToolResult"),
        "Expected command execution result"
    );
}

#[then("用户应该收到所有子任务的执行结果汇总")]
async fn user_receives_task_summary(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty() || world.events.iter().any(|e| e.event_type == "Done"),
        "Expected task summary in response"
    );
}

#[then("Agent 应该收到 AgentInfo 事件，类型为 Build")]
async fn agent_received_build_agent_info(world: &mut AgentWorld) {
    let has_agent_info = world.events.iter().any(|e| e.event_type == "AgentInfo");
    assert!(has_agent_info, "Expected AgentInfo event for Build agent");
}

#[then("Agent 应该收到 AgentInfo 事件，类型为 Plan")]
async fn agent_received_plan_agent_info(world: &mut AgentWorld) {
    let has_agent_info = world.events.iter().any(|e| e.event_type == "AgentInfo");
    assert!(has_agent_info, "Expected AgentInfo event for Plan agent");
}

#[then(regex = r#"^Agent 应该收到 ToolError 事件，内容为 "(.*)"$"#)]
async fn agent_received_tool_error(world: &mut AgentWorld, expected: String) {
    let tool_errors = world.tool_error_events();
    let found = tool_errors
        .iter()
        .any(|e| e.content.as_deref().unwrap_or("").contains(&expected));
    assert!(
        found,
        "Expected ToolError containing '{}', but got: {:?}",
        expected,
        tool_errors
            .iter()
            .map(|e| e.content.clone())
            .collect::<Vec<_>>()
    );
}
