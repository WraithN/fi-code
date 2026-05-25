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

//! E2E 测试桥接脚本
//!
//! 该脚本作为 Cargo 测试目标，负责调用 pytest 运行 Python E2E 测试。
//! 通过 `cargo test --test e2e_all` 触发，保持与既有 cargo test 工作流的兼容。

use std::process::{Command, exit};

/// E2E 测试入口
///
/// 通过标准输出打印启动信息，然后调用 pytest 执行 tests/e2e/ 目录下的所有测试。
/// pytest 的退出码直接透传给 Cargo，确保测试失败时 CI 能正确识别。
#[test]
fn run_e2e_tests() {
    println!("Running E2E tests via pytest...");

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let e2e_dir = format!("{}/e2e", manifest_dir);

    let status = Command::new("pytest")
        .arg(&e2e_dir)
        .arg("-v")
        .status()
        .expect("Failed to execute pytest. Please ensure pytest is installed: pip install pytest");

    if !status.success() {
        eprintln!("pytest exited with code: {:?}", status.code());
        exit(status.code().unwrap_or(1));
    }

    println!("E2E tests completed successfully.");
}
