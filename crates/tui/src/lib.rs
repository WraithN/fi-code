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

pub mod app;
pub mod client;
pub mod components;
pub mod i18n;
pub mod layout;
pub mod theme;

use app::TuiApp;
use fi_code_shared::constants::*;
use std::sync::{Arc, RwLock};

/// 启动 TUI 模式（包含嵌入式 Server + TUI 界面）。
///
/// 该函数负责：
/// 1. 加载配置并初始化 Provider。
/// 2. 启动日志广播器。
/// 3. 在后台启动 HTTP Server。
/// 4. 等待 Server 就绪后启动 TUI 界面。
/// 5. TUI 退出后自动关闭 Server。
pub async fn run_tui_mode(port: Option<u16>) -> anyhow::Result<()> {
    let config = Arc::new(RwLock::new(fi_code_core::config::Config::load()?));
    {
        let cfg = config.read().map_err(|_| anyhow::anyhow!("配置锁中毒"))?;
        let extra = cfg.skills.as_ref().map(|s| s.directories.as_slice());
        fi_code_core::skills::init_skills(extra);
    }
    let provider = Arc::new(RwLock::new(fi_code_core::provider::Provider::new(
        Arc::clone(&config),
    )?));

    let log_broadcaster = Arc::new(fi_code_core::utils::log_store::LogBroadcaster::new(1000));
    fi_code_core::utils::log::set_global_log_broadcaster(Arc::clone(&log_broadcaster));

    // 启动 Server（后台任务）
    let server =
        fi_code_core::server::Server::new(Arc::clone(&provider), Arc::clone(&config), port)
            .with_log_broadcaster(log_broadcaster);
    let server_handle = tokio::spawn(async move {
        server.run().await;
    });

    // 等待 Server 启动
    tokio::time::sleep(std::time::Duration::from_millis(
        TUI_STARTUP_POLL_INTERVAL_MS,
    ))
    .await;

    // 测试模式下不启动 TUI，直接返回，便于 E2E 测试验证后端服务
    if std::env::var("FI_CODE_TEST_MODE").is_ok() {
        // 保持 Server 运行一段时间，让测试可以连接验证
        tokio::time::sleep(std::time::Duration::from_secs(TUI_SERVER_STARTUP_WAIT_SECS)).await;
        server_handle.abort();
        return Ok(());
    }

    // 启动 TUI
    let result = run_tui().await;

    // TUI 退出后关闭 Server
    server_handle.abort();

    result
}

/// 启动纯 TUI 界面。
///
/// 该函数负责：
/// 1. 初始化 ratatui 终端后端（自动启用备用屏幕、隐藏光标、捕获键盘事件）。
/// 2. 清屏后创建 `TuiApp` 并进入主循环。
/// 3. 无论运行结果如何，最终调用 `ratatui::restore()` 还原终端状态，防止退出后终端乱码。
pub async fn run_tui() -> anyhow::Result<()> {
    // 设置 panic hook：在 TUI panic 时自动恢复终端状态，避免退出后终端乱码
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // 恢复终端状态：禁用鼠标捕获、离开备用屏幕、显示光标、关闭 raw mode
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = crossterm::terminal::disable_raw_mode();

        // 打印友好的错误提示
        eprintln!("\n❌ 程序发生致命错误，正在退出...\n");

        // 调用原始 panic hook 输出 backtrace
        original_hook(info);
    }));

    let mut terminal = ratatui::init();
    terminal.clear()?;

    // 启用鼠标事件捕获（滚轮 + 点击）
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    // 正常退出：恢复原始 panic hook，禁用鼠标捕获，还原终端
    let _ = std::panic::take_hook();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}
