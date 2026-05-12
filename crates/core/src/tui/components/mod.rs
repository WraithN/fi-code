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

use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub mod card_widget;
pub mod chat;
pub mod input;
pub mod left_drawer;
pub mod log_window;
pub mod question_dialog;
pub mod right_drawer;
pub mod status_bar;

pub use log_window::LogWindow;

/// 所有 TUI 组件必须实现的公共 trait。
///
/// 采用"渲染-事件-更新"三段式架构：
/// - `draw`：每帧调用，负责在指定 `Rect` 内绘制自身。
/// - `handle_event`：接收终端事件，返回可选的 `AppEvent` 以通知上层处理业务逻辑。
/// - `update`：接收应用事件，用于同步外部状态（如会话列表刷新、SSE 消息到达）。
/// - `is_focusable`：标识该组件是否可接收焦点（如状态栏不可聚焦）。
pub trait Component {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool);
    fn handle_event(&mut self, event: &Event, focus: bool) -> Option<AppEvent>;
    fn update(&mut self, _event: &AppEvent) {}
    fn is_focusable(&self) -> bool {
        true
    }
}
