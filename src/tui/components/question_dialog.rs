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

use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListItem, Paragraph, Borders, Wrap};
use crate::tui::event::{QuestionOption, QuestionAnswer};

#[derive(Debug, Clone)]
pub enum QuestionDialogAction {
    Submit(QuestionAnswer),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct QuestionDialog {
    pub question: String,
    pub options: Vec<QuestionOption>,
    pub recommended: Option<String>,
    pub allow_custom: bool,
    pub selected_index: usize,
    pub custom_input: String,
    pub cursor_position: usize,
}

impl QuestionDialog {
    pub fn new(
        question: String,
        options: Vec<QuestionOption>,
        recommended: Option<String>,
        allow_custom: bool,
    ) -> Self {
        Self {
            question,
            options,
            recommended,
            allow_custom,
            selected_index: 0,
            custom_input: String::new(),
            cursor_position: 0,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Option<QuestionDialogAction> {
        match code {
            KeyCode::Enter => {
                if self.is_custom_selected() {
                    Some(QuestionDialogAction::Submit(QuestionAnswer::Custom(self.custom_input.clone())))
                } else if let Some(option) = self.options.get(self.selected_index) {
                    Some(QuestionDialogAction::Submit(QuestionAnswer::Option {
                        id: option.id.clone(),
                        label: option.label.clone(),
                    }))
                } else {
                    None
                }
            }
            KeyCode::Esc => Some(QuestionDialogAction::Cancel),
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }
            KeyCode::Down => {
                let max_index = self.max_index();
                if self.selected_index < max_index {
                    self.selected_index += 1;
                }
                None
            }
            KeyCode::Char(c) if self.is_custom_selected() => {
                self.custom_input.insert(self.cursor_position, c);
                self.cursor_position += 1;
                None
            }
            KeyCode::Backspace if self.is_custom_selected() => {
                if self.cursor_position > 0 {
                    self.custom_input.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
                None
            }
            KeyCode::Left if self.is_custom_selected() => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                None
            }
            KeyCode::Right if self.is_custom_selected() => {
                if self.cursor_position < self.custom_input.len() {
                    self.cursor_position += 1;
                }
                None
            }
            _ => None,
        }
    }

    fn is_custom_selected(&self) -> bool {
        self.allow_custom && self.selected_index == self.options.len()
    }

    fn max_index(&self) -> usize {
        if self.allow_custom {
            self.options.len()
        } else {
            self.options.len() - 1
        }
    }

    pub fn draw(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Question ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));
        
        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(inner);

        // 问题文本
        let question_text = Paragraph::new(self.question.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        f.render_widget(question_text, chunks[0]);

        // 选项列表
        let options_area = chunks[1];
        let list_items: Vec<ListItem> = self.options.iter().enumerate().map(|(i, option)| {
            let is_recommended = self.recommended.as_ref() == Some(&option.id);
            let label = if is_recommended {
                format!("{} (推荐)", option.label)
            } else {
                option.label.clone()
            };
            let content = if let Some(desc) = &option.description {
                format!("{}\n  {}", label, desc)
            } else {
                label
            };
            let style = if i == self.selected_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(content).style(style)
        }).collect();

        let mut items = list_items;
        if self.allow_custom {
            let style = if self.is_custom_selected() {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            items.push(ListItem::new("自定义答案").style(style));
        }

        let list = List::new(items)
            .highlight_symbol("> ");
        f.render_widget(list, options_area);

        // 自定义输入框（如果选中自定义）
        if self.is_custom_selected() {
            let input_area = chunks[2];
            let input = Paragraph::new(self.custom_input.as_str())
                .style(Style::default().bg(Color::DarkGray));
            f.render_widget(input, input_area);
        }
    }
}
