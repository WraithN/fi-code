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

use std::cell::{Cell, RefCell};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::components::part_renderer::PartRendererRegistry;
use crate::components::Component;
use crate::theme::Theme;
use fi_code_core::log_debug;
use fi_code_core::log_error;
use fi_code_core::log_info;
use fi_code_core::log_warn;
use fi_code_core::server::transport::sse::SseEvent;
use fi_code_core::session::message::Part;
use fi_code_shared::tui_event::AppEvent;

/// 对话回合：包含用户消息和 AI 回复的 Part 列表。
#[derive(Debug, Clone)]
pub struct Turn {
    pub user_message: String,
    pub parts: Vec<Part>,
    pub is_complete: bool,
}

/// 聊天消息结构（保留用于兼容系统消息等旧逻辑）。
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// 消息发送者角色。
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,      // 用户
    Assistant, // AI 助手
    System,    // 系统提示
    Error,     // 错误信息
}

/// 聊天组件，负责显示对话历史、处理 SSE 流式消息、渲染生成动画。
pub struct Chat {
    pub turns: Vec<Turn>,                             // 对话回合列表
    messages: Vec<Message>,                           // 保留的系统消息/错误消息（向后兼容）
    pub scroll_offset: usize,                         // 垂直滚动偏移（以行为单位）
    pub is_generating: bool,                          // 是否正在生成回复
    spinner_frame: usize,                             // 当前 spinner 动画帧索引
    pub card_hit_areas: RefCell<Vec<(String, Rect)>>, // 卡片点击区域（card_id -> rect）
    pub renderer_registry: PartRendererRegistry,      // Part 渲染器注册表
    last_inner_size: Cell<Option<Rect>>, // 最近一次 draw 的 inner 区域尺寸（用于滚动 clamp）
    pub auto_scroll: bool,               // 是否自动滚动到底部跟随新内容
}

// SPINNER_FRAMES 已从 fi_code_shared::constants 导入
use fi_code_shared::constants::SPINNER_FRAMES;

impl Chat {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            messages: Vec::new(),
            scroll_offset: 0,
            is_generating: false,
            spinner_frame: 0,
            card_hit_areas: RefCell::new(Vec::new()),
            renderer_registry: PartRendererRegistry::new(),
            last_inner_size: Cell::new(None),
            auto_scroll: true,
        }
    }

    /// 添加一条用户发送的消息，创建新的 Turn。
    pub fn add_user_message(&mut self, content: &str) {
        log_debug!(
            "[Client] Chat add_user_message | turns={} | content_len={}",
            self.turns.len(),
            content.len()
        );
        self.turns.push(Turn {
            user_message: content.to_string(),
            parts: Vec::new(),
            is_complete: false,
        });
    }

    /// 添加一条系统消息（保留向后兼容）。
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
        });
    }

    /// 清空所有消息。
    pub fn clear_messages(&mut self) {
        self.turns.clear();
        self.messages.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
        self.card_hit_areas.borrow_mut().clear();
        self.last_inner_size.set(None);
    }

    /// 计算当前可见区域下的最大滚动偏移量。
    fn max_scroll_offset(&self) -> usize {
        let Some(inner) = self.last_inner_size.get() else {
            return 0;
        };
        let total = self.total_height(inner.width);
        total.saturating_sub(inner.height) as usize
    }

    /// 定时 tick：若正在生成回复，则推进 spinner 动画帧。
    pub fn on_tick(&mut self) {
        if self.is_generating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// 创建 Thinking 占位 Part（在收到第一个 token 前显示）。
    pub fn create_thinking_card(&mut self) {
        let turn_idx = self.turns.len().saturating_sub(1);
        log_debug!("[Client] Chat create_thinking_card | turn_idx={}", turn_idx);
        if let Some(last_turn) = self.turns.last_mut() {
            last_turn.parts.push(Part::Reasoning {
                thinking: String::new(),
                signature: None,
            });
        }
    }

    /// 处理 SSE 事件：将流式内容追加到当前 Turn 的 Part 列表中。
    pub fn handle_sse_event(&mut self, event: &SseEvent) {
        let Some(last_turn) = self.turns.last_mut() else {
            log_warn!("[Client] Chat handle_sse_event: no turns available");
            return;
        };

        match event {
            SseEvent::Message { content } => {
                log_debug!("[Client] Chat SSE Message | content_len={}", content.len());
                // 追加到最后的 Text Part，或创建新的 Text Part
                if let Some(Part::Text { text }) = last_turn.parts.last_mut() {
                    text.push_str(content);
                } else {
                    // 移除空的 Reasoning（Thinking）占位 Part
                    last_turn.parts.retain(
                        |p| !(matches!(p, Part::Reasoning { thinking, .. } if thinking.is_empty())),
                    );
                    last_turn.parts.push(Part::Text {
                        text: content.clone(),
                    });
                }
            }
            SseEvent::Part { part } => {
                match part {
                    Part::ToolUse { id, .. } => {
                        log_info!("[Client] Chat SSE ToolUse | id={}", id);
                        // 按 id 查找已有的 ToolUse 并更新，避免重复 push
                        if let Some(existing) = last_turn.parts.iter_mut().find_map(|p| {
                            if let Part::ToolUse {
                                id: existing_id, ..
                            } = p
                            {
                                if existing_id == id {
                                    Some(p)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }) {
                            *existing = part.clone();
                        } else {
                            last_turn.parts.push(part.clone());
                        }
                    }
                    Part::ToolResult {
                        tool_call_id,
                        content,
                        ..
                    } => {
                        log_info!(
                            "[Client] Chat SSE ToolResult | tool_use_id={} | content_len={}",
                            tool_call_id,
                            content.len()
                        );
                        last_turn.parts.push(part.clone());
                    }
                    _ => {
                        last_turn.parts.push(part.clone());
                    }
                }
            }
            SseEvent::TaskProgress { plan_id, tasks } => {
                log_debug!(
                    "[Client] Chat SSE TaskProgress | plan_id={} | tasks={}",
                    plan_id,
                    tasks.len()
                );
                let task_count = tasks.len();
                let mut content = String::new();
                for task in tasks {
                    let icon = match task.status.as_str() {
                        "pending" => "⏳",
                        "in_progress" => "🔵",
                        "completed" => "✅",
                        "failed" => "❌",
                        _ => "❓",
                    };
                    content.push_str(&format!("{} {}\n", icon, task.name));
                }
                last_turn.parts.push(Part::Text {
                    text: format!("Task Plan ({} tasks)\n{}", task_count, content),
                });
            }
            SseEvent::Error { message } => {
                log_error!("[Client] Chat SSE Error | {}", message);
                last_turn.parts.push(Part::Text {
                    text: message.clone(),
                });
            }
            _ => {}
        }
    }

    /// 设置生成状态：开始生成时显示 spinner，结束时重置动画。
    pub fn set_generating(&mut self, generating: bool) {
        log_debug!(
            "[Client] Chat set_generating | {} -> {}",
            self.is_generating,
            generating
        );
        self.is_generating = generating;
        if !generating {
            self.spinner_frame = 0;
            // 标记最后一轮为完成
            if let Some(last) = self.turns.last_mut() {
                last.is_complete = true;
            }
        }
    }

    /// 准备重试指定 Turn：标记为完成并移除错误 Part，返回用户消息。
    pub fn retry_turn(&mut self, turn_index: usize) -> Option<String> {
        let turn = self.turns.get_mut(turn_index)?;
        turn.is_complete = true;
        turn.parts.retain(|p| !matches!(p, Part::ToolError { .. }));
        Some(turn.user_message.clone())
    }

    /// 向上滚动一页。
    fn handle_page_up(&mut self) -> Option<AppEvent> {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        self.auto_scroll = false; // 用户手动向上滚动，暂停自动跟随
        Some(AppEvent::ScrollUp)
    }

    /// 计算给定宽度下所有内容的总高度。
    fn total_height(&self, width: u16) -> u16 {
        let mut height = 0u16;
        for turn in &self.turns {
            height += 1; // "You" 前缀
            let content_para = Paragraph::new(turn.user_message.clone()).wrap(Wrap { trim: true });
            height += content_para.line_count(width).max(1) as u16;
            height += 1; // 空行

            // 按操作组统计高度，与 draw 逻辑保持一致
            let mut part_idx = 0;
            let mut turn_tool_count = 0usize;

            while part_idx < turn.parts.len() {
                let part = &turn.parts[part_idx];
                let is_tool_sequence = matches!(part, Part::ToolUse { .. });

                if is_tool_sequence {
                    let seq_start = part_idx;
                    let mut seq_end = part_idx;
                    while seq_end < turn.parts.len() {
                        match &turn.parts[seq_end] {
                            Part::ToolUse { .. }
                            | Part::ToolResult { .. }
                            | Part::ToolError { .. } => seq_end += 1,
                            _ => break,
                        }
                    }
                    let tool_count = seq_end - seq_start;
                    turn_tool_count += tool_count;

                    // 组标题高度（多于1个工具时）
                    if tool_count > 1 {
                        height += 1 + 1; // 标题行 + 间距
                    }

                    // 组内 parts 高度
                    for j in seq_start..seq_end {
                        let p = &turn.parts[j];
                        if let Some(renderer) = self.renderer_registry.get(p) {
                            height += renderer.height(p, width);
                            height += 1; // Part 间距
                        }
                    }
                    part_idx = seq_end;
                } else {
                    if let Some(renderer) = self.renderer_registry.get(part) {
                        height += renderer.height(part, width);
                        height += 1; // Part 间距
                    }
                    part_idx += 1;
                }
            }

            // 执行摘要行高度
            if turn_tool_count > 0 {
                height += 1 + 1; // 摘要行 + 间距
            }
        }
        for msg in &self.messages {
            height += 1; // 前缀
            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            height += content_para.line_count(width).max(1) as u16;
            height += 1; // 空行
        }
        if self.is_generating || !self.turns.is_empty() {
            height += 1; // spinner 或静止满格状态指示器
        }
        height
    }
}

impl Component for Chat {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        let border_type = if is_focused {
            BorderType::Double
        } else {
            BorderType::Plain
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        self.last_inner_size.set(Some(inner));
        frame.render_widget(block, area);

        // 计算总高度和最大滚动偏移
        let total_height = self.total_height(inner.width);
        let max_scroll_offset = total_height.saturating_sub(inner.height) as usize;

        // 如果开启了自动跟随，滚动偏移始终锁定到底部；否则使用用户手动设置的偏移
        let scroll_y = if self.auto_scroll {
            max_scroll_offset
        } else {
            self.scroll_offset.min(max_scroll_offset)
        } as u16;

        let mut current_y = 0u16; // 虚拟 Y 坐标（相对于内容顶部）

        // 辅助函数：计算元素与视口的交集区域，返回渲染 Rect 和顶部跳过的行数。
        // skip_lines 用于 Paragraph::scroll，保证被视口裁剪掉的上半部分内容不会重新渲染在顶部。
        let clip_rect = |cy: u16, h: u16| -> Option<(Rect, u16)> {
            let view_top = scroll_y;
            let view_bottom = scroll_y.saturating_add(inner.height);
            let elem_top = cy;
            let elem_bottom = cy.saturating_add(h);

            // 无交集
            if elem_bottom <= view_top || elem_top >= view_bottom {
                return None;
            }

            let clipped_top = elem_top.max(view_top);
            let clipped_bottom = elem_bottom.min(view_bottom);
            let render_y = inner.y.saturating_add(clipped_top.saturating_sub(view_top));
            let render_h = clipped_bottom.saturating_sub(clipped_top);
            let skip_lines = clipped_top.saturating_sub(elem_top);

            Some((
                Rect {
                    x: inner.x,
                    y: render_y,
                    width: inner.width,
                    height: render_h,
                },
                skip_lines,
            ))
        };

        for turn in &self.turns {
            // 用户消息前缀
            let prefix_height = 1u16;
            if let Some((rect, _)) = clip_rect(current_y, prefix_height) {
                let para = Paragraph::new(Line::from(vec![
                    Span::styled("● ", theme.style_user()),
                    Span::styled("You", theme.style_user().add_modifier(Modifier::BOLD)),
                ]));
                frame.render_widget(para, rect);
            }
            current_y += prefix_height;

            // 用户消息内容
            let content_para = Paragraph::new(turn.user_message.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if let Some((rect, skip_lines)) = clip_rect(current_y, content_height) {
                frame.render_widget(content_para.scroll((skip_lines, 0)), rect);
            }
            current_y += content_height;

            // 空行
            current_y += 1;

            // AI Parts — 按操作组渲染，连续的工具调用序列归为一组
            let mut part_idx = 0;
            let mut turn_tool_count = 0usize;
            let mut turn_success_count = 0usize;
            let mut turn_error_count = 0usize;
            let mut turn_total_duration = 0u64;

            while part_idx < turn.parts.len() {
                let part = &turn.parts[part_idx];

                // 检查是否是连续工具调用序列的开始
                let is_tool_sequence = matches!(part, Part::ToolUse { .. });

                if is_tool_sequence {
                    // 收集整个工具调用序列（ToolUse + ToolResult + ToolError）
                    let seq_start = part_idx;
                    let mut seq_end = part_idx;

                    while seq_end < turn.parts.len() {
                        match &turn.parts[seq_end] {
                            Part::ToolUse { .. }
                            | Part::ToolResult { .. }
                            | Part::ToolError { .. } => {
                                seq_end += 1;
                            }
                            _ => break,
                        }
                    }

                    let tool_count = seq_end - seq_start;

                    // 统计该序列的信息
                    let mut seq_success = 0usize;
                    let mut seq_error = 0usize;
                    let mut seq_duration = 0u64;
                    let mut seq_tool_names: Vec<&str> = Vec::new();

                    for j in seq_start..seq_end {
                        match &turn.parts[j] {
                            Part::ToolUse { name, .. } => {
                                seq_tool_names.push(name);
                            }
                            Part::ToolResult {
                                content,
                                duration_ms,
                                ..
                            } => {
                                if content.contains("Error") || content.contains("error") {
                                    seq_error += 1;
                                } else {
                                    seq_success += 1;
                                }
                                if let Some(ms) = duration_ms {
                                    seq_duration += ms;
                                }
                            }
                            Part::ToolError { .. } => {
                                seq_error += 1;
                            }
                            _ => {}
                        }
                    }

                    // 累加到 Turn 级别统计
                    turn_tool_count += seq_tool_names.len();
                    turn_success_count += seq_success;
                    turn_error_count += seq_error;
                    turn_total_duration += seq_duration;

                    // 如果有多于1个工具，渲染操作组标题
                    if tool_count > 1 {
                        let group_title = if seq_duration > 0 {
                            if seq_duration < 1000 {
                                format!(
                                    "▶ {} 个工具 | ✅ {} | ❌ {} | ⏱ {}ms",
                                    seq_tool_names.len(),
                                    seq_success,
                                    seq_error,
                                    seq_duration
                                )
                            } else {
                                format!(
                                    "▶ {} 个工具 | ✅ {} | ❌ {} | ⏱ {:.1}s",
                                    seq_tool_names.len(),
                                    seq_success,
                                    seq_error,
                                    seq_duration as f64 / 1000.0
                                )
                            }
                        } else {
                            format!(
                                "▶ {} 个工具 | ✅ {} | ❌ {}",
                                seq_tool_names.len(),
                                seq_success,
                                seq_error
                            )
                        };
                        let group_line = Line::from(vec![Span::styled(
                            group_title,
                            Style::default()
                                .fg(theme.text_muted)
                                .add_modifier(Modifier::BOLD),
                        )]);
                        let group_para = Paragraph::new(group_line);
                        let group_h = 1u16;
                        if let Some((rect, _)) = clip_rect(current_y, group_h) {
                            frame.render_widget(group_para, rect);
                        }
                        current_y += group_h + 1;
                    }

                    // 渲染序列内所有 parts
                    for j in seq_start..seq_end {
                        let p = &turn.parts[j];
                        if let Some(renderer) = self.renderer_registry.get(p) {
                            let part_height = renderer.height(p, inner.width);
                            if let Some((rect, skip_lines)) = clip_rect(current_y, part_height) {
                                renderer.draw(frame, rect, p, theme, skip_lines);
                            }
                            current_y += part_height + 1;
                        }
                    }

                    part_idx = seq_end;
                } else {
                    // 渲染单个非工具 Part（Text、Reasoning、Image 等）
                    if let Some(renderer) = self.renderer_registry.get(part) {
                        let part_height = renderer.height(part, inner.width);
                        if let Some((rect, skip_lines)) = clip_rect(current_y, part_height) {
                            renderer.draw(frame, rect, part, theme, skip_lines);
                        }
                        current_y += part_height + 1;
                    }
                    part_idx += 1;
                }
            }

            // 如果该 Turn 有工具调用，在末尾渲染执行摘要行
            if turn_tool_count > 0 {
                let summary = if turn_total_duration > 0 {
                    if turn_total_duration < 1000 {
                        format!(
                            "共 {} 个工具 | ✅ {} | ❌ {} | ⏱ {}ms",
                            turn_tool_count,
                            turn_success_count,
                            turn_error_count,
                            turn_total_duration
                        )
                    } else {
                        format!(
                            "共 {} 个工具 | ✅ {} | ❌ {} | ⏱ {:.1}s",
                            turn_tool_count,
                            turn_success_count,
                            turn_error_count,
                            turn_total_duration as f64 / 1000.0
                        )
                    }
                } else {
                    format!(
                        "共 {} 个工具 | ✅ {} | ❌ {}",
                        turn_tool_count, turn_success_count, turn_error_count
                    )
                };
                let summary_line = Line::from(vec![Span::styled(
                    summary,
                    Style::default().fg(theme.text_muted),
                )]);
                let summary_h = 1u16;
                if let Some((rect, _)) = clip_rect(current_y, summary_h) {
                    frame.render_widget(Paragraph::new(summary_line), rect);
                }
                current_y += summary_h + 1;
            }
        }

        // 渲染保留的系统消息/错误消息
        for msg in &self.messages {
            let (prefix, style) = match msg.role {
                MessageRole::User => ("You", theme.style_user().add_modifier(Modifier::BOLD)),
                MessageRole::Assistant => {
                    ("◆ AI", theme.style_brand().add_modifier(Modifier::BOLD))
                }
                MessageRole::System => ("ℹ️ ", Style::default().fg(theme.warning)),
                MessageRole::Error => ("❌ ", Style::default().fg(theme.error)),
            };

            let prefix_height = 1u16;
            if let Some((rect, _)) = clip_rect(current_y, prefix_height) {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(prefix, style)])),
                    rect,
                );
            }
            current_y += prefix_height;

            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if let Some((rect, skip_lines)) = clip_rect(current_y, content_height) {
                frame.render_widget(content_para.scroll((skip_lines, 0)), rect);
            }
            current_y += content_height;
            current_y += 1; // 空行
        }

        // AI 状态指示器
        // 生成中：动画 spinner；等待用户输入：静止满格 ●
        let has_conversation = !self.turns.is_empty();
        if self.is_generating {
            let spinner = SPINNER_FRAMES[self.spinner_frame];
            let spinner_height = 1u16;
            if let Some((rect, _)) = clip_rect(current_y, spinner_height) {
                let spinner_line = Line::from(vec![
                    Span::styled("◆ ", theme.style_brand()),
                    Span::styled("AI ", theme.style_brand().add_modifier(Modifier::BOLD)),
                    Span::styled(spinner, theme.style_brand()),
                ]);
                frame.render_widget(Paragraph::new(spinner_line), rect);
            }
        } else if has_conversation {
            // 有对话历史且未在生成：显示静止满格，颜色反映最后一轮状态
            // 检查最后一轮是否有错误（ToolError 或包含 Error 的 ToolResult）
            let last_turn_has_error = self.turns.last().map_or(false, |turn| {
                turn.parts.iter().any(|p| match p {
                    Part::ToolError { .. } => true,
                    Part::ToolResult { content, .. } => {
                        content.contains("Error") || content.contains("error")
                    }
                    _ => false,
                })
            });
            let dot_color = if last_turn_has_error {
                theme.error // 红色：最后一轮有错误
            } else {
                theme.success // 绿色：正常完成
            };
            let status_height = 1u16;
            if let Some((rect, _)) = clip_rect(current_y, status_height) {
                let status_line = Line::from(vec![
                    Span::styled("◆ ", theme.style_brand()),
                    Span::styled("AI ", theme.style_brand().add_modifier(Modifier::BOLD)),
                    Span::styled("●", Style::default().fg(dot_color)),
                ]);
                frame.render_widget(Paragraph::new(status_line), rect);
            }
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        match event {
            Event::Mouse(mouse) => {
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(3);
                        self.auto_scroll = false; // 用户手动向上滚动，暂停自动跟随
                        Some(AppEvent::ScrollUp)
                    }
                    MouseEventKind::ScrollDown => {
                        let max_offset = self.max_scroll_offset();
                        self.scroll_offset = (self.scroll_offset + 3).min(max_offset);
                        // 如果滚动到了底部，恢复自动跟随
                        self.auto_scroll = self.scroll_offset >= max_offset;
                        Some(AppEvent::ScrollDown)
                    }
                    _ => None,
                }
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return None;
                }
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Up)
                    | (KeyModifiers::NONE, KeyCode::PageUp) => self.handle_page_up(),
                    (KeyModifiers::CONTROL, KeyCode::Down)
                    | (KeyModifiers::NONE, KeyCode::PageDown) => {
                        let max_offset = self.max_scroll_offset();
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                        // 如果滚动到了底部，恢复自动跟随
                        self.auto_scroll = self.scroll_offset >= max_offset;
                        Some(AppEvent::ScrollDown)
                    }
                    (KeyModifiers::NONE, KeyCode::Char('g')) => {
                        self.turns.iter().rev().find_map(|turn| {
                            turn.parts.iter().find_map(|part| match part {
                                Part::WaveMarker {
                                    git_snapshot: Some(hash),
                                    ..
                                } => Some(AppEvent::BrowseGitSnapshot(hash.clone())),
                                _ => None,
                            })
                        })
                    }
                    (KeyModifiers::NONE, KeyCode::Char('r')) => {
                        self.turns.iter().rev().find_map(|turn| {
                            turn.parts.iter().find_map(|part| match part {
                                Part::WaveMarker {
                                    git_snapshot: Some(snapshot),
                                    step,
                                    ..
                                } => Some(AppEvent::RollbackToWave {
                                    snapshot: snapshot.clone(),
                                    step: *step,
                                }),
                                _ => None,
                            })
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, MouseEvent};

    #[test]
    fn test_add_user_message_creates_turn() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        assert_eq!(chat.turns.len(), 1);
        assert_eq!(chat.turns[0].user_message, "hello");
    }

    #[test]
    fn test_sse_message_creates_text_part() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.handle_sse_event(&SseEvent::Message {
            content: "world".to_string(),
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Text { .. }));
        if let Part::Text { text } = &chat.turns[0].parts[0] {
            assert_eq!(text, "world");
        }
    }

    #[test]
    fn test_generating_state() {
        let mut chat = Chat::new();
        chat.set_generating(true);
        assert!(chat.is_generating);
        chat.on_tick();
        assert_eq!(chat.spinner_frame, 1);
    }

    #[test]
    fn test_add_system_message() {
        let mut chat = Chat::new();
        chat.add_system_message("System alert");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, MessageRole::System);
        assert_eq!(chat.messages[0].content, "System alert");
    }

    #[test]
    fn test_clear_messages() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.add_system_message("System alert");
        chat.clear_messages();
        assert!(chat.turns.is_empty());
        assert!(chat.messages.is_empty());
    }

    #[test]
    fn test_tool_use_creates_tool_part() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"cmd": "ls"}),
            },
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::ToolUse { .. }));
    }

    #[test]
    fn test_tool_result_creates_tool_result_part() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"cmd": "ls"}),
            },
        });
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolResult {
                tool_call_id: "tool_1".to_string(),
                content: "file.txt".to_string(),
                duration_ms: Some(120),
                metadata: None,
                for_context_only: false,
            },
        });
        assert_eq!(chat.turns[0].parts.len(), 2);
        assert!(matches!(chat.turns[0].parts[1], Part::ToolResult { .. }));
    }

    #[test]
    fn test_thinking_placeholder_removed_on_message() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.create_thinking_card();
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Reasoning { .. }));

        chat.handle_sse_event(&SseEvent::Message {
            content: "world".to_string(),
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Text { .. }));
    }

    // ========== 自动滚动跟随测试 ==========

    #[test]
    fn test_auto_scroll_default_true() {
        let chat = Chat::new();
        assert!(chat.auto_scroll, "默认应开启自动跟随");
    }

    #[test]
    fn test_page_up_disables_auto_scroll() {
        let mut chat = Chat::new();
        // 设置一个较小的视口，使内容可以滚动
        chat.last_inner_size.set(Some(Rect::new(0, 0, 10, 5)));
        chat.add_user_message("a very long message that wraps across multiple lines");
        chat.scroll_offset = 5;
        chat.auto_scroll = true;

        let event = Event::Key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        chat.handle_event(&event, true);

        assert!(!chat.auto_scroll, "PageUp 后应暂停自动跟随");
    }

    #[test]
    fn test_scroll_up_disables_auto_scroll() {
        let mut chat = Chat::new();
        chat.last_inner_size.set(Some(Rect::new(0, 0, 10, 5)));
        chat.add_user_message("a very long message that wraps across multiple lines");
        chat.scroll_offset = 5;
        chat.auto_scroll = true;

        let event = Event::Mouse(MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        chat.handle_event(&event, true);

        assert!(!chat.auto_scroll, "鼠标滚轮向上滚动后应暂停自动跟随");
    }

    #[test]
    fn test_page_down_to_bottom_enables_auto_scroll() {
        let mut chat = Chat::new();
        // 使用窄视口和足够长的消息，确保 max_scroll_offset > 0
        chat.last_inner_size.set(Some(Rect::new(0, 0, 10, 5)));
        chat.add_user_message("line1 line2 line3 line4 line5 line6 line7 line8 line9 line10");
        chat.add_user_message("another line to make it scrollable for sure");

        let max_offset = chat.max_scroll_offset();
        assert!(max_offset > 0, "测试前提：内容应超出视口高度");

        // 模拟用户先向上滚动，然后 PageDown 回到底部
        chat.scroll_offset = max_offset.saturating_sub(1);
        chat.auto_scroll = false;

        let event = Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        chat.handle_event(&event, true);

        assert!(chat.auto_scroll, "PageDown 回到底部后应恢复自动跟随");
    }

    #[test]
    fn test_scroll_down_to_bottom_enables_auto_scroll() {
        let mut chat = Chat::new();
        chat.last_inner_size.set(Some(Rect::new(0, 0, 10, 5)));
        chat.add_user_message("line1 line2 line3 line4 line5 line6 line7 line8 line9 line10");
        chat.add_user_message("another line to make it scrollable for sure");

        let max_offset = chat.max_scroll_offset();
        assert!(
            max_offset > 2,
            "测试前提：内容应足够长，允许从 max-2 滚到底部"
        );

        chat.scroll_offset = max_offset.saturating_sub(2);
        chat.auto_scroll = false;

        let event = Event::Mouse(MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        chat.handle_event(&event, true);

        assert!(chat.auto_scroll, "鼠标滚轮向下回到底部后应恢复自动跟随");
    }

    #[test]
    fn test_page_down_not_to_bottom_keeps_auto_scroll_false() {
        let mut chat = Chat::new();
        chat.last_inner_size.set(Some(Rect::new(0, 0, 10, 5)));
        chat.add_user_message("line1 line2 line3 line4 line5 line6 line7 line8 line9 line10");
        chat.add_user_message("another line to make it scrollable for sure");

        let max_offset = chat.max_scroll_offset();
        assert!(
            max_offset > 2,
            "测试前提：max_scroll_offset 应大于 2，以便从 0 滚 1 行不到底部"
        );

        // 从顶部开始，只滚一行，不到底部
        chat.scroll_offset = 0;
        chat.auto_scroll = false;

        let event = Event::Key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        chat.handle_event(&event, true);

        assert!(!chat.auto_scroll, "PageDown 不到底部时应保持暂停状态");
        assert_eq!(chat.scroll_offset, 1);
    }

    #[test]
    fn test_clear_messages_resets_auto_scroll() {
        let mut chat = Chat::new();
        chat.auto_scroll = false;
        chat.clear_messages();
        assert!(
            chat.auto_scroll,
            "clear_messages 应重置 auto_scroll 为 true"
        );
    }
}
