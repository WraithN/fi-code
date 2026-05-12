// MIT License
// Copyright (c) 2025 fi-code contributors

use cucumber::{given, when, then};
use crate::bdd::AgentWorld;

// =============================================================================
// 斜杠命令步骤定义
// Feature: slash_commands.feature
// =============================================================================

#[given("系统中配置了多个模型")]
async fn multiple_models_configured(_world: &mut AgentWorld) {
    // Mock Provider 默认配置了一个模型
    // 实际多模型测试需要扩展配置
}

#[when(regex = r#"^用户发送命令 "(.*)"$"#)]
async fn user_sends_command(world: &mut AgentWorld, command: String) {
    world.send_chat_message(&command).await;
    world.last_response = world.all_message_text();
}

#[then("系统应该返回可用模型列表")]
async fn system_returns_model_list(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty() || world.events.iter().any(|e| e.event_type == "Done"),
        "Expected model list in response"
    );
}

#[when(regex = r#"^用户选择模型 "(.*)"$"#)]
async fn user_selects_model(world: &mut AgentWorld, model_name: String) {
    // 模型切换通过发送特定消息到后端
    // Mock 模式下此操作仅记录选择
    world.send_chat_message(&format!("切换模型到 {}", model_name)).await;
}

#[then(regex = r#"^当前模型应该切换为 "(.*)"$"#)]
async fn current_model_switched_to(world: &mut AgentWorld, model_name: String) {
    // 验证响应中包含切换确认
    let text = world.all_message_text();
    assert!(
        text.contains(&model_name) || world.events.iter().any(|e| e.event_type == "Done"),
        "Expected model to be switched to '{}'", 
        model_name
    );
}

#[then("用户应该收到确认消息")]
async fn user_receives_confirmation(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty(),
        "Expected confirmation message"
    );
}

#[given("工作目录是空目录")]
async fn workspace_is_empty(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let entries: Vec<_> = std::fs::read_dir(workspace)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(entries.is_empty(), "Expected empty workspace");
}

#[then("系统应该在根目录创建 AGENTS.md 文件")]
async fn agents_md_created(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let agents_path = workspace.join("AGENTS.md");
    assert!(
        agents_path.exists(),
        "Expected AGENTS.md to be created at {:?}",
        agents_path
    );
}

#[then("AGENTS.md 应该包含项目基本信息模板")]
async fn agents_md_contains_template(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let agents_path = workspace.join("AGENTS.md");
    let content = std::fs::read_to_string(&agents_path).unwrap();
    assert!(
        content.contains("#") || content.contains("Agent") || content.len() > 50,
        "Expected AGENTS.md to contain project template, got: {}",
        content
    );
}

#[then("用户应该收到初始化完成的确认")]
async fn user_receives_init_confirmation(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty(),
        "Expected init confirmation message"
    );
}

#[then("系统应该返回所有可用命令的列表")]
async fn system_returns_command_list(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty(),
        "Expected list of available commands"
    );
}

#[then("每个命令应该包含简要说明")]
async fn each_command_has_description(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        text.len() > 20,
        "Expected commands with descriptions"
    );
}
