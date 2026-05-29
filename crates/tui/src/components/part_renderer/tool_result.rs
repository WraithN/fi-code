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

/// 将 ToolResult 格式化为人类可读的摘要。
///
/// 返回 (title, body)：title 用于卡片标题，body 用于卡片内容。
fn format_tool_result(content: &str) -> (String, String) {
    // 尝试解析为 JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        // 提取常见字段
        if let Some(obj) = json.as_object() {
            // 检查是否包含 success 字段
            let is_success = obj.get("success").and_then(|v| v.as_bool()).unwrap_or(true);

            let title = if is_success {
                "✅ Result".to_string()
            } else {
                "❌ Result".to_string()
            };

            // 提取 output / error / path 等字段
            let mut body_parts = Vec::new();
            if let Some(output) = obj.get("output").and_then(|v| v.as_str()) {
                body_parts.push(output.to_string());
            }
            if let Some(error) = obj.get("error").and_then(|v| v.as_str()) {
                body_parts.push(format!("Error: {}", error));
            }
            if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                body_parts.push(format!("path: {}", path));
            }

            let body = if body_parts.is_empty() {
                // 如果没有提取到特定字段，展示关键字段摘要
                let mut summary = Vec::new();
                for (k, v) in obj.iter() {
                    if k == "success" {
                        continue;
                    }
                    if let Some(s) = v.as_str() {
                        if s.len() > 200 {
                            summary.push(format!("{}: {}...", k, &s[..200]));
                        } else {
                            summary.push(format!("{}: {}", k, s));
                        }
                    } else {
                        summary.push(format!("{}: {}", k, v));
                    }
                }
                summary.join("\n")
            } else {
                body_parts.join("\n")
            };

            return (title, body);
        }
    }

    // 不是 JSON，直接展示原始内容
    ("📤 Result".to_string(), content.to_string())
}

pub struct ToolResultRenderer;

impl PartRenderer for ToolResultRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::ToolResult {
            content,
            duration_ms,
            for_context_only,
            ..
        } = part
        {
            // 如果标记为仅用于上下文，不占用空间
            if *for_context_only {
                return 0;
            }
            let (_, body) = format_tool_result(content);
            let lines: Vec<&str> = body.lines().collect();
            let mut h = 0u16;
            for line in lines {
                let w = line.chars().count() as u16;
                h += (w / width.max(1)).max(0) + 1;
            }
            // +2 for borders, +1 for duration footer if present
            let footer = if duration_ms.is_some() { 1 } else { 0 };
            h.max(1) + 2 + footer
        } else {
            3
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, skip_lines: u16) {
        if let Part::ToolResult {
            content,
            duration_ms,
            for_context_only,
            ..
        } = part
        {
            // 如果标记为仅用于上下文，不渲染
            if *for_context_only {
                return;
            }
            let (title, body) = format_tool_result(content);
            let border_color = if content.contains("error") || content.contains("Error") {
                theme.error
            } else {
                theme.border
            };

            // 如果有耗时信息，在底部增加一行 footer 显示耗时
            let footer_text = duration_ms.map(|ms| {
                if ms < 1000 {
                    format!("⏱ {}ms", ms)
                } else {
                    format!("⏱ {:.1}s", ms as f64 / 1000.0)
                }
            });

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Line::from(title).style(theme.style_primary().add_modifier(Modifier::BOLD)));

            let mut text_lines = body.lines().map(|s| s.to_string()).collect::<Vec<_>>();
            if let Some(ref ft) = footer_text {
                text_lines.push(format!("\n{}", ft));
            }
            let full_body = text_lines.join("\n");

            let paragraph = Paragraph::new(full_body)
                .wrap(Wrap { trim: true })
                .style(theme.style_primary())
                .block(block)
                .scroll((skip_lines, 0));
            frame.render_widget(paragraph, area);
        }
    }
}
