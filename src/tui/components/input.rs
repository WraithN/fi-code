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
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::commands::registry::CommandMeta;
use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 底部输入框组件，处理用户键盘输入、光标管理、斜杠命令提示与消息提交。
pub struct Input {
    content: String,
    cursor_position: usize,       // 光标在 content 中的字节位置
    dropdown_visible: bool,       // 斜杠命令下拉菜单是否可见
    dropdown_items: Vec<CommandMeta>,
    dropdown_selected: usize,
    session_id: Option<String>,   // 当前会话 ID，用于在输入框上方显示
    last_drawn_area: Option<Rect>,
    dropdown_area: Option<Rect>,
    commands_loaded: bool,
}

impl Input {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            dropdown_visible: false,
            dropdown_items: Vec::new(),
            dropdown_selected: 0,
            session_id: None,
            last_drawn_area: None,
            dropdown_area: None,
            commands_loaded: false,
        }
    }

    pub fn set_session_id(&mut self, id: Option<String>) {
        self.session_id = id;
    }

    pub fn set_commands(&mut self, commands: Vec<CommandMeta>) {
        self.dropdown_items = commands;
        self.commands_loaded = true;
        if self.content == "/" {
            self.dropdown_visible = true;
            self.dropdown_selected = 0;
        }
    }

    pub fn is_dropdown_visible(&self) -> bool {
        self.dropdown_visible
    }

    pub fn set_last_drawn_area(&mut self, area: Rect) {
        self.last_drawn_area = Some(area);
    }

    pub fn update_dropdown_area(&mut self, input_area: Rect) {
        if !self.dropdown_visible || self.dropdown_items.is_empty() {
            self.dropdown_area = None;
            return;
        }
        let items_len = self.dropdown_items.len() as u16;
        let height = items_len + 2;
        let width = 40u16.min(input_area.width);
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height);
        self.dropdown_area = Some(Rect::new(x, y, width, height));
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_position = self.content.len();
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.cursor_position = pos;
    }

    pub fn close_dropdown(&mut self) {
        self.dropdown_visible = false;
    }

    pub fn clear_content(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
        self.dropdown_visible = false;
    }

    /// 输入框显示行数（固定 2 行）。
    pub fn visible_lines(&self) -> u16 {
        2
    }

    /// 在光标位置插入字符，并向后移动光标。
    fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// 删除光标前一个字符，并将光标前移。
    fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            // 通过 char_indices 找到前一个字符的起始字节位置
            let prev_pos = self.content[..self.cursor_position]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    /// 检测是否触发斜杠命令下拉菜单：仅当内容恰好为 "/" 时显示。
    fn check_slash_commands(&mut self) -> Option<AppEvent> {
        if self.content == "/" {
            if self.commands_loaded {
                self.dropdown_visible = true;
                self.dropdown_selected = 0;
                None
            } else {
                Some(AppEvent::LoadCommands)
            }
        } else if !self.content.starts_with('/') {
            self.dropdown_visible = false;
            None
        } else {
            None
        }
    }
}

impl Component for Input {
    /// 渲染输入框：包含可选的会话 ID 标签、带边框的输入区域、placeholder、光标位置，
    /// 以及斜杠命令下拉菜单。
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        // 在输入框上方显示会话 ID
        let mut y_offset = 0u16;
        if let Some(ref id) = self.session_id {
            let session_label = format!("--[Session: {}]---", id);
            let label_rect = Rect::new(area.x, area.y, area.width, 1);
            let label = Paragraph::new(session_label)
                .style(theme.style_muted());
            frame.render_widget(label, label_rect);
            y_offset = 1;
        }

        let placeholder = if self.content.is_empty() {
            "Type your message, or paste code..."
        } else {
            ""
        };

        let border_type = if is_focused {
            ratatui::widgets::BorderType::Double
        } else {
            ratatui::widgets::BorderType::Plain
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(theme.border))
            .style(theme.input_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.content.is_empty() {
            let text = Paragraph::new(placeholder).style(
                Style::default()
                    .fg(theme.text_placeholder)
                    .bg(theme.bg_surface),
            );
            frame.render_widget(text, inner);
        } else {
            let text = Paragraph::new(self.content.as_str())
                .style(theme.style_primary().bg(theme.bg_surface));
            frame.render_widget(text, inner);
        }

        // 计算光标在终端上的绝对坐标（支持多行换行）
        let text_before_cursor = &self.content[..self.cursor_position];
        let lines: Vec<&str> = text_before_cursor.split('\n').collect();
        let cursor_row = lines.len().saturating_sub(1) as u16;
        let cursor_col = lines.last().unwrap_or(&"").chars().count() as u16;
        let cursor_x = inner.x + cursor_col;
        let cursor_y = inner.y + cursor_row;
        frame.set_cursor_position((cursor_x, cursor_y));

        if self.dropdown_visible && !self.dropdown_items.is_empty() {
            self.draw_dropdown(frame, area, theme);
        }
    }

    /// 处理输入框事件：支持下拉菜单导航、普通字符输入、回车提交、Shift+Enter 换行、退格删除。
    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return None;
                }

                if self.dropdown_visible {
                    match key.code {
                        KeyCode::Up => {
                            if self.dropdown_selected > 0 {
                                self.dropdown_selected -= 1;
                            }
                            return None;
                        }
                        KeyCode::Down => {
                            if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
                                self.dropdown_selected += 1;
                            }
                            return None;
                        }
                        KeyCode::Enter => {
                            if let Some(cmd) = self.dropdown_items.get(self.dropdown_selected) {
                                return Some(AppEvent::ExecuteSlashCommand {
                                    name: cmd.name.clone(),
                                    args_hint: cmd.args_hint.clone(),
                                });
                            }
                        }
                        KeyCode::Esc => {
                            self.dropdown_visible = false;
                            return None;
                        }
                        _ => {}
                    }
                }

                match (key.modifiers, key.code) {
                    (KeyModifiers::SHIFT, KeyCode::Enter) => {
                        self.insert_char('\n');
                        let ev = self.check_slash_commands();
                        if ev.is_some() {
                            return ev;
                        }
                        return Some(AppEvent::InputChanged(self.content.clone()));
                    }
                    (KeyModifiers::NONE, KeyCode::Enter) => {
                        if !self.content.trim().is_empty() {
                            let msg = self.content.clone();
                            self.content.clear();
                            self.cursor_position = 0;
                            self.dropdown_visible = false;
                            return Some(AppEvent::SubmitMessage(msg));
                        }
                    }
                    (KeyModifiers::NONE, KeyCode::Char(c)) => {
                        self.insert_char(c);
                        let ev = self.check_slash_commands();
                        if ev.is_some() {
                            return ev;
                        }
                        return Some(AppEvent::InputChanged(self.content.clone()));
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) => {
                        self.delete_char();
                        if self.content.is_empty() {
                            self.dropdown_visible = false;
                        }
                        return Some(AppEvent::InputChanged(self.content.clone()));
                    }
                    _ => {}
                }
                None
            }
            Event::Mouse(mouse) => {
                if !self.dropdown_visible {
                    return None;
                }
                match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollUp => {
                        if self.dropdown_selected > 0 {
                            self.dropdown_selected -= 1;
                        }
                        None
                    }
                    crossterm::event::MouseEventKind::ScrollDown => {
                        if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
                            self.dropdown_selected += 1;
                        }
                        None
                    }
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        if let Some(area) = self.dropdown_area {
                            let mx = mouse.column;
                            let my = mouse.row;
                            if mx >= area.x && mx < area.x + area.width
                                && my >= area.y && my < area.y + area.height
                            {
                                let item_y = my.saturating_sub(area.y + 1);
                                let index = item_y as usize;
                                if index < self.dropdown_items.len() {
                                    self.dropdown_selected = index;
                                    let cmd = &self.dropdown_items[index];
                                    return Some(AppEvent::ExecuteSlashCommand {
                                        name: cmd.name.clone(),
                                        args_hint: cmd.args_hint.clone(),
                                    });
                                }
                            } else {
                                self.dropdown_visible = false;
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl Input {
    /// 渲染斜杠命令下拉菜单：显示在输入框上方，包含命令名与描述。
    fn draw_dropdown(&self, frame: &mut Frame, input_area: Rect, theme: &Theme) {
        let items: Vec<Line> = self
            .dropdown_items
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let style = if i == self.dropdown_selected {
                    theme.style_selection()
                } else {
                    theme.style_primary()
                };
                Line::from(vec![
                    Span::styled(format!("/{}", cmd.name), style.add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!(" - {}", cmd.description),
                        style,
                    ),
                ])
            })
            .collect();

        let height = items.len() as u16 + 2;
        let width = 40u16.min(input_area.width);
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height);

        let area = Rect::new(x, y, width, height);

        let paragraph = Paragraph::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(theme.drawer_style()),
        );
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_delete() {
        let mut input = Input::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.content, "hi");
        assert_eq!(input.cursor_position, 2);

        input.delete_char();
        assert_eq!(input.content, "h");
        assert_eq!(input.cursor_position, 1);
    }

    #[test]
    fn test_multiline_lines() {
        let mut input = Input::new();
        input.insert_char('a');
        input.insert_char('\n');
        input.insert_char('b');
        assert_eq!(input.visible_lines(), 2);
    }

    #[test]
    fn test_slash_command_detection() {
        let mut input = Input::new();
        input.set_commands(vec![
            CommandMeta { name: "clear".into(), description: "Clear".into(), args_hint: None },
        ]);
        input.insert_char('/');
        input.check_slash_commands();
        assert!(input.dropdown_visible);

        input.content.clear();
        input.check_slash_commands();
        assert!(!input.dropdown_visible);
    }
}
