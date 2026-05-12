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

use std::cell::RefCell;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_warn;
use crate::server::transport::sse::SseEvent;
use crate::tui::components::card_widget::{Card, CardKind, CardState, CardWidget};
use crate::tui::components::Component;
use crate::tui::event::{AppEvent, CardAction};
use crate::tui::theme::Theme;

/// 对话回合：包含用户消息和 AI 回复的卡片列表。
#[derive(Debug, Clone)]
pub struct Turn {
    pub user_message: String,
    pub cards: Vec<Card>,
    pub is_complete: bool,
}

/// 聊天消息结构（保留用于兼容系统消息等旧逻辑）。
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// 结构化详情（思考过程、工具调用等）
    pub details: Option<Vec<crate::server::transport::sse::DetailBlock>>,
    /// 详情是否展开
    pub details_expanded: bool,
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
    turns: Vec<Turn>,         // 对话回合列表
    messages: Vec<Message>,   // 保留的系统消息/错误消息（向后兼容）
    scroll_offset: usize,     // 垂直滚动偏移（以行为单位）
    is_generating: bool,      // 是否正在生成回复
    spinner_frame: usize,     // 当前 spinner 动画帧索引
    card_hit_areas: RefCell<Vec<(String, Rect)>>, // 卡片点击区域（card_id -> rect）
}

/// 终端 spinner 动画帧（Braille 点阵字符），每 tick 轮播一帧。
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl Chat {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            messages: Vec::new(),
            scroll_offset: 0,
            is_generating: false,
            spinner_frame: 0,
            card_hit_areas: RefCell::new(Vec::new()),
        }
    }

    /// 添加一条用户发送的消息，创建新的 Turn。
    pub fn add_user_message(&mut self, content: &str) {
        log_debug!("[Client] Chat add_user_message | turns={} | content_len={}", self.turns.len(), content.len());
        self.turns.push(Turn {
            user_message: content.to_string(),
            cards: Vec::new(),
            is_complete: false,
        });
    }

    /// 添加一条系统消息（保留向后兼容）。
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
            details: None,
            details_expanded: false,
        });
    }

    /// 清空所有消息。
    pub fn clear_messages(&mut self) {
        self.turns.clear();
        self.messages.clear();
        self.scroll_offset = 0;
        self.card_hit_areas.borrow_mut().clear();
    }

    /// 定时 tick：若正在生成回复，则推进 spinner 动画帧。
    pub fn on_tick(&mut self) {
        if self.is_generating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// 创建 Thinking 占位卡片（在收到第一个 token 前显示）。
    pub fn create_thinking_card(&mut self) {
        let turn_idx = self.turns.len().saturating_sub(1);
        log_debug!("[Client] Chat create_thinking_card | turn_idx={}", turn_idx);
        if let Some(last_turn) = self.turns.last_mut() {
            last_turn.cards.push(Card {
                id: format!("thinking-{}", turn_idx),
                kind: CardKind::Thinking,
                title: "Thinking".to_string(),
                content: String::new(),
                full_content: None,
                right_content: None,
                state: CardState::Animating,
            });
        }
    }

    /// 处理 SSE 事件：将流式内容追加到当前 Turn 的卡片中。
    pub fn handle_sse_event(&mut self, event: &SseEvent) {
        let Some(last_turn) = self.turns.last_mut() else {
            log_warn!("[Client] Chat handle_sse_event: no turns available");
            return;
        };

        match event {
            SseEvent::Message { content } => {
                log_debug!("[Client] Chat SSE Message | content_len={}", content.len());
                // 查找或创建 Summary 卡片
                if let Some(card) = last_turn
                    .cards
                    .iter_mut()
                    .find(|c| matches!(c.kind, CardKind::Summary))
                {
                    card.content.push_str(content);
                } else {
                    // 移除空的 Thinking 卡片
                    last_turn
                        .cards
                        .retain(|c| !(matches!(c.kind, CardKind::Thinking) && c.content.is_empty()));

                    last_turn.cards.push(Card {
                        id: format!("summary-{}", last_turn.cards.len()),
                        kind: CardKind::Summary,
                        title: "AI".to_string(),
                        content: content.clone(),
                        full_content: None,
                        right_content: None,
                        state: CardState::Completed,
                    });
                }
            }
            SseEvent::ToolUse { id, name, arguments } => {
                log_info!("[Client] Chat SSE ToolUse | id={} | name={}", id, name);
                let args_str = serde_json::to_string_pretty(arguments).unwrap_or_default();
                last_turn.cards.push(Card {
                    id: id.clone(),
                    kind: CardKind::ToolUse { name: name.clone() },
                    title: name.clone(),
                    content: args_str,
                    full_content: None,
                    right_content: None,
                    state: CardState::Completed,
                });
            }
            SseEvent::ToolResult {
                tool_use_id,
                content,
                diff,
                is_new_file: _,
            } => {
                log_info!("[Client] Chat SSE ToolResult | tool_use_id={} | content_len={}", tool_use_id, content.len());
                if let Some(card) = last_turn.cards.iter_mut().find(|c| c.id == *tool_use_id) {
                    let name = match &card.kind {
                        CardKind::ToolUse { name } => name.clone(),
                        _ => "Result".to_string(),
                    };

                    let path = if name == "write" || name == "edit" {
                        serde_json::from_str::<serde_json::Value>(&card.content)
                            .ok()
                            .and_then(|v| v.get("path").or_else(|| v.get("file_path")).cloned())
                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                    } else {
                        None
                    };

                    let is_write_file = path.is_some();
                    let (display_content, full_content, state) =
                        if content.chars().count() > 200 {
                            let truncated: String = content.chars().take(200).collect();
                            (format!("{}...", truncated), Some(content.clone()), CardState::Collapsed)
                        } else {
                            (content.clone(), None, CardState::Completed)
                        };

                    *card = Card {
                        id: tool_use_id.clone(),
                        kind: if let Some(p) = path {
                            CardKind::WriteFile { path: p }
                        } else {
                            CardKind::ToolResult
                        },
                        title: format!("{} Result", name),
                        content: display_content,
                        full_content,
                        right_content: diff.clone(),
                        state,
                    };
                }
            }
            SseEvent::TaskProgress { plan_id, tasks } => {
                log_debug!("[Client] Chat SSE TaskProgress | plan_id={} | tasks={}", plan_id, tasks.len());
                if let Some(card) = last_turn.cards.iter_mut().find(|c| {
                    matches!(c.kind, CardKind::TodoList { plan_id: ref pid, .. } if pid == plan_id)
                }) {
                    if let CardKind::TodoList {
                        ref mut tasks,
                        ..
                    } = card.kind
                    {
                        *tasks = tasks.clone();
                    }
                } else {
                    let task_count = tasks.len();
                    let mut content = String::new();
                    for task in tasks {
                        let icon = match task.status {
                            crate::tools::task::TaskStatus::Pending => "⏳",
                            crate::tools::task::TaskStatus::InProgress => "🔵",
                            crate::tools::task::TaskStatus::Completed => "✅",
                            crate::tools::task::TaskStatus::Failed => "❌",
                        };
                        content.push_str(&format!("{} {}\n", icon, task.name));
                    }

                    last_turn.cards.push(Card {
                        id: plan_id.clone(),
                        kind: CardKind::TodoList {
                            plan_id: plan_id.clone(),
                            tasks: tasks.clone(),
                        },
                        title: format!("Task Plan ({} tasks)", task_count),
                        content,
                        full_content: None,
                        right_content: None,
                        state: CardState::Completed,
                    });
                }
            }
            SseEvent::Error { message } => {
                log_error!("[Client] Chat SSE Error | {}", message);
                last_turn.cards.push(Card {
                    id: format!("error-{}", last_turn.cards.len()),
                    kind: CardKind::Error,
                    title: "Error".to_string(),
                    content: message.clone(),
                    full_content: None,
                    right_content: None,
                    state: CardState::Completed,
                });
            }
            SseEvent::MessageDetails { blocks } => {
                // 将 MessageDetails 同步到旧 messages 列表的最后一条 Assistant 消息（兼容旧逻辑）
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.details = Some(blocks.clone());
                    }
                }
            }
            _ => {}
        }
    }

    /// 设置生成状态：开始生成时显示 spinner，结束时重置动画。
    pub fn set_generating(&mut self, generating: bool) {
        log_debug!("[Client] Chat set_generating | {} -> {}", self.is_generating, generating);
        self.is_generating = generating;
        if !generating {
            self.spinner_frame = 0;
            // 标记最后一轮为完成
            if let Some(last) = self.turns.last_mut() {
                last.is_complete = true;
            }
        }
    }

    /// 处理卡片动作（展开/折叠/重试）。
    pub fn handle_card_action(&mut self, action: &CardAction) {
        match action {
            CardAction::Expand(card_id) => {
                if let Some(card) = self.find_card_by_id_mut(card_id) {
                    if let Some(ref full) = card.full_content {
                        card.content = full.clone();
                        card.state = CardState::Expanded;
                    }
                }
            }
            CardAction::Collapse(card_id) => {
                if let Some(card) = self.find_card_by_id_mut(card_id) {
                    let truncated: String = card.content.chars().take(200).collect();
                    card.content = format!("{}...", truncated);
                    card.state = CardState::Collapsed;
                }
            }
            CardAction::Retry(_card_id) => {
                // Retry 逻辑由 App 层处理（通过 RetryTurn 事件）
            }
        }
    }

    /// 根据卡片 ID 查找其所在 Turn 的索引。
    pub fn find_turn_index_by_card_id(&self, card_id: &str) -> Option<usize> {
        self.turns
            .iter()
            .enumerate()
            .find(|(_, t)| t.cards.iter().any(|c| c.id == card_id))
            .map(|(i, _)| i)
    }

    /// 准备重试指定 Turn：标记为完成并移除错误卡片，返回用户消息。
    pub fn retry_turn(&mut self, turn_index: usize) -> Option<String> {
        let turn = self.turns.get_mut(turn_index)?;
        turn.is_complete = true;
        turn.cards.retain(|c| !matches!(c.kind, CardKind::Error));
        Some(turn.user_message.clone())
    }

    /// 查找指定 ID 的卡片（不可变）。
    fn find_card_by_id(&self, card_id: &str) -> Option<&Card> {
        self.turns
            .iter()
            .flat_map(|t| t.cards.iter())
            .find(|c| c.id == card_id)
    }

    /// 查找指定 ID 的卡片（可变）。
    fn find_card_by_id_mut(&mut self, card_id: &str) -> Option<&mut Card> {
        self.turns
            .iter_mut()
            .flat_map(|t| t.cards.iter_mut())
            .find(|c| c.id == card_id)
    }

    /// 向上滚动一页。
    fn handle_page_up(&mut self) -> Option<AppEvent> {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
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
            for card in &turn.cards {
                height += CardWidget::new(card).calculate_height(width);
            }
        }
        for msg in &self.messages {
            height += 1; // 前缀
            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            height += content_para.line_count(width).max(1) as u16;
            height += 1; // 空行
        }
        if self.is_generating {
            height += 1; // spinner
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
        frame.render_widget(block, area);

        let scroll_y = self.scroll_offset as u16;
        let mut current_y = 0u16; // 虚拟 Y 坐标（相对于内容顶部）
        let bottom = inner.y + inner.height;

        // 辅助函数：计算实际渲染 Y 坐标
        let render_y = |cy: u16| -> u16 { inner.y.saturating_add(cy.saturating_sub(scroll_y)) };

        // 辅助函数：判断元素是否可见
        let is_visible = |cy: u16, h: u16| -> bool {
            cy + h > scroll_y && cy < scroll_y + inner.height
        };

        // 清除旧的点击区域（本帧会重新收集）
        self.card_hit_areas.borrow_mut().clear();

        for turn in &self.turns {
            // 用户消息前缀
            let prefix_height = 1u16;
            if is_visible(current_y, prefix_height) {
                let y = render_y(current_y);
                let para = Paragraph::new(Line::from(vec![Span::styled(
                    "You",
                    theme.style_user().add_modifier(Modifier::BOLD),
                )]));
                frame.render_widget(
                    para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: prefix_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += prefix_height;

            // 用户消息内容
            let content_para = Paragraph::new(turn.user_message.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if is_visible(current_y, content_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    content_para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: content_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += content_height;

            // 空行
            current_y += 1;

            // 卡片
            for card in &turn.cards {
                let widget = CardWidget::new(card);
                let card_height = widget.calculate_height(inner.width);
                if is_visible(current_y, card_height) {
                    let y = render_y(current_y);
                    let card_area = Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: card_height.min(bottom.saturating_sub(y)),
                    };
                    widget.draw(frame, card_area, theme);
                    self.card_hit_areas.borrow_mut().push((card.id.clone(), card_area));
                }
                current_y += card_height;
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
            if is_visible(current_y, prefix_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(prefix, style)])),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: prefix_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += prefix_height;

            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if is_visible(current_y, content_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    content_para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: content_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += content_height;
            current_y += 1; // 空行
        }

        // Spinner
        if self.is_generating {
            let spinner = SPINNER_FRAMES[self.spinner_frame];
            let spinner_height = 1u16;
            if is_visible(current_y, spinner_height) {
                let y = render_y(current_y);
                let spinner_line = Line::from(vec![
                    Span::styled("◆ AI ", theme.style_brand().add_modifier(Modifier::BOLD)),
                    Span::styled(spinner, theme.style_brand()),
                ]);
                frame.render_widget(
                    Paragraph::new(spinner_line),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: spinner_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
        }

        // 存储点击区域供 handle_event 使用
        // 注意：由于 self 不可变，这里无法直接修改 self.card_hit_areas
        // 我们通过 UnsafeCell 或其他方式绕过的成本太高。
        // 替代方案：让 Chat 内部使用 RefCell<Vec<...>> 存储点击区域。
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        match event {
            Event::Mouse(mouse) => {
                use crossterm::event::{MouseButton, MouseEventKind};
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    for (card_id, rect) in self.card_hit_areas.borrow().iter() {
                        if rect_contains(*rect, mouse.column, mouse.row) {
                            if let Some(card) = self.find_card_by_id(card_id) {
                                if let Some(action) =
                                    CardWidget::new(card).handle_click(mouse.column, mouse.row, *rect)
                                {
                                    return Some(AppEvent::CardAction(action));
                                }
                            }
                        }
                    }
                }
                None
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
                        self.scroll_offset += 1;
                        Some(AppEvent::ScrollDown)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// 判断点 (x, y) 是否在矩形 rect 内。
fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_user_message_creates_turn() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        assert_eq!(chat.turns.len(), 1);
        assert_eq!(chat.turns[0].user_message, "hello");
    }

    #[test]
    fn test_sse_message_creates_summary_card() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.handle_sse_event(&SseEvent::Message {
            content: "world".to_string(),
        });
        assert_eq!(chat.turns[0].cards.len(), 1);
        assert!(matches!(chat.turns[0].cards[0].kind, CardKind::Summary));
        assert_eq!(chat.turns[0].cards[0].content, "world");
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
    fn test_tool_use_creates_tool_card() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::ToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"cmd": "ls"}),
        });
        assert_eq!(chat.turns[0].cards.len(), 1);
        assert!(matches!(chat.turns[0].cards[0].kind, CardKind::ToolUse { .. }));
    }

    #[test]
    fn test_tool_result_updates_card() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::ToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"cmd": "ls"}),
        });
        chat.handle_sse_event(&SseEvent::ToolResult {
            tool_use_id: "tool_1".to_string(),
            content: "file.txt".to_string(),
            diff: None,
            is_new_file: false,
        });
        assert_eq!(chat.turns[0].cards.len(), 1);
        assert!(matches!(chat.turns[0].cards[0].kind, CardKind::ToolResult));
    }

    #[test]
    fn test_card_expand_collapse() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.turns[0].cards.push(Card {
            id: "c1".to_string(),
            kind: CardKind::ToolResult,
            title: "Result".to_string(),
            content: "short".to_string(),
            full_content: Some("this is the full content".to_string()),
            right_content: None,
            state: CardState::Collapsed,
        });

        chat.handle_card_action(&CardAction::Expand("c1".to_string()));
        assert_eq!(chat.turns[0].cards[0].content, "this is the full content");
        assert_eq!(chat.turns[0].cards[0].state, CardState::Expanded);

        chat.handle_card_action(&CardAction::Collapse("c1".to_string()));
        assert!(chat.turns[0].cards[0].content.ends_with("..."));
        assert_eq!(chat.turns[0].cards[0].state, CardState::Collapsed);
    }
}
