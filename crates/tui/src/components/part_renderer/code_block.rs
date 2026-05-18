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
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};

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

/// 代码块渲染器，支持语法高亮 + 行号
pub struct CodeBlockRenderer;

/// 创建默认的 CodeBlockRenderer 实例
pub fn new() -> CodeBlockRenderer {
    CodeBlockRenderer
}

impl CodeBlockRenderer {
    /// 根据语言提示和代码内容，返回带语法高亮和行号的 `Line` 列表。
    /// `content_width` 是可用内容宽度（不含行号列）。
    pub fn render(
        &self,
        code: &str,
        language_hint: Option<&str>,
        _theme: &crate::theme::Theme,
    ) -> Vec<Line<'static>> {
        // 选择 syntect 语法主题（使用 "base16-ocean.dark" 作为暗色默认）
        let syntect_theme = THEME_SET
            .themes
            .get("base16-ocean.dark")
            .or_else(|| THEME_SET.themes.values().next())
            .expect("至少存在一个默认主题");

        // 根据语言提示查找语法定义
        let syntax = language_hint
            .and_then(|lang| SYNTAX_SET.find_syntax_by_token(lang))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, syntect_theme);

        let lines: Vec<&str> = code.lines().collect();
        let line_num_width = lines.len().to_string().len().max(2);
        let mut result = Vec::with_capacity(lines.len());

        for (idx, line) in lines.iter().enumerate() {
            // 行号 Span
            let line_num = format!("{:>width$} ", idx + 1, width = line_num_width);
            let line_num_span = Span::styled(line_num, Style::default().fg(Color::Rgb(80, 80, 80)));

            // 语法高亮：逐行处理
            let highlighted = highlighter
                .highlight_line(line, &SYNTAX_SET)
                .unwrap_or_default();
            let mut spans: Vec<Span<'static>> = vec![line_num_span];
            for (style, text) in highlighted {
                spans.push(Span::styled(text.to_string(), syntect_to_ratatui(style)));
            }

            result.push(Line::from(spans));
        }

        result
    }

    /// 计算代码块在给定宽度下需要占用的行高
    pub fn height(&self, code: &str, _width: u16) -> u16 {
        let lines = code.lines().count() as u16;
        lines.max(1)
    }
}
