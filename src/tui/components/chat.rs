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

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::server::sse::SseEvent;
use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 聊天消息结构。
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// 消息发送者角色。
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,       // 用户
    Assistant,  // AI 助手
    System,     // 系统提示
    Error,      // 错误信息
}

/// 聊天组件，负责显示对话历史、处理 SSE 流式消息、渲染生成动画。
pub struct Chat {
    messages: Vec<Message>,      // 消息列表
    scroll_offset: usize,        // 垂直滚动偏移（以行为单位）
    is_generating: bool,         // 是否正在生成回复
    spinner_frame: usize,        // 当前 spinner 动画帧索引
}

/// 终端 spinner 动画帧（Braille 点阵字符），每 tick 轮播一帧。
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// 将 SSE 内容追加到上一条 Assistant 消息；若上一条不是 Assistant，则新建一条。
fn append_assistant_message(messages: &mut Vec<Message>, content: &str) {
    if let Some(last) = messages.last_mut() {
        if last.role == MessageRole::Assistant {
            last.content.push_str(content);
            return;
        }
    }
    messages.push(Message {
        role: MessageRole::Assistant,
        content: content.to_string(),
    });
}

impl Chat {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            is_generating: false,
            spinner_frame: 0,
        }
    }

    /// 添加一条用户发送的消息。
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.to_string(),
        });
    }

    /// 添加一条系统消息。
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
        });
    }

    /// 清空所有消息。
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// 定时 tick：若正在生成回复，则推进 spinner 动画帧。
    pub fn on_tick(&mut self) {
        if self.is_generating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// 处理 SSE 事件：将流式内容追加到 Assistant 消息，或将错误显示在聊天区。
    pub fn handle_sse_event(&mut self, event: &SseEvent) {
        match event {
            SseEvent::Message { content } => {
                append_assistant_message(&mut self.messages, content);
            }
            SseEvent::Error { message } => {
                self.messages.push(Message {
                    role: MessageRole::Error,
                    content: message.clone(),
                });
            }
            _ => {}
        }
    }

    /// 设置生成状态：开始生成时显示 spinner，结束时重置动画。
    pub fn set_generating(&mut self, generating: bool) {
        self.is_generating = generating;
        if !generating {
            self.spinner_frame = 0;
        }
    }

    /// 向上滚动一页。
    fn handle_page_up(&mut self) -> Option<AppEvent> {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        Some(AppEvent::ScrollUp)
    }
}

impl Component for Chat {
    /// 渲染聊天区：绘制边框、消息列表，以及底部的生成中 spinner。
    ///
    /// 每条消息前会显示角色前缀（You / ◆ AI / ℹ️ / ❌），
    /// 消息内容按行拆分后逐行渲染，支持自动换行与滚动。
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        let border_type = if is_focused {
            ratatui::widgets::BorderType::Double
        } else {
            ratatui::widgets::BorderType::Plain
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.messages {
            let (prefix, style) = match msg.role {
                MessageRole::User => ("You", theme.style_user().add_modifier(Modifier::BOLD)),
                MessageRole::Assistant => {
                    ("◆ AI", theme.style_brand().add_modifier(Modifier::BOLD))
                }
                MessageRole::System => ("ℹ️ ", Style::default().fg(theme.warning)),
                MessageRole::Error => ("❌ ", Style::default().fg(theme.error)),
            };

            lines.push(Line::from(vec![Span::styled(prefix, style)]));

            for text_line in msg.content.lines() {
                lines.push(Line::from(Span::styled(text_line, theme.style_primary())));
            }

            lines.push(Line::from(""));
        }

        if self.is_generating {
            let spinner = SPINNER_FRAMES[self.spinner_frame];
            lines.push(Line::from(vec![
                Span::styled("◆ AI ", theme.style_brand().add_modifier(Modifier::BOLD)),
                Span::styled(spinner, theme.style_brand()),
            ]));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        frame.render_widget(paragraph, inner);
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        let Event::Key(key) = event else { return None };
        if key.kind != KeyEventKind::Press {
            return None;
        }
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.handle_page_up()
            }
            (KeyModifiers::CONTROL, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.scroll_offset += 1;
                Some(AppEvent::ScrollDown)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_message() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, MessageRole::User);
    }

    #[test]
    fn test_sse_message_appends() {
        let mut chat = Chat::new();
        chat.handle_sse_event(&SseEvent::Message {
            content: "Hello".to_string(),
        });
        chat.handle_sse_event(&SseEvent::Message {
            content: " world".to_string(),
        });
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "Hello world");
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
        assert!(chat.messages.is_empty());
    }
}
