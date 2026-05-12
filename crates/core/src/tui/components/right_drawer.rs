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
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 会话元信息。
#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool, // 是否为当前活跃会话
}

/// 右侧常驻栏组件，展示 Task 完成情况和本次会话变更文件。
///
/// 当前为占位实现，等后端 API 提供 Task 和变更文件数据后填充真实内容。
pub struct RightDrawer {
    sessions: Vec<SessionMeta>,
    selected_index: usize,
    filter: String,      // 预留：会话名称过滤
    filter_active: bool, // 预留：是否处于过滤模式
}

impl RightDrawer {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected_index: 0,
            filter: String::new(),
            filter_active: false,
        }
    }

    /// 设置会话列表并重置选中位置。
    pub fn set_sessions(&mut self, sessions: Vec<SessionMeta>) {
        self.sessions = sessions;
        self.selected_index = 0;
    }
}

impl Component for RightDrawer {
    /// 渲染会话历史抽屉：显示会话名称、消息数量、当前会话指示器（●），
    /// 选中项使用反色高亮，当前会话使用品牌色。
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
            .title("Tasks & Changes")
            .style(theme.drawer_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // 上下两个区块：Tasks（上）和 Changes（下）
        let half_height = inner.height / 2;
        let top_area = Rect::new(inner.x, inner.y, inner.width, half_height);
        let bottom_area = Rect::new(
            inner.x,
            inner.y + half_height,
            inner.width,
            inner.height - half_height,
        );

        // 上半区：Tasks
        let tasks_lines = vec![
            Line::styled(
                "📋 Tasks",
                theme.style_primary().add_modifier(Modifier::BOLD),
            ),
            Line::styled("  No active tasks", theme.style_muted()),
        ];
        frame.render_widget(Paragraph::new(tasks_lines), top_area);

        // 下半区：Changes
        let changes_lines = vec![
            Line::styled(
                "📁 Changes",
                theme.style_primary().add_modifier(Modifier::BOLD),
            ),
            Line::styled("  No changes yet", theme.style_muted()),
        ];
        frame.render_widget(Paragraph::new(changes_lines), bottom_area);
    }

    /// 右侧常驻栏当前为占位展示，不处理导航事件。
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_navigation() {
        let mut drawer = RightDrawer::new();
        drawer.set_sessions(vec![
            SessionMeta {
                id: "1".to_string(),
                name: "test1".to_string(),
                last_active: "".to_string(),
                message_count: 5,
                is_current: true,
            },
            SessionMeta {
                id: "2".to_string(),
                name: "test2".to_string(),
                last_active: "".to_string(),
                message_count: 3,
                is_current: false,
            },
        ]);

        assert_eq!(drawer.selected_index, 0);
    }
}
