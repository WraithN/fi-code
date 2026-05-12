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

/// 文件树节点。
#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize, // 缩进深度，用于层级可视化
}

/// 左侧文件抽屉组件，展示项目文件树，支持上下导航与选中。
pub struct LeftDrawer {
    files: Vec<FileNode>,
    selected_index: usize,
    expanded_folders: std::collections::HashSet<String>, // 预留：记录已展开的文件夹
}

impl LeftDrawer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            selected_index: 0,
            expanded_folders: std::collections::HashSet::new(),
        }
    }

    /// 设置文件列表并重置选中位置到顶部。
    pub fn set_files(&mut self, files: Vec<FileNode>) {
        self.files = files;
        self.selected_index = 0;
    }
}

impl Component for LeftDrawer {
    /// 渲染文件抽屉：绘制边框与标题，按深度缩进显示文件/文件夹图标列表，
    /// 当前选中项使用反色高亮。
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
            .title("Files")
            .style(theme.drawer_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<Line> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let indent = "  ".repeat(file.depth);
                let icon = if file.is_dir { "📁 " } else { "📄 " };
                let style = if i == self.selected_index {
                    theme.style_selection()
                } else {
                    theme.style_primary()
                };

                Line::from(vec![Span::styled(
                    format!("{}{}{}", indent, icon, file.name),
                    style,
                )])
            })
            .collect();

        let paragraph = Paragraph::new(items);
        frame.render_widget(paragraph, inner);
    }

    /// 处理导航事件：上下方向键移动选中，Enter 触发选中文件事件。
    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }

            match key.code {
                KeyCode::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                    None
                }
                KeyCode::Down => {
                    if self.selected_index < self.files.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                    None
                }
                KeyCode::Enter => {
                    if let Some(file) = self.files.get(self.selected_index) {
                        return Some(AppEvent::SelectFile(file.path.clone()));
                    }
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
    fn test_file_navigation() {
        let mut drawer = LeftDrawer::new();
        drawer.set_files(vec![
            FileNode {
                path: "src".to_string(),
                name: "src".to_string(),
                is_dir: true,
                depth: 0,
            },
            FileNode {
                path: "Cargo.toml".to_string(),
                name: "Cargo.toml".to_string(),
                is_dir: false,
                depth: 0,
            },
        ]);

        assert_eq!(drawer.selected_index, 0);
    }
}
