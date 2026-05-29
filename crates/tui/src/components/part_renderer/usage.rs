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

use ratatui::{layout::Alignment, text::Line, widgets::Paragraph};

use super::*;

pub struct UsageRenderer;

/// 将 token 数量格式化为易读字符串，例如 1200 → "1.2k"。
fn format_tokens(n: u32) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

impl PartRenderer for UsageRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, _skip_lines: u16) {
        if let Part::Usage {
            prompt_tokens,
            completion_tokens,
            latency_ms,
            cost,
        } = part
        {
            let mut text = format!(
                "⬆️{} ⬇️{} · LAT:{:.1}s",
                format_tokens(*prompt_tokens),
                format_tokens(*completion_tokens),
                *latency_ms as f64 / 1000.0
            );
            if let Some(c) = cost {
                text.push_str(&format!(" · ${:.4}", c));
            }
            let line = Line::from(text);
            let paragraph = Paragraph::new(line)
                .alignment(Alignment::Right)
                .style(theme.style_muted());
            frame.render_widget(paragraph, area);
        }
    }
}
