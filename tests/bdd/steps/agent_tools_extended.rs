// MIT License
// Copyright (c) 2025 fi-code contributors

use crate::bdd::AgentWorld;
use cucumber::then;

// =============================================================================
// Agent 扩展工具调用步骤定义
// Feature: agent_tools_extended.feature
// =============================================================================

#[then("Agent 应该调用 grep 工具")]
async fn agent_calls_grep_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_grep = tool_use_events
        .iter()
        .any(|e| e.tool_name.as_deref() == Some("grep"));
    assert!(
        has_grep,
        "Expected Agent to call grep tool, but got: {:?}",
        tool_use_events
            .iter()
            .map(|e| e.tool_name.clone())
            .collect::<Vec<_>>()
    );
}

#[then("Agent 应该调用 glob 工具")]
async fn agent_calls_glob_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_glob = tool_use_events
        .iter()
        .any(|e| e.tool_name.as_deref() == Some("glob"));
    assert!(
        has_glob,
        "Expected Agent to call glob tool, but got: {:?}",
        tool_use_events
            .iter()
            .map(|e| e.tool_name.clone())
            .collect::<Vec<_>>()
    );
}

#[then("Agent 应该调用 git_status 工具")]
async fn agent_calls_git_status_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_git_status = tool_use_events
        .iter()
        .any(|e| e.tool_name.as_deref() == Some("git_status"));
    assert!(
        has_git_status,
        "Expected Agent to call git_status tool, but got: {:?}",
        tool_use_events
            .iter()
            .map(|e| e.tool_name.clone())
            .collect::<Vec<_>>()
    );
}

#[then("Agent 应该调用 git_log 工具")]
async fn agent_calls_git_log_tool(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_git_log = tool_use_events
        .iter()
        .any(|e| e.tool_name.as_deref() == Some("git_log"));
    assert!(
        has_git_log,
        "Expected Agent to call git_log tool, but got: {:?}",
        tool_use_events
            .iter()
            .map(|e| e.tool_name.clone())
            .collect::<Vec<_>>()
    );
}

#[then("工具结果应该非空")]
async fn tool_result_should_not_be_empty(world: &mut AgentWorld) {
    let text = world.all_tool_result_text();
    assert!(
        !text.is_empty(),
        "Expected tool result to be non-empty, but got empty"
    );
}
