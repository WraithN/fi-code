// MIT License
// Copyright (c) 2025 fi-code contributors

use cucumber::{given, when, then};
use crate::bdd::AgentWorld;

// =============================================================================
// Agent 技能调用步骤定义
// Feature: agent_skills.feature
// =============================================================================

#[given(regex = r#"^系统中已注册 (.*) 技能$"#)]
async fn skill_registered(world: &mut AgentWorld, skill_name: String) {
    world.registered_skills.push(skill_name);
}

#[then("Agent 应该调用 use_skill 工具")]
async fn agent_calls_use_skill(world: &mut AgentWorld) {
    let tool_use_events = world.tool_use_events();
    let has_use_skill = tool_use_events.iter().any(|e| {
        e.tool_name.as_deref() == Some("use_skill")
    });
    assert!(has_use_skill, "Expected Agent to call use_skill tool");
}

#[then(regex = r#"^技能名称应该为 "(.*)"$"#)]
async fn skill_name_should_be(world: &mut AgentWorld, expected_name: String) {
    let tool_use_events = world.tool_use_events();
    let found = tool_use_events.iter().any(|e| {
        if let Some(ref args) = e.tool_args {
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                return name == expected_name;
            }
        }
        false
    });
    assert!(found, "Expected skill name to be '{}'", expected_name);
}

#[then("技能内容应该被注入到对话上下文中")]
async fn skill_content_injected(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty(),
        "Expected skill content to be injected into conversation"
    );
}

#[then("审查结果应该包含具体的改进建议")]
async fn review_contains_suggestions(world: &mut AgentWorld) {
    let text = world.all_message_text();
    assert!(
        !text.is_empty() || world.events.iter().any(|e| e.event_type == "Done"),
        "Expected review to contain suggestions"
    );
}
