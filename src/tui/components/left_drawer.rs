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

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

pub struct LeftDrawer {
    files: Vec<FileNode>,
    selected_index: usize,
    expanded_folders: std::collections::HashSet<String>,
}

impl LeftDrawer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            selected_index: 0,
            expanded_folders: std::collections::HashSet::new(),
        }
    }

    pub fn set_files(&mut self, files: Vec<FileNode>) {
        self.files = files;
        self.selected_index = 0;
    }
}

impl Component for LeftDrawer {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
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
