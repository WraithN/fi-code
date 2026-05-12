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

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::{LogLevel, LogLine};
use crate::tui::theme::Theme;

/// 日志浮窗组件，用于实时显示应用运行日志。
pub struct LogWindow {
    visible: bool,
    lines: Vec<LogLine>,
    scroll_offset: usize, // 0 = 底部（最新）
    auto_scroll: bool,
    disconnected: bool,
}

impl LogWindow {
    pub fn new() -> Self {
        Self {
            visible: false,
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            disconnected: false,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.scroll_offset = 0;
            self.auto_scroll = true;
            self.disconnected = false;
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_lines(&mut self, lines: Vec<LogLine>) {
        self.lines = lines;
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    pub fn append(&mut self, line: LogLine) {
        self.lines.push(line);
        const MAX_LINES: usize = 5000;
        if self.lines.len() > MAX_LINES {
            self.lines.drain(..self.lines.len() - MAX_LINES);
        }
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    pub fn set_disconnected(&mut self, disconnected: bool) {
        self.disconnected = disconnected;
    }

    pub fn scroll_up(&mut self, delta: usize) {
        let max_offset = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + delta).min(max_offset);
        self.auto_scroll = self.scroll_offset == 0;
    }

    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(delta);
        self.auto_scroll = self.scroll_offset == 0;
    }
}

impl Component for LogWindow {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        if !self.visible {
            return;
        }

        let mut text_lines: Vec<Line> = Vec::new();

        if self.disconnected {
            text_lines.push(Line::from(vec![Span::styled(
                "⚠ 日志连接已断开",
                Style::default().fg(theme.error),
            )]));
        }

        let visible_height = area.height.saturating_sub(2) as usize; // 减去边框
        let start = if self.scroll_offset == 0 {
            self.lines.len().saturating_sub(visible_height)
        } else {
            self.lines
                .len()
                .saturating_sub(visible_height + self.scroll_offset)
        };
        let start = start.min(self.lines.len());
        let end = (start + visible_height).min(self.lines.len());

        for line in &self.lines[start..end] {
            let level_color = match line.level {
                LogLevel::Info => theme.text_primary,
                LogLevel::Debug => theme.text_secondary,
                LogLevel::Trace => theme.text_muted,
                LogLevel::Error => theme.error,
            };
            let level_str = match line.level {
                LogLevel::Info => "INFO ",
                LogLevel::Debug => "DEBUG",
                LogLevel::Trace => "TRACE",
                LogLevel::Error => "ERROR",
            };
            text_lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", line.timestamp),
                    Style::default().fg(theme.text_muted),
                ),
                Span::styled(
                    format!("[{:<5}] ", level_str),
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("[{:<20.20}] ", line.module),
                    Style::default().fg(theme.text_muted),
                ),
                Span::styled(&line.message, Style::default().fg(theme.text_primary)),
            ]));
        }

        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(theme.drawer_style());

        let paragraph = Paragraph::new(text_lines).block(block);
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<crate::tui::event::AppEvent> {
        if !self.visible {
            return None;
        }
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    self.scroll_up(1);
                    return None;
                }
                KeyCode::Down => {
                    self.scroll_down(1);
                    return None;
                }
                KeyCode::PageUp => {
                    self.scroll_up(10);
                    return None;
                }
                KeyCode::PageDown => {
                    self.scroll_down(10);
                    return None;
                }
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_window_append_and_scroll() {
        let mut window = LogWindow::new();
        window.set_visible(true);
        for i in 0..5 {
            window.append(LogLine {
                timestamp: "12:00:00".into(),
                level: LogLevel::Info,
                module: "test".into(),
                message: format!("msg{}", i),
            });
        }
        assert_eq!(window.lines.len(), 5);
        window.scroll_up(2);
        assert_eq!(window.scroll_offset, 2);
        window.scroll_down(1);
        assert_eq!(window.scroll_offset, 1);
        window.scroll_down(10);
        assert_eq!(window.scroll_offset, 0);
    }

    #[test]
    fn test_log_window_max_lines() {
        let mut window = LogWindow::new();
        for i in 0..6000 {
            window.append(LogLine {
                timestamp: "12:00:00".into(),
                level: LogLevel::Info,
                module: "test".into(),
                message: format!("{}", i),
            });
        }
        assert_eq!(window.lines.len(), 5000);
    }
}
