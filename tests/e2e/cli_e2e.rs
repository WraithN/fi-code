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

use std::process::Output;

/// 获取 CLI 二进制路径
fn cli_bin() -> String {
    std::env::var("CARGO_BIN_EXE_fi-code-cli")
        .unwrap_or_else(|_| {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            std::path::Path::new(&manifest_dir)
                .parent()
                .unwrap()
                .join("target/debug/fi-code-cli")
                .to_string_lossy()
                .to_string()
        })
}

/// 运行 CLI 命令并返回输出
async fn run_cli(args: &[&str]) -> anyhow::Result<Output> {
    let output = tokio::process::Command::new(&cli_bin())
        .args(args)
        .output()
        .await?;
    Ok(output)
}

/// 验证输出包含预期字符串
fn assert_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{} {}", stdout, stderr);
    assert!(
        combined.contains(expected),
        "Expected output to contain '{}', but got:\nstdout:\n{}\nstderr:\n{}",
        expected,
        stdout,
        stderr
    );
}

mod e2e_cli {
    use super::*;

    #[tokio::test]
    async fn test_cli_help_flag_shows_usage() {
        let output = run_cli(&["--help"]).await.expect("Failed to run CLI");
        assert!(output.status.success());
        assert_contains(&output, "fi-code");
        assert_contains(&output, "Usage:");
    }

    #[tokio::test]
    async fn test_cli_version_flag_shows_version() {
        let output = run_cli(&["--version"]).await.expect("Failed to run CLI");
        assert!(output.status.success());
        assert_contains(&output, "0.1.0");
    }

    #[tokio::test]
    async fn test_cli_models_flag_lists_providers() {
        let output = run_cli(&["--models"]).await.expect("Failed to run CLI");
        assert!(output.status.success());
        assert_contains(&output, "Providers and Models");
    }

    #[tokio::test]
    async fn test_cli_single_command_mode() {
        let output = run_cli(&["--command", "你好"]).await.expect("Failed to run CLI");
        assert_contains(&output, "你好");
    }

    #[tokio::test]
    async fn test_cli_session_flag_lists_sessions() {
        let output = run_cli(&["--session"]).await.expect("Failed to run CLI");
        assert!(output.status.success());
        assert_contains(&output, "sessions");
    }

    #[tokio::test]
    async fn test_cli_server_subcommand_starts_server() {
        let mut child = tokio::process::Command::new(&cli_bin())
            .args(&["server", "--port", "9999"])
            .spawn()
            .expect("Failed to start server");

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        
        let result = client.get("http://localhost:9999/api/config").send().await;
        assert!(result.is_ok(), "Server should be accessible");
        
        if let Ok(resp) = result {
            assert_eq!(resp.status(), 200);
        }

        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}
