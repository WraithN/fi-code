// MIT License
// Copyright (c) 2025 fi-code contributors

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::components::Component;
use crate::theme::Theme;
use fi_code_shared::tui_event::AppEvent;

/// 会话元信息。
#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool,
}

/// 右侧常驻栏组件，展示 Task 完成情况和本次会话变更文件。
pub struct RightDrawer {
    sessions: Vec<SessionMeta>,
    selected_index: usize,
    scroll_offset: usize,
}

impl RightDrawer {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(delta);
    }

    pub fn scroll_down(&mut self, delta: usize) {
        let max = self.sessions.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + delta).min(max);
    }

    /// 设置会话列表并重置选中位置。
    pub fn set_sessions(&mut self, sessions: Vec<SessionMeta>) {
        self.sessions = sessions;
        self.selected_index = 0;
    }
}

impl Component for RightDrawer {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title("Tasks & Changes")
            .style(theme.drawer_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let viewport_height = inner.height as usize;

        let mut all_lines = vec![
            Line::styled(
                "📋 Tasks",
                theme.style_primary().add_modifier(Modifier::BOLD),
            ),
            Line::styled("  No active tasks", theme.style_muted()),
            Line::styled("", theme.style_primary()),
            Line::styled(
                "📁 Changes",
                theme.style_primary().add_modifier(Modifier::BOLD),
            ),
            Line::styled("  No changes yet", theme.style_muted()),
        ];

        for (i, session) in self.sessions.iter().enumerate() {
            if i == 0 {
                all_lines.push(Line::styled("", theme.style_primary()));
                all_lines.push(Line::styled(
                    "📝 Sessions",
                    theme.style_primary().add_modifier(Modifier::BOLD),
                ));
            }
            let marker = if session.is_current { "● " } else { "○ " };
            let style = if i == self.selected_index {
                theme.style_selection()
            } else {
                theme.style_primary()
            };
            all_lines.push(Line::styled(
                format!("  {}{} ({})", marker, session.name, session.message_count),
                style,
            ));
        }

        let visible_lines: Vec<Line> = all_lines
            .into_iter()
            .skip(self.scroll_offset)
            .take(viewport_height)
            .collect();

        frame.render_widget(Paragraph::new(visible_lines), inner);

        let total_lines = self.sessions.len() + 5;
        if total_lines > viewport_height {
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(total_lines.saturating_sub(1))
                .position(self.scroll_offset)
                .viewport_content_length(viewport_height);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .style(Style::default().fg(theme.border));

            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    self.scroll_up(1);
                    None
                }
                KeyCode::Down => {
                    self.scroll_down(1);
                    None
                }
                _ => None,
            }
        } else {
            None
        }
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
