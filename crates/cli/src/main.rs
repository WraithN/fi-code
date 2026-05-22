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

mod cli_args;
mod entry;

use entry::{run, EntryOutcome};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化可观测性：失败仅 warn（CLI 模式宽松，不阻塞用户工作流）
    // 注意：observability::init 内部已 log_warn 提示，此处直接尝试初始化
    if let Ok(cfg) = fi_code_core::config::Config::load() {
        let _ = fi_code_core::observability::init(&cfg);
    }

    let result = match run().await? {
        EntryOutcome::Completed => Ok(()),
        EntryOutcome::StartTui { port } => fi_code_tui::run_tui_mode(port).await,
    };

    // 优雅退出时 flush 残留 span
    fi_code_core::observability::shutdown();
    result
}
