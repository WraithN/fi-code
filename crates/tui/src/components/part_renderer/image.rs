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

use ratatui::{text::Line, widgets::Paragraph};

use super::*;

pub struct ImageRenderer;

impl PartRenderer for ImageRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, _skip_lines: u16) {
        if let Part::Image { source } = part {
            let text = match source {
                fi_code_core::session::message::ImageSource::Path { path } => {
                    format!("🖼 {}", path)
                }
                fi_code_core::session::message::ImageSource::Base64 { media_type, .. } => {
                    format!("🖼 [Base64 image: {}]", media_type)
                }
                fi_code_core::session::message::ImageSource::Url { url } => {
                    format!("🖼 {}", url)
                }
            };
            let line = Line::from(text);
            let paragraph = Paragraph::new(line).style(theme.style_muted());
            frame.render_widget(paragraph, area);
        }
    }
}
