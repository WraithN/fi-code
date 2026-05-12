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

use std::net::TcpListener;
use std::process::Output;

/// 获取 TUI 二进制路径
fn tui_bin() -> String {
    std::env::var("CARGO_BIN_EXE_fi-code-tui")
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            std::path::Path::new(&manifest_dir)
                .parent()
                .unwrap()
                .join("target/debug/fi-code-tui")
                .to_string_lossy()
                .to_string()
        })
}

/// 获取一个随机可用端口
fn get_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

/// 运行 TUI 命令并返回输出
async fn run_tui(args: &[&str]) -> anyhow::Result<Output> {
    let output = tokio::process::Command::new(&tui_bin())
        .args(args)
        .output()
        .await?;
    Ok(output)
}

mod e2e_tui {
    use super::*;

    #[tokio::test]
    async fn test_tui_help_flag_shows_usage() {
        let output = run_tui(&["--help"]).await.expect("Failed to run TUI");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{} {}", stdout, stderr);
        assert!(
            combined.contains("fi-code") || combined.contains("Usage:"),
            "Expected help output, got:\nstdout:\n{}\nstderr:\n{}",
            stdout,
            stderr
        );
    }

    #[tokio::test]
    async fn test_tui_version_flag_shows_version() {
        let output = run_tui(&["--version"]).await.expect("Failed to run TUI");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("0.1.0"),
            "Expected version output, got:\n{}",
            stdout
        );
    }

    #[tokio::test]
    async fn test_tui_starts_backend_server() {
        let port = get_available_port();
        let mut child = tokio::process::Command::new(&tui_bin())
            .env("FI_CODE_TEST_MODE", "1")
            .args(&["--port", &port.to_string()])
            .spawn()
            .expect("Failed to start TUI");

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        
        let result = client
            .get(&format!("http://localhost:{}/api/config", port))
            .send()
            .await;
        assert!(result.is_ok(), "TUI backend server should be accessible");
        
        if let Ok(resp) = result {
            assert_eq!(resp.status(), 200);
        }

        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}
