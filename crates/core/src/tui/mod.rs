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

// 子模块声明：TUI 由应用主循环、HTTP 客户端、UI 组件、事件、布局、主题六大部分组成
pub mod app;
pub mod client;
pub mod components;
pub mod event;
pub mod layout;
pub mod theme;

use app::TuiApp;

/// 启动 TUI 界面。
///
/// 该函数负责：
/// 1. 初始化 ratatui 终端后端（自动启用备用屏幕、隐藏光标、捕获键盘事件）。
/// 2. 清屏后创建 `TuiApp` 并进入主循环。
/// 3. 无论运行结果如何，最终调用 `ratatui::restore()` 还原终端状态，防止退出后终端乱码。
pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // 启用鼠标事件捕获（滚轮 + 点击）
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    // 退出前禁用鼠标捕获
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}
