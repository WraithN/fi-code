// MIT License
// Copyright (c) 2025 fi-code contributors

use cucumber::{given, when, then};
use crate::bdd::AgentWorld;

// =============================================================================
// Agent 工具调用步骤定义
// Feature: agent_tools.feature
// =============================================================================

#[then("Agent 应该调用 write 工具")]
async fn agent_calls_write_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_write = tool_use_events.iter().any(|e| {
        e.tool_name.as_deref() == Some("write")
    });
    assert!(has_write, "Expected Agent to call write tool, but got: {:?}", 
        tool_use_events.iter().map(|e| e.tool_name.clone()).collect::<Vec<_>>());
}

#[then("文件应该被写入到工作目录")]
async fn file_written_to_workspace(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    // MockAIClient 写入的是 test_output/hello.rs
    let file_path = workspace.join("test_output/hello.rs");
    assert!(
        file_path.exists(),
        "File should be written to {:?}",
        file_path
    );
}

#[then("Agent 应该调用 read 工具")]
async fn agent_calls_read_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_read = tool_use_events.iter().any(|e| {
        e.tool_name.as_deref() == Some("read")
    });
    assert!(has_read, "Expected Agent to call read tool");
}

#[then("Agent 应该调用 bash 工具")]
async fn agent_calls_bash_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_bash = tool_use_events.iter().any(|e| {
        e.tool_name.as_deref() == Some("bash")
    });
    assert!(has_bash, "Expected Agent to call bash tool");
}

#[then("Agent 应该调用 edit 工具")]
async fn agent_calls_edit_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_edit = tool_use_events.iter().any(|e| {
        e.tool_name.as_deref() == Some("edit")
    });
    assert!(has_edit, "Expected Agent to call edit tool");
}

#[then("文件内容应该包含修改后的代码")]
async fn file_contains_modified_code(world: &mut AgentWorld) {
    let workspace = world.workspace.as_ref().expect("Workspace not set");
    let filename = world.current_file.as_ref().expect("Current file not set");
    let file_path = workspace.join(filename);
    assert!(file_path.exists(), "File should exist after edit");
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.len() > "fn main() {}".len(),
        "File should contain modified code, got: {}",
        content
    );
}
