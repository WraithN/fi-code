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

use std::path::PathBuf;
use std::sync::Mutex;

// =============================================================================
// 全局工作目录配置
// =============================================================================
// 通过 Mutex 包装，允许在程序启动时设置一次，也便于测试中动态调整。

static WORKSPACE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 设置全局工作目录。
pub fn set_workspace(path: PathBuf) {
    let mut guard = WORKSPACE.lock().unwrap();
    *guard = Some(path);
}

/// 获取当前配置的工作目录。
/// - 如果已调用 `set_workspace`，返回设置的目录
/// - 否则默认返回用户主目录
pub fn workspace_dir() -> PathBuf {
    WORKSPACE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| dirs::home_dir().expect("无法获取用户主目录"))
}
