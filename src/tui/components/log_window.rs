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
use crate::tui::event::{AppEvent, LogLevel, LogLine};
use crate::tui::theme::Theme;

/// 日志窗口组件，显示应用日志并支持滚动查看。
pub struct LogWindow {
    visible: bool,
    lines: Vec<LogLine>,
    scroll_offset: usize, // 0 = bottom (newest)
    auto_scroll: bool,
    disconnected: bool,
}

/// 内存中保留的最大日志行数。
const MAX_LINES: usize = 5000;

impl LogWindow {
    /// 创建一个新的日志窗口，默认不可见且自动滚动到底部。
    pub fn new() -> Self {
        Self {
            visible: false,
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            disconnected: false,
        }
    }

    /// 设置窗口是否可见。
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// 获取当前可见状态。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 批量替换日志内容。
    pub fn set_lines(&mut self, lines: Vec<LogLine>) {
        self.lines = lines;
        if self.lines.len() > MAX_LINES {
            self.lines.drain(..self.lines.len() - MAX_LINES);
        }
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    /// 追加一行日志。超出最大行数时自动丢弃最旧的内容。
    pub fn append(&mut self, line: LogLine) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            self.lines.remove(0);
        }
        if self.auto_scroll {
            self.scroll_offset = 0;
        } else {
            self.scroll_offset += 1;
        }
    }

    /// 设置连接断开状态，用于在顶部显示警告。
    pub fn set_disconnected(&mut self, disconnected: bool) {
        self.disconnected = disconnected;
    }

    /// 向上滚动指定行数（远离底部）。
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(delta);
        self.auto_scroll = false;
    }

    /// 向下滚动指定行数（靠近底部）。
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(delta);
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// 根据日志级别返回对应的颜色。
    fn level_color(&self, level: LogLevel, theme: &Theme) -> ratatui::style::Color {
        match level {
            LogLevel::Info => theme.text_primary,
            LogLevel::Debug => theme.text_secondary,
            LogLevel::Trace => theme.text_muted,
            LogLevel::Error => theme.error,
        }
    }
}

impl Component for LogWindow {
    /// 渲染日志窗口：绘制边框、可选的断开连接警告，以及带颜色分级的日志行。
    ///
    /// 仅当 `visible == true` 时才会实际绘制。
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        if !self.visible {
            return;
        }

        let border_type = if is_focused {
            ratatui::widgets::BorderType::Double
        } else {
            ratatui::widgets::BorderType::Plain
        };

        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut draw_lines: Vec<Line> = Vec::new();

        // 如果连接断开，在顶部显示红色警告
        if self.disconnected {
            draw_lines.push(Line::from(vec![Span::styled(
                "⚠ 日志连接已断开",
                Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
            )]));
        }

        let warning_lines = if self.disconnected { 1 } else { 0 };
        let available_height = inner.height.saturating_sub(warning_lines) as usize;

        if available_height > 0 && !self.lines.is_empty() {
            let end_idx = self.lines.len().saturating_sub(self.scroll_offset);
            let start_idx = end_idx.saturating_sub(available_height);
            let visible_lines = &self.lines[start_idx..end_idx];

            for line in visible_lines {
                let level_color = self.level_color(line.level, theme);
                let spans = vec![
                    Span::styled(
                        format!("[{}] ", line.timestamp),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::styled(
                        format!("[{:?}] ", line.level),
                        Style::default().fg(level_color),
                    ),
                    Span::styled(
                        format!("[{}] ", line.module),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::styled(&line.message, Style::default().fg(theme.text_primary)),
                ];
                draw_lines.push(Line::from(spans));
            }
        }

        let paragraph = Paragraph::new(draw_lines).style(theme.style_primary());
        frame.render_widget(paragraph, inner);
    }

    /// 处理键盘事件：方向键和翻页键控制滚动。
    ///
    /// 仅当窗口可见时才会响应按键；所有事件均返回 `None`，不向上层发送 `AppEvent`。
    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if !self.visible {
            return None;
        }

        let Event::Key(key) = event else { return None };
        if key.kind != KeyEventKind::Press {
            return None;
        }

        match key.code {
            KeyCode::Up => {
                self.scroll_up(1);
            }
            KeyCode::Down => {
                self.scroll_down(1);
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
            }
            _ => {}
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(level: LogLevel, message: &str) -> LogLine {
        LogLine {
            timestamp: "12:34:56".to_string(),
            level,
            module: "test".to_string(),
            message: message.to_string(),
        }
    }

    #[test]
    fn test_new_log_window() {
        let window = LogWindow::new();
        assert!(!window.is_visible());
        assert!(window.lines.is_empty());
        assert_eq!(window.scroll_offset, 0);
        assert!(window.auto_scroll);
        assert!(!window.disconnected);
    }

    #[test]
    fn test_set_visible() {
        let mut window = LogWindow::new();
        window.set_visible(true);
        assert!(window.is_visible());
        window.set_visible(false);
        assert!(!window.is_visible());
    }

    #[test]
    fn test_append_and_max_lines() {
        let mut window = LogWindow::new();
        for i in 0..MAX_LINES + 100 {
            window.append(make_line(LogLevel::Info, &format!("msg {}", i)));
        }
        assert_eq!(window.lines.len(), MAX_LINES);
        assert_eq!(window.lines[0].message, "msg 100");
    }

    #[test]
    fn test_scroll_up_down() {
        let mut window = LogWindow::new();
        window.append(make_line(LogLevel::Info, "a"));
        window.append(make_line(LogLevel::Info, "b"));
        window.scroll_up(1);
        assert_eq!(window.scroll_offset, 1);
        assert!(!window.auto_scroll);
        window.scroll_down(1);
        assert_eq!(window.scroll_offset, 0);
        assert!(window.auto_scroll);
    }

    #[test]
    fn test_scroll_down_clamps_to_zero() {
        let mut window = LogWindow::new();
        window.scroll_up(5);
        window.scroll_down(10);
        assert_eq!(window.scroll_offset, 0);
        assert!(window.auto_scroll);
    }

    #[test]
    fn test_set_lines() {
        let mut window = LogWindow::new();
        let lines = vec![
            make_line(LogLevel::Info, "first"),
            make_line(LogLevel::Debug, "second"),
        ];
        window.set_lines(lines);
        assert_eq!(window.lines.len(), 2);
    }

    #[test]
    fn test_set_disconnected() {
        let mut window = LogWindow::new();
        window.set_disconnected(true);
        assert!(window.disconnected);
    }

    #[test]
    fn test_append_updates_scroll_when_not_auto_scroll() {
        let mut window = LogWindow::new();
        window.scroll_up(1); // auto_scroll becomes false
        window.append(make_line(LogLevel::Info, "hello"));
        assert_eq!(window.scroll_offset, 2);
    }
}
