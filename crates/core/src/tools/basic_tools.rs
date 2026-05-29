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

use crate::log_debug;
use crate::log_trace;
use crate::tools::windows_compat::{get_bash_path, get_compat_mode, WindowsCompatMode};
use crate::utils::workspace::workspace_dir;
use fi_code_shared::constants::*;
use glob::glob_with;
use glob::MatchOptions;
use ignore::Walk;
use std::cmp::min;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

// =============================================================================
// BasicTool：最底层的文件与命令操作封装
// =============================================================================
// 这是一个"零尺寸类型"（Zero-Sized Type, ZST），因为它不包含任何字段。
// 用空结构体 + `impl` 块来组织静态方法，是一种常见的工具类写法。

pub struct BasicTool {}

/// 缓存系统是否安装 ripgrep，避免每次 run_grep 都 spawn 子进程
static RG_AVAILABLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::process::Command::new("rg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
});

/// 检查是否为隐藏文件/目录（以点开头的名称）
fn is_hidden(entry: &ignore::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

impl BasicTool {
    // =========================================================================
    // 安全路径检查
    // =========================================================================
    // `canonicalize` 会把路径解析为绝对路径，并消除 `.` 和 `..`
    // `starts_with` 确保用户不会通过 `../../etc/passwd` 这种方式逃逸出工作目录

    fn safe_path(p: &str) -> Result<PathBuf, String> {
        let base = workspace_dir();
        let path = base.join(p);

        // 规范化路径：解析 . 和 ..，防止路径遍历攻击
        let normalized = Self::normalize_path(&path);

        // 如果路径已存在，直接 canonicalize 并检查是否在工作目录内
        if let Ok(canonical) = path.canonicalize() {
            if !canonical.starts_with(&base) {
                return Err(format!("路径逃逸出工作目录: {}", p));
            }
            return Ok(canonical);
        }

        // 如果路径不存在（常见于 write），检查规范化后的路径是否以 base 开头
        if normalized.starts_with(&base) {
            return Ok(normalized);
        }

        // 尝试 canonicalize 父目录
        if let Some(parent) = path.parent() {
            if let Ok(canonical_parent) = parent.canonicalize() {
                if !canonical_parent.starts_with(&base) {
                    return Err(format!("路径逃逸出工作目录: {}", p));
                }
                return Ok(canonical_parent.join(path.file_name().unwrap_or_default()));
            }
        }

        Err(format!("路径解析失败: {}", p))
    }

    /// 手动规范化路径：解析 . 和 .. 组件，防止路径遍历。
    fn normalize_path(path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::Normal(c) => {
                    result.push(c);
                }
                std::path::Component::CurDir => {}
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    result.push(component);
                }
            }
        }
        result
    }

    // =========================================================================
    // 同步函数：读取文件内容
    // =========================================================================
    // `BufReader` 带缓冲的读取器，减少系统调用次数，提升 IO 性能
    // `collect::<Result<Vec<_>, _>>()` 把迭代器收集成 Result，任何一行读取失败都会提前返回错误

    pub fn run_read(
        path: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<String, String> {
        let path = Self::safe_path(path)?;
        log_trace!(
            "run_read | path={:?} | limit={:?} | offset={:?}",
            path,
            limit,
            offset
        );

        let file = File::open(&path).map_err(|e| format!("Error: {}", e))?;

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Error: {}", e))?;

        let total_lines = lines.len();
        let start = offset.unwrap_or(1).saturating_sub(1); // 1-based → 0-based
        let start = min(start, total_lines);
        let end = if let Some(lim) = limit {
            min(start + lim, total_lines)
        } else {
            total_lines
        };

        if start >= total_lines {
            return Ok("".to_string());
        }

        let selected: Vec<String> = lines[start..end].to_vec();
        let mut result = selected.join("\n");

        if end < total_lines {
            result.push_str(&format!("\n... ({} more lines)", total_lines - end));
        }
        if start > 0 {
            result = format!("... ({} lines skipped)\n{}", start, result);
        }

        Ok(result
            .chars()
            .take(OUTPUT_TRUNCATE_LENGTH)
            .collect::<String>())
    }

    // =========================================================================
    // 同步函数：执行 bash 命令（带 120 秒超时）
    // =========================================================================
    // 为了不依赖 tokio 运行时，这里使用 `std::process::Command` 在独立线程中执行命令，
    // 主线程通过 `mpsc::channel` 接收结果，并用 `recv_timeout` 实现超时控制。

    pub fn run_bash(command: &str) -> String {
        log_trace!("run_bash | command={}", command);
        let command = command.to_string();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let compat_mode = get_compat_mode();

            let result = match compat_mode {
                WindowsCompatMode::Native => std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env(
                        "HOME",
                        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
                    )
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output(),
                WindowsCompatMode::Wsl2 => std::process::Command::new("wsl.exe")
                    .arg("sh")
                    .arg("-c")
                    .arg(&command)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output(),
                WindowsCompatMode::GitBash | WindowsCompatMode::Cygwin => {
                    if let Some(bash_path) = get_bash_path() {
                        std::process::Command::new(bash_path)
                            .arg("-c")
                            .arg(&command)
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .output()
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Bash executable not found",
                        ))
                    }
                }
                WindowsCompatMode::None => {
                    let error_msg =
                        "Error: 未找到兼容的 bash 环境。请安装 WSL2、Git Bash 或 Cygwin。";
                    return tx
                        .send(Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            error_msg,
                        )))
                        .unwrap();
                }
            };

            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_secs(BASH_TIMEOUT_SECS)) {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let combined = format!("{}{}", stdout, stderr).trim().to_string();
                log_trace!(
                    "run_bash result | len={} | preview={}",
                    combined.len(),
                    combined.chars().take(200).collect::<String>()
                );

                if combined.is_empty() {
                    "(no output)".to_string()
                } else {
                    combined.chars().take(OUTPUT_TRUNCATE_LENGTH).collect()
                }
            }
            Ok(Err(e)) => format!("Error: {}", e),
            Err(_) => "Error: Timeout (120s)".to_string(),
        }
    }

    // =========================================================================
    // 同步函数：写入文件
    // =========================================================================
    // `create_dir_all` 递归创建父目录，`.parent()` 获取文件所在目录
    // `std::fs::write` 是标准库提供的同步写文件 API

    pub fn run_write(path: &str, content: &str) -> Result<String, String> {
        let fp = Self::safe_path(path)?;
        log_trace!("run_write | path={:?} | content_len={}", fp, content.len());

        // 读取原文件内容（用于 diff）
        let original_content = std::fs::read_to_string(&fp).ok();
        let is_new_file = original_content.is_none();

        if let Some(parent) = fp.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Error: {e}"))?;
        }
        std::fs::write(&fp, content).map_err(|e| format!("Error: {e}"))?;

        // 计算 diff
        let diff_text = original_content.and_then(|orig| {
            use similar::{ChangeTag, TextDiff};
            let diff = TextDiff::from_lines(orig.as_str(), content);
            let mut result = String::new();
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                result.push_str(&format!("{}{}", sign, change.value()));
            }
            if result.is_empty() {
                None
            } else {
                Some(result)
            }
        });

        if let Some(diff) = diff_text {
            Ok(diff)
        } else if is_new_file {
            Ok(format!("New file: {} ({} bytes)", path, content.len()))
        } else {
            Ok(format!("Wrote {} bytes to {}", content.len(), path))
        }
    }

    // =========================================================================
    // 同步函数：编辑文件（文本替换）
    // =========================================================================
    // `replacen(old, new, 1)` 只替换第一次出现的位置，避免全局替换导致误伤
    // 替换前先检查 `contains`，给调用方更明确的错误信息

    pub fn run_edit(path: &str, old_text: &str, new_text: &str) -> Result<String, String> {
        let fp = Self::safe_path(path)?;
        log_trace!(
            "run_edit | path={:?} | old_len={} | new_len={}",
            fp,
            old_text.len(),
            new_text.len()
        );
        let content = std::fs::read_to_string(&fp).map_err(|e| format!("Error: {}", e))?;

        if !content.contains(old_text) {
            return Err(format!("Error: Text not found in {}", path));
        }

        let new_content = content.replacen(old_text, new_text, 1);

        std::fs::write(&fp, &new_content).map_err(|e| format!("Error: {}", e))?;

        // 计算 diff
        use similar::{ChangeTag, TextDiff};
        let diff = TextDiff::from_lines(&content, &new_content);
        let mut diff_text = String::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            diff_text.push_str(&format!("{}{}", sign, change.value()));
        }
        let diff_opt = if diff_text.is_empty() {
            None
        } else {
            Some(diff_text)
        };

        if let Some(diff) = diff_opt {
            Ok(diff)
        } else {
            Ok(format!("Edited {} (no changes)", path))
        }
    }

    // =========================================================================
    // 同步函数：根据 URL 获取网页内容并转换为 Markdown
    // =========================================================================

    pub fn run_web_fetch(url: &str) -> Result<String, String> {
        log_trace!("run_web_fetch | url={}", url);
        let resp = reqwest::blocking::get(url).map_err(|e| format!("Request failed: {}", e))?;
        let html = resp
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))?;
        let md = html2md::parse_html(&html);
        Ok(md.chars().take(OUTPUT_TRUNCATE_LENGTH).collect())
    }

    // =========================================================================
    // 同步函数：递归搜索目录下匹配正则的文件内容
    // =========================================================================

    /// 使用系统 ripgrep 进行智能搜索，自动尊重 .gitignore
    fn rg_smart_grep(dir: &std::path::Path, pattern: &str) -> Result<String, String> {
        log_trace!("rg_smart_grep | dir={:?} | pattern={}", dir, pattern);

        let mut cmd = std::process::Command::new("rg");
        cmd.arg(pattern)
            .arg(dir)
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("--follow");

        let output = cmd
            .output()
            .map_err(|e| format!("rg execution failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // rg 返回 1 表示无匹配，这是正常情况
            if output.status.code() == Some(1) && stderr.is_empty() {
                return Ok("No matches found".to_string());
            }
            return Err(format!("rg error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line_count = stdout.lines().count();
        let mut result = if line_count > 500 {
            let mut truncated: String = stdout.lines().take(500).collect::<Vec<_>>().join("\n");
            truncated.push_str("\n... (too many matches)");
            truncated
        } else {
            stdout.to_string()
        };
        if result.len() > OUTPUT_TRUNCATE_LENGTH {
            result = result.chars().take(OUTPUT_TRUNCATE_LENGTH).collect();
        }
        Ok(result)
    }

    pub fn run_grep(dir: &str, pattern: &str) -> Result<String, String> {
        let dir = Self::safe_path(dir)?;
        log_trace!("run_grep | dir={:?} | pattern={}", dir, pattern);

        // 统一在校验 regex 合法性后再分支，确保 rg 和 fallback 行为一致
        let re = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;

        if *RG_AVAILABLE {
            log_debug!("run_grep | using ripgrep");
            return Self::rg_smart_grep(&dir, pattern);
        }

        log_debug!("run_grep | using fallback recursive grep");
        let mut matches = Vec::new();
        Self::grep_recursive(&dir, &re, &mut matches)?;

        if matches.is_empty() {
            Ok("No matches found".to_string())
        } else {
            let result = matches.join("\n");
            Ok(result.chars().take(OUTPUT_TRUNCATE_LENGTH).collect())
        }
    }

    fn grep_file(
        path: &std::path::Path,
        re: &regex::Regex,
        matches: &mut Vec<String>,
    ) -> Result<(), String> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Ok(());
        };
        let relative = path
            .strip_prefix(&workspace_dir())
            .unwrap_or(path)
            .display()
            .to_string();
        for (line_num, line) in content.lines().enumerate() {
            if !re.is_match(line) {
                continue;
            }
            matches.push(format!("{}:{}: {}", relative, line_num + 1, line));
            if matches.len() >= 500 {
                matches.push("... (too many matches)".to_string());
                return Ok(());
            }
        }
        Ok(())
    }

    fn grep_recursive(
        dir: &std::path::Path,
        re: &regex::Regex,
        matches: &mut Vec<String>,
    ) -> Result<(), String> {
        for result in Walk::new(dir) {
            let entry = match result {
                Ok(e) => e,
                Err(e) => {
                    log_trace!("walk error: {}", e);
                    continue;
                }
            };

            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            // 跳过隐藏文件（与 rg 默认行为保持一致）
            if is_hidden(&entry) {
                continue;
            }

            Self::grep_file(entry.path(), re, matches)?;
        }
        Ok(())
    }

    pub fn run_glob(pattern: &str, dir: Option<&str>) -> Result<String, String> {
        let base = workspace_dir();
        let search_dir = match dir {
            Some(d) => {
                let safe_dir = Self::safe_path(d)?;
                safe_dir
            }
            None => base.clone(),
        };

        log_trace!("run_glob | pattern={} | dir={:?}", pattern, search_dir);

        // 优先使用 ripgrep --files（自动尊重 .gitignore，性能更好）
        if *RG_AVAILABLE {
            return Self::rg_smart_glob(pattern, &search_dir, &base);
        }

        // Fallback：使用 ignore::Walk + glob 过滤（同样尊重 .gitignore）
        Self::walk_glob(pattern, &search_dir, &base)
    }

    /// 使用 ripgrep --files 快速列出文件，再按 glob 模式过滤
    fn rg_smart_glob(pattern: &str, search_dir: &Path, base: &Path) -> Result<String, String> {
        log_trace!("rg_smart_glob | pattern={} | dir={:?}", pattern, search_dir);

        let output = std::process::Command::new("rg")
            .arg("--files")
            .arg(search_dir)
            .arg("--follow")
            .output()
            .map_err(|e| format!("rg --files execution failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // rg 返回 1 表示无文件，这是正常情况
            if output.status.code() == Some(1) && stderr.is_empty() {
                return Ok("No files found matching pattern".to_string());
            }
            return Err(format!("rg error: {}", stderr));
        }

        let glob_pattern =
            glob::Pattern::new(pattern).map_err(|e| format!("Invalid glob pattern: {}", e))?;

        let mut files = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            let path = Path::new(line);
            // 转换为相对路径进行 glob 匹配
            let relative = path.strip_prefix(base).unwrap_or(path);
            let relative_str = relative.to_str().unwrap_or("");

            if !glob_pattern.matches(relative_str) {
                continue;
            }

            files.push(relative_str.to_string());

            if files.len() >= 1000 {
                files.push("... (too many matches)".to_string());
                break;
            }
        }

        if files.is_empty() {
            Ok("No files found matching pattern".to_string())
        } else {
            let result = files.join("\n");
            Ok(result.chars().take(OUTPUT_TRUNCATE_LENGTH).collect())
        }
    }

    /// Fallback：使用 ignore::Walk 遍历，再按 glob 模式过滤
    fn walk_glob(pattern: &str, search_dir: &Path, base: &Path) -> Result<String, String> {
        log_trace!("walk_glob | pattern={} | dir={:?}", pattern, search_dir);

        let glob_pattern =
            glob::Pattern::new(pattern).map_err(|e| format!("Invalid glob pattern: {}", e))?;

        let mut files = Vec::new();

        for result in Walk::new(search_dir) {
            let entry = match result {
                Ok(e) => e,
                Err(e) => {
                    log_trace!("walk error: {}", e);
                    continue;
                }
            };

            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            // 跳过隐藏文件（与 rg 默认行为保持一致）
            if is_hidden(&entry) {
                continue;
            }

            let path = entry.path();
            let relative = path.strip_prefix(base).unwrap_or(path);
            let relative_str = relative.to_str().unwrap_or("");

            if !glob_pattern.matches(relative_str) {
                continue;
            }

            files.push(relative_str.to_string());

            if files.len() >= 1000 {
                files.push("... (too many matches)".to_string());
                break;
            }
        }

        if files.is_empty() {
            Ok("No files found matching pattern".to_string())
        } else {
            let result = files.join("\n");
            Ok(result.chars().take(OUTPUT_TRUNCATE_LENGTH).collect())
        }
    }

    // =========================================================================
    // Git 命令执行
    // =========================================================================
    // 通用的 git 命令执行函数，所有具体 git 工具都基于此构建

    pub fn run_git_command(args: &[&str]) -> String {
        use std::process::Command;
        use std::thread;

        log_trace!("run_git_command | args={:?}", args);

        let (tx, rx) = mpsc::channel();

        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        thread::spawn(move || {
            let output = Command::new("git")
                .args(&args_vec)
                .current_dir(workspace_dir())
                .output();

            let _ = tx.send(output);
        });

        match rx.recv_timeout(Duration::from_secs(BASH_TIMEOUT_SECS)) {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let combined = format!("{}{}", stdout, stderr).trim().to_string();
                log_trace!(
                    "run_git_command result | len={} | preview={}",
                    combined.len(),
                    combined.chars().take(200).collect::<String>()
                );

                if combined.is_empty() {
                    "(no output)".to_string()
                } else {
                    combined.chars().take(OUTPUT_TRUNCATE_LENGTH).collect()
                }
            }
            Ok(Err(e)) => format!("Error: {}", e),
            Err(_) => "Error: Timeout (120s)".to_string(),
        }
    }

    pub fn run_git_status() -> String {
        Self::run_git_command(&["status"])
    }

    pub fn run_git_diff(path: Option<&str>) -> String {
        match path {
            Some(p) => Self::run_git_command(&["diff", p]),
            None => Self::run_git_command(&["diff"]),
        }
    }

    pub fn run_git_add(files: &[&str]) -> String {
        let mut args = vec!["add"];
        args.extend(files.iter());
        Self::run_git_command(&args)
    }

    pub fn run_git_commit(message: &str) -> String {
        Self::run_git_command(&["commit", "-m", message])
    }

    pub fn run_git_log(limit: Option<usize>) -> String {
        match limit {
            Some(n) => {
                let n_str = format!("-{}", n);
                Self::run_git_command(&["log", &n_str])
            }
            None => Self::run_git_command(&["log"]),
        }
    }

    pub fn run_git_worktree(args: &[&str]) -> String {
        let mut git_args = vec!["worktree"];
        git_args.extend(args.iter());
        Self::run_git_command(&git_args)
    }

    pub fn git_write_tree() -> Result<String, String> {
        use std::process::Command;
        let output = Command::new("git")
            .args(["write-tree"])
            .current_dir(std::env::current_dir().map_err(|e| e.to_string())?)
            .output()
            .map_err(|e| format!("Failed to run git write-tree: {}", e))?;

        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(hash)
        } else {
            Err(format!(
                "git write-tree failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

// =============================================================================
// 单元测试
// =============================================================================
// `#[cfg(test)]` 表示这部分代码只在运行 `cargo test` 时编译

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::workspace::set_workspace;

    fn ensure_workspace() {
        set_workspace(std::env::current_dir().expect("获取当前目录失败"));
    }

    #[test]
    fn test_run_read() {
        ensure_workspace();
        let content = BasicTool::run_read(
            "src/tools/basic_tools.rs",
            Some(DEFAULT_READ_MAX_LINES),
            None,
        )
        .unwrap();
        assert_ne!(content, "");
        // 验证返回的是纯文本而非 JSON
        assert!(
            !content.starts_with('{'),
            "run_read should return plain text, not JSON"
        );
    }

    #[test]
    fn test_run_read_with_offset() {
        ensure_workspace();
        let content = BasicTool::run_read("src/tools/basic_tools.rs", Some(5), Some(1)).unwrap();
        // 从第1行开始读5行，应该包含 MIT License 头
        assert!(content.contains("MIT License"), "Should contain first line");

        let content_offset =
            BasicTool::run_read("src/tools/basic_tools.rs", Some(5), Some(10)).unwrap();
        // 从第10行开始，应该不包含 MIT License 头
        assert!(
            !content_offset.contains("MIT License"),
            "Should skip first lines"
        );
        assert!(
            content_offset.contains("... (9 lines skipped)"),
            "Should show skipped lines hint"
        );
    }

    #[test]
    fn test_run_bash() {
        ensure_workspace();
        let result = BasicTool::run_bash("ls -l");
        debug_assert_ne!(result, "");
    }

    #[test]
    fn test_run_write() {
        ensure_workspace();
        let path: &str = "target/test_write_file";
        let result = BasicTool::run_write(path, "test");
        assert!(result.is_ok());
        let content = result.unwrap();
        // 新文件应返回提示文本
        assert!(
            content.contains("New file") || content.contains("Wrote"),
            "write result should be plain text, got: {}",
            content
        );
        BasicTool::run_bash(&format!("rm {}", path));
    }

    #[test]
    fn test_run_edit() {
        ensure_workspace();
        let path = "target/test_edit_file";
        let result = BasicTool::run_write(path, "this is a test file");
        assert!(result.is_ok());
        let result = BasicTool::run_edit(path, "test file", "test edit file");
        assert!(result.is_ok());
        let content = result.unwrap();
        // diff 文本应包含 +/- 标记，或无变化提示
        assert!(
            content.contains('+') || content.contains("no changes"),
            "edit result should contain diff markers or no-changes hint, got: {}",
            content
        );
        BasicTool::run_bash(&format!("rm {}", path));
    }

    #[test]
    fn test_run_grep() {
        ensure_workspace();
        let result = BasicTool::run_grep("src/tools", "run_read");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.contains("run_read"),
            "grep should find 'run_read', got: {}",
            output
        );
    }

    #[test]
    fn test_run_grep_no_matches() {
        ensure_workspace();
        let dir = "target/test_grep_dir";
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(format!("{}/test.txt", dir), "hello world").unwrap();
        let result = BasicTool::run_grep(dir, "___NONEXISTENT___");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No matches found");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_run_glob() {
        ensure_workspace();
        let result = BasicTool::run_glob("**/Cargo.toml", None);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Cargo.toml"));
    }

    #[test]
    fn test_run_glob_no_matches() {
        ensure_workspace();
        let result = BasicTool::run_glob("**/nonexistent_file_1234.xyz", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No files found matching pattern");
    }

    #[test]
    fn test_run_git_command() {
        ensure_workspace();
        let result = BasicTool::run_git_command(&["status"]);
        // 只检查命令执行没有错误，输出内容是变化的
        assert!(!result.is_empty());
    }

    #[test]
    fn test_run_git_status() {
        ensure_workspace();
        let result = BasicTool::run_git_status();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_run_git_log() {
        ensure_workspace();
        let result = BasicTool::run_git_log(Some(5));
        assert!(!result.is_empty());
    }

    #[test]
    fn test_git_write_tree() {
        ensure_workspace();
        let result = BasicTool::git_write_tree();
        // 在 git 仓库中应该成功，否则可能失败
        if std::path::Path::new(".git").exists() {
            assert!(
                result.is_ok(),
                "git write-tree should succeed in a git repo"
            );
            let hash = result.unwrap();
            assert!(!hash.is_empty(), "tree hash should not be empty");
        }
    }

    #[test]
    fn test_grep_respects_gitignore() {
        use std::fs;

        let tmp = tempfile::Builder::new()
            .prefix("testgrep")
            .tempdir_in(".")
            .unwrap();
        let root = tmp.path();

        // 正常文件应该被搜到
        fs::write(root.join("main.rs"), "fn main() {\n    let x = 42;\n}\n").unwrap();

        // target/ 目录被 .gitignore 排除，不应该被搜到
        let target_dir = root.join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("cached.rs"), "let x = 42;\n").unwrap();

        // .git/ 目录被 .gitignore 排除，不应该被搜到
        let git_dir = root.join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("config"), "let x = 42;\n").unwrap();

        // node_modules/ 目录被 .gitignore 排除，不应该被搜到
        let node_dir = root.join("node_modules");
        fs::create_dir(&node_dir).unwrap();
        fs::write(node_dir.join("index.js"), "let x = 42;\n").unwrap();

        // 创建 .gitignore 文件
        fs::write(root.join(".gitignore"), "target/\n.git/\nnode_modules/\n").unwrap();

        let result = BasicTool::run_grep(root.to_str().unwrap(), "let x = 42").unwrap();
        assert!(result.contains("main.rs"), "Should find match in main.rs");
        assert!(
            !result.contains("cached.rs"),
            "Should skip target/ directory via .gitignore"
        );
        assert!(
            !result.contains("index.js"),
            "Should skip node_modules/ directory via .gitignore"
        );
    }

    #[test]
    fn test_grep_skips_hidden_files() {
        use std::fs;

        let tmp = tempfile::Builder::new()
            .prefix("testgrep")
            .tempdir_in(".")
            .unwrap();
        let root = tmp.path();

        // 正常文件应该被搜到
        fs::write(root.join("visible.rs"), "let secret = 42;\n").unwrap();

        // 隐藏文件不应该被搜到
        fs::write(root.join(".hidden.rs"), "let secret = 42;\n").unwrap();

        let result = BasicTool::run_grep(root.to_str().unwrap(), "let secret = 42").unwrap();
        assert!(
            result.contains("visible.rs"),
            "Should find match in visible.rs"
        );
        assert!(!result.contains(".hidden.rs"), "Should skip hidden files");
    }

    #[test]
    fn test_grep_skips_hidden_dirs() {
        use std::fs;

        let tmp = tempfile::Builder::new()
            .prefix("testgrep")
            .tempdir_in(".")
            .unwrap();
        let root = tmp.path();

        // 正常文件应该被搜到
        fs::write(root.join("normal.rs"), "let hidden_dir_test = 42;\n").unwrap();

        // 隐藏目录下的文件不应该被搜到
        let hidden_dir = root.join(".hidden_dir");
        fs::create_dir(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("inside.rs"), "let hidden_dir_test = 42;\n").unwrap();

        let result =
            BasicTool::run_grep(root.to_str().unwrap(), "let hidden_dir_test = 42").unwrap();
        assert!(
            result.contains("normal.rs"),
            "Should find match in normal.rs"
        );
        assert!(
            !result.contains("inside.rs"),
            "Should skip files inside hidden directories"
        );
    }
}
