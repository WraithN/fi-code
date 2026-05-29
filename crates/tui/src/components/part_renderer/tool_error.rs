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
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::*;

pub struct ToolErrorRenderer;

impl PartRenderer for ToolErrorRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::ToolError {
            error_message,
            for_context_only,
            ..
        } = part
        {
            // 如果标记为仅用于上下文，不占用空间
            if *for_context_only {
                return 0;
            }
            let lines: Vec<&str> = error_message.lines().collect();
            let mut h = 0u16;
            for line in lines {
                let w = line.chars().count() as u16;
                h += (w / width.max(1)).max(0) + 1;
            }
            h.max(1) + 2 // +2 for borders
        } else {
            3
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, skip_lines: u16) {
        if let Part::ToolError {
            error_message,
            for_context_only,
            ..
        } = part
        {
            // 如果标记为仅用于上下文，不渲染
            if *for_context_only {
                return;
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.error))
                .title(
                    Line::from("❌ Tool Error")
                        .style(theme.style_error().add_modifier(Modifier::BOLD)),
                );
            let paragraph = Paragraph::new(error_message.as_str())
                .wrap(Wrap { trim: true })
                .style(theme.style_primary())
                .block(block)
                .scroll((skip_lines, 0));
            frame.render_widget(paragraph, area);
        }
    }
}
