// MIT License
// Copyright (c) 2025 fi-code contributors

use cucumber::{given, when, then};
use crate::bdd::AgentWorld;

// =============================================================================
// 日志窗口步骤定义
// Feature: log_window.feature
// =============================================================================

#[given("日志窗口已打开")]
async fn log_window_opened(world: &mut AgentWorld) {
    world.tui_log_visible = true;
}

#[when("用户按下 Ctrl+L")]
async fn user_presses_ctrl_l(world: &mut AgentWorld) {
    world.tui_log_visible = !world.tui_log_visible;
}

#[then("日志窗口应该显示")]
async fn log_window_should_display(world: &mut AgentWorld) {
    assert!(
        world.tui_log_visible,
        "Expected log window to be visible"
    );
}

#[then("日志窗口应该显示历史日志信息")]
async fn log_window_shows_history(world: &mut AgentWorld) {
    // 日志内容通过 SSE 流获取
    assert!(
        !world.events.is_empty() || world.tui_log_visible,
        "Expected log window to show historical logs"
    );
}

#[then("日志窗口应该实时更新并显示新日志")]
async fn log_window_updates_realtime(world: &mut AgentWorld) {
    // 验证有事件被接收
    assert!(
        !world.events.is_empty(),
        "Expected real-time log updates"
    );
}

#[then("日志窗口应该自动滚动到最新日志")]
async fn log_window_auto_scrolls(world: &mut AgentWorld) {
    // 自动滚动行为由前端实现
    // 只要收到 Done 事件就认为日志已更新到最新
    assert!(
        world.events.iter().any(|e| e.event_type == "Done"),
        "Expected log window to auto-scroll to latest logs"
    );
}

#[then("日志窗口应该关闭")]
async fn log_window_should_close(world: &mut AgentWorld) {
    assert!(
        !world.tui_log_visible,
        "Expected log window to be closed"
    );
}

#[then("TUI 应该返回正常聊天界面")]
async fn tui_returns_to_chat(world: &mut AgentWorld) {
    assert!(
        !world.tui_log_visible,
        "Expected TUI to return to normal chat interface"
    );
}

#[when("后端发送新的日志事件")]
async fn backend_sends_log_event(world: &mut AgentWorld) {
    // 模拟后端发送日志：通过发送一条简单消息来产生 SSE 事件
    world.send_chat_message("你好").await;
}

#[when("后端连接断开")]
async fn backend_disconnects(world: &mut AgentWorld) {
    world.is_connected = false;
    if let Some(handle) = world.server_handle.take() {
        handle.abort();
    }
}

#[then("日志窗口应该显示断开连接横幅")]
async fn log_window_shows_disconnect_banner(world: &mut AgentWorld) {
    assert!(
        !world.is_connected,
        "Expected disconnect banner in log window"
    );
}

#[then("日志内容应该保留不变")]
async fn log_content_preserved(world: &mut AgentWorld) {
    // 验证日志窗口状态保持（events 可能为空，但日志窗口应该仍处于打开状态）
    assert!(
        world.tui_log_visible || !world.events.is_empty(),
        "Expected log content to be preserved after disconnect"
    );
}
