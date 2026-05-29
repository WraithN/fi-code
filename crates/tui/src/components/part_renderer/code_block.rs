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

use std::sync::LazyLock;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};

use super::PartRenderer;
use crate::theme::Theme;
use fi_code_core::session::message::Part;

// 全局懒加载的 SyntaxSet 和 ThemeSet
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// 将 syntect 的 Style 转换为 ratatui 的 Style
fn syntect_to_ratatui(style: SyntectStyle) -> Style {
    let fg = style.foreground;
    Style::default()
        .fg(Color::Rgb(fg.r, fg.g, fg.b))
        .bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ))
}

/// 代码块渲染器，支持语法高亮 + diff 着色 + 行号
pub struct CodeBlockRenderer;

impl PartRenderer for CodeBlockRenderer {
    fn height(&self, part: &Part, _width: u16) -> u16 {
        if let Part::CodeBlock {
            code,
            for_context_only,
            ..
        } = part
        {
            // 如果标记为仅用于上下文，不占用空间
            if *for_context_only {
                return 0;
            }
            let lines = code.lines().count() as u16;
            // +2 for borders
            lines.max(1) + 2
        } else {
            3
        }
    }

    fn draw(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        part: &Part,
        theme: &Theme,
        skip_lines: u16,
    ) {
        if let Part::CodeBlock {
            code,
            language,
            for_context_only,
        } = part
        {
            // 如果标记为仅用于上下文，不渲染
            if *for_context_only {
                return;
            }
            let syntect_theme = THEME_SET
                .themes
                .get("base16-ocean.dark")
                .or_else(|| THEME_SET.themes.values().next())
                .expect("至少存在一个默认主题");

            let syntax = if language.is_empty() {
                None
            } else {
                SYNTAX_SET.find_syntax_by_token(language)
            };
            let syntax = syntax.unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

            let mut highlighter = HighlightLines::new(syntax, syntect_theme);

            let lines: Vec<&str> = code.lines().collect();
            let line_num_width = lines.len().to_string().len().max(2);
            let mut text_lines = Vec::with_capacity(lines.len());

            for (idx, line) in lines.iter().enumerate() {
                // 处理 diff 行着色：行首 + 绿色，- 红色
                let diff_style = if line.starts_with('+') {
                    Some(Style::default().fg(Color::Green))
                } else if line.starts_with('-') {
                    Some(Style::default().fg(Color::Red))
                } else {
                    None
                };

                // 行号 Span
                let line_num = format!("{:>width$} ", idx + 1, width = line_num_width);
                let line_num_span =
                    Span::styled(line_num, Style::default().fg(Color::Rgb(80, 80, 80)));

                let mut spans: Vec<Span<'static>> = vec![line_num_span];

                if let Some(style) = diff_style {
                    // diff 行：整行使用 diff 着色，不做语法高亮
                    spans.push(Span::styled(line.to_string(), style));
                } else {
                    // 普通行：语法高亮
                    let highlighted = highlighter
                        .highlight_line(line, &SYNTAX_SET)
                        .unwrap_or_default();
                    for (style, text) in highlighted {
                        spans.push(Span::styled(text.to_string(), syntect_to_ratatui(style)));
                    }
                }

                text_lines.push(Line::from(spans));
            }

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title(
                    Line::from(
                        if language.is_empty() {
                            "code"
                        } else {
                            language
                        }
                        .to_string(),
                    )
                    .style(theme.style_primary()),
                );

            let paragraph = Paragraph::new(text_lines)
                .style(theme.style_primary())
                .block(block)
                .scroll((skip_lines, 0));

            frame.render_widget(paragraph, area);
        }
    }
}
