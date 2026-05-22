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

use clap::Parser;

/// FiCode TUI — Terminal User Interface mode
#[derive(Parser)]
#[command(name = "fi-code-tui")]
#[command(version = "0.1.0")]
#[command(about = "Launch FiCode in interactive terminal mode")]
struct TuiArgs {
    /// Port for the embedded server
    #[arg(long, short)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = TuiArgs::parse();

    // 初始化可观测性（失败不阻塞 TUI 启动，仅 warn）
    if let Ok(cfg) = fi_code_core::config::Config::load() {
        let _ = fi_code_core::observability::init(&cfg);
    }

    let result = fi_code_tui::run_tui_mode(args.port).await;
    fi_code_core::observability::shutdown();
    result
}
