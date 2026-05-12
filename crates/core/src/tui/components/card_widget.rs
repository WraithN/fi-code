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

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::server::transport::sse::TaskProgressItem;
use crate::tui::event::CardAction;
use crate::tui::theme::Theme;

/// 卡片数据结构，表示聊天界面中的一个结构化信息块。
#[derive(Debug, Clone)]
pub struct Card {
    pub id: String,
    pub kind: CardKind,
    pub title: String,
    pub content: String,
    pub full_content: Option<String>,
    pub right_content: Option<String>,
    pub state: CardState,
}

/// 卡片类型枚举。
#[derive(Debug, Clone)]
pub enum CardKind {
    Thinking,
    ToolUse { name: String },
    ToolResult,
    WriteFile { path: String },
    TodoList {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
    Summary,
    Error,
}

/// 卡片状态枚举。
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    Animating,
    Collapsed,
    Expanded,
    Completed,
}

/// 卡片渲染组件。
pub struct CardWidget<'a> {
    card: &'a Card,
}

impl<'a> CardWidget<'a> {
    pub fn new(card: &'a Card) -> Self {
        Self { card }
    }

    /// 计算卡片在给定宽度下的渲染高度。
    pub fn calculate_height(&self, width: u16) -> u16 {
        let title_height = 1;
        let content_lines = self.card.content.lines().count() as u16;
        let footer_height = if self.show_footer() { 1 } else { 0 };
        let padding = 2; // top/bottom border
        title_height + content_lines.min(20) + footer_height + padding
    }

    fn show_footer(&self) -> bool {
        (matches!(self.card.state, CardState::Collapsed | CardState::Expanded)
            && self.card.full_content.is_some())
            || matches!(self.card.kind, CardKind::Error)
    }

    /// 在指定区域绘制卡片。
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Split inner area: title (1) + content (rest-1) + footer (1)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(if self.show_footer() { 1 } else { 0 }),
            ])
            .split(inner);

        // Title bar
        let icon = match &self.card.kind {
            CardKind::Thinking => "🧠",
            CardKind::ToolUse { .. } => "🔧",
            CardKind::ToolResult => "📤",
            CardKind::WriteFile { .. } => "📝",
            CardKind::TodoList { .. } => "📋",
            CardKind::Summary => "◆ AI",
            CardKind::Error => "❌",
        };
        let title_line = Line::from(vec![
            Span::styled(format!("{} ", icon), theme.style_brand()),
            Span::styled(
                &self.card.title,
                theme.style_brand().add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(title_line), chunks[0]);

        // Content area (with optional right panel)
        if self.card.right_content.is_some()
            && !matches!(self.card.kind, CardKind::TodoList { .. })
        {
            let h_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(chunks[1]);

            let content_text = Text::from(self.card.content.clone());
            frame.render_widget(
                Paragraph::new(content_text).wrap(Wrap { trim: true }),
                h_chunks[0],
            );

            let right_text = Text::from(self.card.right_content.clone().unwrap());
            frame.render_widget(
                Paragraph::new(right_text).wrap(Wrap { trim: true }),
                h_chunks[1],
            );
        } else {
            let content_text = Text::from(self.card.content.clone());
            frame.render_widget(
                Paragraph::new(content_text).wrap(Wrap { trim: true }),
                chunks[1],
            );
        }

        // Footer
        if self.show_footer() {
            let footer_text = match &self.card.kind {
                CardKind::Error => "[Retry]",
                _ => {
                    if self.card.state == CardState::Expanded {
                        "−Collapse"
                    } else {
                        "+Expand"
                    }
                }
            };
            let footer_line = Line::from(Span::styled(
                footer_text,
                Style::default()
                    .fg(theme.brand)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            let footer_para = Paragraph::new(footer_line).alignment(Alignment::Right);
            frame.render_widget(footer_para, chunks[2]);
        }
    }

    /// 处理鼠标点击事件，返回对应的 CardAction。
    pub fn handle_click(&self, x: u16, y: u16, rect: Rect) -> Option<CardAction> {
        if !self.show_footer() {
            return None;
        }
        // Calculate footer area
        let footer_y = rect.y + rect.height - 2; // account for border
        let footer_area = Rect {
            x: rect.x + 2,
            y: footer_y,
            width: rect.width - 4,
            height: 1,
        };

        if y == footer_y && x >= footer_area.x && x < footer_area.x + footer_area.width {
            match &self.card.kind {
                CardKind::Error => Some(CardAction::Retry(self.card.id.clone())),
                _ => {
                    if self.card.state == CardState::Expanded {
                        Some(CardAction::Collapse(self.card.id.clone()))
                    } else {
                        Some(CardAction::Expand(self.card.id.clone()))
                    }
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn make_card(kind: CardKind, state: CardState, content: &str, full: Option<&str>) -> Card {
        Card {
            id: "test-card".to_string(),
            kind,
            title: "Test".to_string(),
            content: content.to_string(),
            full_content: full.map(|s| s.to_string()),
            right_content: None,
            state,
        }
    }

    #[test]
    fn test_calculate_height_basic() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "line1\nline2", None);
        let widget = CardWidget::new(&card);
        // title(1) + content(2) + padding(2) = 5, no footer
        assert_eq!(widget.calculate_height(40), 5);
    }

    #[test]
    fn test_calculate_height_with_footer() {
        let card = make_card(
            CardKind::Summary,
            CardState::Collapsed,
            "short",
            Some("full content here"),
        );
        let widget = CardWidget::new(&card);
        // title(1) + content(1) + footer(1) + padding(2) = 5
        assert_eq!(widget.calculate_height(40), 5);
    }

    #[test]
    fn test_calculate_height_capped_at_20() {
        let content = (0..30).map(|i| format!("line{}", i)).collect::<Vec<_>>().join("\n");
        let card = make_card(CardKind::Summary, CardState::Collapsed, &content, None);
        let widget = CardWidget::new(&card);
        // title(1) + min(30, 20) + padding(2) = 23
        assert_eq!(widget.calculate_height(40), 23);
    }

    #[test]
    fn test_show_footer_collapsed_with_full() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "a", Some("b"));
        let widget = CardWidget::new(&card);
        assert!(widget.show_footer());
    }

    #[test]
    fn test_show_footer_expanded_with_full() {
        let card = make_card(CardKind::Summary, CardState::Expanded, "a", Some("b"));
        let widget = CardWidget::new(&card);
        assert!(widget.show_footer());
    }

    #[test]
    fn test_show_footer_no_full_content() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "a", None);
        let widget = CardWidget::new(&card);
        assert!(!widget.show_footer());
    }

    #[test]
    fn test_show_footer_error_always() {
        let card = make_card(CardKind::Error, CardState::Completed, "error", None);
        let widget = CardWidget::new(&card);
        assert!(widget.show_footer());
    }

    #[test]
    fn test_show_footer_animating() {
        let card = make_card(CardKind::Summary, CardState::Animating, "a", Some("b"));
        let widget = CardWidget::new(&card);
        // Animating state should not show footer even with full_content
        assert!(!widget.show_footer());
    }

    #[test]
    fn test_handle_click_outside_footer() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "a", Some("b"));
        let widget = CardWidget::new(&card);
        let rect = Rect::new(0, 0, 20, 10);
        // Click in the middle, not on footer
        let action = widget.handle_click(5, 5, rect);
        assert!(action.is_none());
    }

    #[test]
    fn test_handle_click_expand() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "a", Some("b"));
        let widget = CardWidget::new(&card);
        let rect = Rect::new(0, 0, 20, 10);
        // Footer is at rect.y + rect.height - 2 = 8
        let action = widget.handle_click(5, 8, rect);
        assert!(matches!(action, Some(CardAction::Expand(id)) if id == "test-card"));
    }

    #[test]
    fn test_handle_click_collapse() {
        let card = make_card(CardKind::Summary, CardState::Expanded, "a", Some("b"));
        let widget = CardWidget::new(&card);
        let rect = Rect::new(0, 0, 20, 10);
        let action = widget.handle_click(5, 8, rect);
        assert!(matches!(action, Some(CardAction::Collapse(id)) if id == "test-card"));
    }

    #[test]
    fn test_handle_click_retry() {
        let card = make_card(CardKind::Error, CardState::Collapsed, "error", None);
        let widget = CardWidget::new(&card);
        let rect = Rect::new(0, 0, 20, 10);
        let action = widget.handle_click(5, 8, rect);
        assert!(matches!(action, Some(CardAction::Retry(id)) if id == "test-card"));
    }

    #[test]
    fn test_handle_click_no_footer() {
        let card = make_card(CardKind::Summary, CardState::Collapsed, "a", None);
        let widget = CardWidget::new(&card);
        let rect = Rect::new(0, 0, 20, 10);
        let action = widget.handle_click(5, 8, rect);
        assert!(action.is_none());
    }

    // =============================================================================
    // TestBackend 渲染快照测试
    // =============================================================================

    #[test]
    fn test_render_summary_card() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = crate::tui::theme::Theme::deep_ocean();
        let card = Card {
            id: "c1".to_string(),
            kind: CardKind::Summary,
            title: "AI Response".to_string(),
            content: "Hello, this is a test.".to_string(),
            full_content: None,
            right_content: None,
            state: CardState::Completed,
        };
        let widget = CardWidget::new(&card);

        terminal
            .draw(|f| {
                widget.draw(f, f.area(), &theme);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        // 验证标题出现在第 1 行（边框内）
        let row_text: String = (0..buffer.area().width)
            .map(|x| buffer.get(x, 1).symbol().to_string())
            .collect();
        assert!(row_text.contains("AI"), "Card title should be rendered");

        // 验证内容出现在后续行
        let content_row: String = (0..buffer.area().width)
            .map(|x| buffer.get(x, 2).symbol().to_string())
            .collect();
        assert!(
            content_row.contains("Hello") || content_row.contains("test"),
            "Card content should be rendered"
        );
    }

    #[test]
    fn test_render_error_card_with_footer() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(30, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = crate::tui::theme::Theme::deep_ocean();
        let card = Card {
            id: "c2".to_string(),
            kind: CardKind::Error,
            title: "Error".to_string(),
            content: "Something went wrong".to_string(),
            full_content: None,
            right_content: None,
            state: CardState::Completed,
        };
        let widget = CardWidget::new(&card);

        terminal
            .draw(|f| {
                widget.draw(f, f.area(), &theme);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        // 验证错误卡片有 Retry 页脚
        let footer_row = buffer.area().height - 2;
        let row_text: String = (0..buffer.area().width)
            .map(|x| buffer.get(x, footer_row).symbol().to_string())
            .collect();
        assert!(row_text.contains("Retry"), "Error card should show [Retry] footer");
    }
}
