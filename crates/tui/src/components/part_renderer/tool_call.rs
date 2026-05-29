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

/// 将 ToolUse 格式化为人类可读的摘要。
///
/// 返回 (title, body)：title 用于卡片标题，body 用于卡片内容。
fn format_tool_use(name: &str, arguments: &serde_json::Value) -> (String, String) {
    // 如果 arguments 包含 _raw（JSON 解析失败时的回退），尝试从中提取字段
    let arguments = if let Some(raw) = arguments.get("_raw").and_then(|v| v.as_str()) {
        // 尝试解析原始字符串中的已知字段
        let mut extracted = serde_json::Map::new();
        for key in ["path", "content", "command", "pattern", "url", "message"] {
            if let Some(pos) = raw.find(&format!("\"{}\"", key)) {
                let start = raw[pos + key.len() + 3..]
                    .find('"')
                    .map(|i| pos + key.len() + 3 + i + 1);
                if let Some(start) = start {
                    if let Some(end) = raw[start..].find('"') {
                        let value = &raw[start..start + end];
                        extracted.insert(
                            key.to_string(),
                            serde_json::Value::String(value.to_string()),
                        );
                    }
                }
            }
        }
        serde_json::Value::Object(extracted)
    } else {
        arguments.clone()
    };

    // 辅助函数：从 arguments 中提取字段
    let get = |key: &str| -> String {
        arguments
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    match name {
        "write" | "edit" => {
            let path = get("path");
            let content = get("content");
            let body = if content.is_empty() {
                format!("path: {}", path)
            } else {
                format!("path: {}\n[{} bytes]", path, content.len())
            };
            (format!("📝 {}", name), body)
        }
        "read" | "read_file" => {
            let path = get("path");
            (format!("📖 {}", name), format!("path: {}", path))
        }
        "bash" => {
            let cmd = get("command");
            ("⚡ bash".to_string(), format!("$ {}", cmd))
        }
        "grep" => {
            let pattern = get("pattern");
            let path = get("path");
            (
                "🔍 grep".to_string(),
                format!("pattern: {}\npath: {}", pattern, path),
            )
        }
        "web_fetch" => {
            let url = get("url");
            ("🌐 web_fetch".to_string(), format!("url: {}", url))
        }
        "git_status" => ("📋 git status".to_string(), String::new()),
        "git_diff" => {
            let path = get("path");
            let body = if path.is_empty() {
                String::new()
            } else {
                format!("path: {}", path)
            };
            ("📋 git diff".to_string(), body)
        }
        "git_add" => {
            let path = get("path");
            ("➕ git add".to_string(), format!("path: {}", path))
        }
        "git_commit" => {
            let message = get("message");
            ("💾 git commit".to_string(), format!("message: {}", message))
        }
        "git_log" => ("📜 git log".to_string(), String::new()),
        "git_worktree" => ("🌿 git worktree".to_string(), String::new()),
        "glob" => {
            let pattern = get("pattern");
            let dir = get("dir");
            let body = if dir.is_empty() {
                format!("pattern: {}", pattern)
            } else {
                format!("pattern: {}\ndir: {}", pattern, dir)
            };
            ("📁 glob".to_string(), body)
        }
        "create_task_plan" | "handle_task_plan" => {
            let title = format!("📋 {}", name);
            (title, String::new())
        }
        "ask_for_question" => {
            let question = get("question");
            ("❓ ask".to_string(), format!("question: {}", question))
        }
        "use_skill" => {
            let skill_name = get("name");
            ("🎯 use_skill".to_string(), format!("skill: {}", skill_name))
        }
        _ => {
            // 未知工具：展示参数摘要（排除过长的 content 字段）
            let summary = if let Some(obj) = arguments.as_object() {
                let mut parts = Vec::new();
                for (k, v) in obj.iter() {
                    if k == "content" {
                        if let Some(s) = v.as_str() {
                            parts.push(format!("{}: [{} bytes]", k, s.len()));
                        }
                    } else {
                        parts.push(format!("{}: {}", k, v));
                    }
                }
                parts.join("\n")
            } else {
                arguments.to_string()
            };
            (format!("🔧 {}", name), summary)
        }
    }
}

pub struct ToolCallRenderer;

impl PartRenderer for ToolCallRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::ToolUse {
            name, arguments, ..
        } = part
        {
            let (_, body) = format_tool_use(name, arguments);
            let lines: Vec<&str> = body.lines().collect();
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
        if let Part::ToolUse {
            name, arguments, ..
        } = part
        {
            let (title, body) = format_tool_use(name, arguments);
            // 根据工具类型分配不同的边框颜色，提升视觉辨识度
            let border_color = match name.as_str() {
                "bash" => theme.warning,
                "write" | "edit" => theme.user,
                "read" | "read_file" | "grep" | "glob" => theme.brand,
                "git_status" | "git_diff" | "git_add" | "git_commit" | "git_log"
                | "git_worktree" | "git" => theme.success,
                "web_fetch" => theme.accent_hover,
                "create_task_plan" | "handle_task_plan" => theme.user,
                "ask_for_question" => theme.warning,
                _ => theme.border,
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Line::from(title).style(theme.style_primary().add_modifier(Modifier::BOLD)));
            let paragraph = Paragraph::new(body)
                .wrap(Wrap { trim: true })
                .style(theme.style_primary())
                .block(block)
                .scroll((skip_lines, 0));
            frame.render_widget(paragraph, area);
        }
    }
}
