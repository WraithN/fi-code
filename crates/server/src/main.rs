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

use fi_code_core::config::Config;
use fi_code_core::provider::Provider;
use fi_code_core::server::Server;
use std::sync::{Arc, RwLock};

#[tokio::main]
async fn main() {
    let config = Arc::new(RwLock::new(Config::load().unwrap()));
    let provider = Arc::new(RwLock::new(Provider::new(Arc::clone(&config)).unwrap()));

    // 初始化可观测性子系统：失败则致命退出（保证 trace 数据完整性）
    {
        let cfg = config.read().expect("config read");
        if let Err(e) = fi_code_core::observability::init(&cfg) {
            eprintln!("[fatal] observability init failed: {}", e);
            std::process::exit(1);
        }
    }

    // 使用 tokio::select 以便在 ctrl_c 时优雅关闭，flush 残留 span
    tokio::select! {
        _ = Server::new(provider, config, None).run() => {},
        _ = tokio::signal::ctrl_c() => {},
    }
    fi_code_core::observability::shutdown();
}
