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

use std::path::{Path, PathBuf};
use std::process::Output;
use anyhow::Result;

/// 创建临时测试工作目录
pub fn create_temp_workspace() -> Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    Ok(dir)
}

/// 在指定目录下初始化一个简单的项目结构
pub fn init_test_project<P: AsRef<Path>>(workspace: P) -> Result<()> {
    let workspace = workspace.as_ref();
    std::fs::create_dir_all(workspace.join("src"))?;
    std::fs::write(workspace.join("Cargo.toml"), r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#)?;
    std::fs::write(workspace.join("src/main.rs"), "fn main() {}")?;
    Ok(())
}

/// 运行 CLI 命令并捕获输出
pub async fn run_cli(args: &[&str]) -> Result<Output> {
    let cli_path = std::env::var("CARGO_BIN_EXE_fi-code-cli")
        .unwrap_or_else(|_| "./target/debug/fi-code-cli".to_string());
    
    let output = tokio::process::Command::new(&cli_path)
        .args(args)
        .output()
        .await?;
    
    Ok(output)
}

/// 运行 Server 命令并在后台启动
pub async fn run_server(port: u16) -> Result<tokio::process::Child> {
    let server_path = std::env::var("CARGO_BIN_EXE_fi-code-server")
        .unwrap_or_else(|_| "./target/debug/fi-code-server".to_string());
    
    let child = tokio::process::Command::new(&server_path)
        .arg("--port")
        .arg(port.to_string())
        .spawn()?;
    
    Ok(child)
}

/// 等待一小段时间让服务启动
pub async fn wait_for_startup() {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// 验证输出是否包含预期字符串
pub fn assert_output_contains(output: &Output, expected: &str) {
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

/// 创建测试用的临时配置文件目录
pub fn create_test_config_dir() -> Result<PathBuf> {
    let dir = tempfile::tempdir()?;
    let config_dir = dir.path().join(".config/fi-code");
    std::fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}
