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

use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::theme::Theme;
use fi_code_core::session::message::Part;

/// Part 渲染器 trait：为每一种 `Part` 变体提供高度计算、绘制和可选的事件处理。
pub trait PartRenderer {
    /// 计算该 Part 在给定宽度下需要占用的行高。
    fn height(&self, part: &Part, width: u16) -> u16;
    /// 在指定区域绘制该 Part。
    /// `skip_lines` 表示该 Part 顶部被视口裁剪掉的行数，多行 Paragraph 需要通过
    /// `.scroll((skip_lines, 0))` 跳过这些行，保证渲染内容和滚动位置一致。
    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, skip_lines: u16);
    /// 可选的事件处理，返回 true 表示事件已被消费。
    fn handle_event(&mut self, _part: &mut Part, _event: &Event) -> bool {
        false
    }
}

/// Part 渲染器注册表，根据 `Part` 变体分发到对应的渲染器。
pub struct PartRendererRegistry {
    renderers: std::collections::HashMap<String, Box<dyn PartRenderer>>,
}

impl PartRendererRegistry {
    pub fn new() -> Self {
        use std::collections::HashMap;
        let mut registry = Self {
            renderers: HashMap::new(),
        };
        registry.register("text", Box::new(text::TextRenderer));
        registry.register("reasoning", Box::new(thinking::ThinkingRenderer));
        registry.register("tool_use", Box::new(tool_call::ToolCallRenderer));
        registry.register("tool_result", Box::new(tool_result::ToolResultRenderer));
        registry.register("tool_error", Box::new(tool_error::ToolErrorRenderer));
        registry.register("wave_marker", Box::new(wave_marker::WaveMarkerRenderer));
        registry.register("usage", Box::new(usage::UsageRenderer));
        registry.register("image", Box::new(image::ImageRenderer));
        registry.register("code_block", Box::new(code_block::CodeBlockRenderer));
        registry
    }

    pub fn register(&mut self, name: &str, renderer: Box<dyn PartRenderer>) {
        self.renderers.insert(name.to_string(), renderer);
    }

    pub fn get(&self, part: &Part) -> Option<&dyn PartRenderer> {
        let key = match part {
            Part::Text { .. } => "text",
            Part::Image { .. } => "image",
            Part::ToolUse { .. } => "tool_use",
            Part::ToolResult { .. } => "tool_result",
            Part::ToolError { .. } => "tool_error",
            Part::Reasoning { .. } => "reasoning",
            Part::WaveMarker { .. } => "wave_marker",
            Part::Usage { .. } => "usage",
            Part::CodeBlock { .. } => "code_block",
            Part::SystemNotice { .. } => "text",
        };
        self.renderers.get(key).map(|b| b.as_ref())
    }
}

impl Default for PartRendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

mod code_block;
mod image;
mod text;
mod thinking;
mod tool_call;
mod tool_error;
mod tool_result;
mod usage;
mod wave_marker;
