use crate::log_trace;
use crate::utils::workspace::workspace_dir;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

// =============================================================================
// BasicTool：最底层的文件与命令操作封装
// =============================================================================
// 这是一个"零尺寸类型"（Zero-Sized Type, ZST），因为它不包含任何字段。
// 用空结构体 + `impl` 块来组织静态方法，是一种常见的工具类写法。

pub struct BasicTool {}

impl BasicTool {
    // =========================================================================
    // 安全路径检查
    // =========================================================================
    // `canonicalize` 会把路径解析为绝对路径，并消除 `.` 和 `..`
    // `starts_with` 确保用户不会通过 `../../etc/passwd` 这种方式逃逸出工作目录

    fn safe_path(p: &str) -> Result<PathBuf, String> {
        let base = workspace_dir();
        let path = base.join(p);
        // 如果路径已存在，直接 canonicalize 并检查是否在工作目录内
        if let Ok(canonical) = path.canonicalize() {
            if !canonical.starts_with(&base) {
                return Err(format!("路径逃逸出工作目录: {}", p));
            }
            return Ok(canonical);
        }
        // 如果路径不存在（常见于 write），尝试 canonicalize 父目录
        if let Some(parent) = path.parent() {
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| format!("路径解析失败: {}", e))?;
            if !canonical_parent.starts_with(&base) {
                return Err(format!("路径逃逸出工作目录: {}", p));
            }
            return Ok(canonical_parent.join(path.file_name().unwrap_or_default()));
        }
        Err(format!("路径解析失败: {}", p))
    }

    // =========================================================================
    // 同步函数：读取文件内容
    // =========================================================================
    // `BufReader` 带缓冲的读取器，减少系统调用次数，提升 IO 性能
    // `collect::<Result<Vec<_>, _>>()` 把迭代器收集成 Result，任何一行读取失败都会提前返回错误

    pub fn run_read(path: &str, limit: Option<usize>) -> Result<String, String> {
        let path = Self::safe_path(path)?;
        log_trace!("run_read | path={:?} | limit={:?}", path, limit);

        let file = File::open(&path).map_err(|e| format!("Error: {}", e))?;

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Error: {}", e))?;

        let result = if let Some(lim) = limit {
            if lim < lines.len() {
                let mut result: Vec<String> =
                    lines.iter().take(lim).map(|s| s.to_string()).collect();
                result.push(format!("... ({} more)", lines.len() - lim));
                result.join("\n")
            } else {
                lines.join("\n")
            }
        } else {
            lines.join("\n")
        };

        // 限制返回内容最大 50000 字符，防止一次性返回超大文件撑爆上下文
        Ok(result.chars().take(50000).collect())
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
            // 方案 3：沙箱化执行
            // 1. 清除所有继承的环境变量，阻断 LD_PRELOAD / BASH_ENV / ENV / IFS 等注入通道
            // 2. 仅注入最小必要环境（PATH + HOME），防止 PATH 劫持
            let result = std::process::Command::new("sh")
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
                .output();
            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(output)) => {
                // `String::from_utf8_lossy` 将字节转换为字符串，处理非法 UTF-8 序列
                // `&output.stdout` 借用 output 的标准输出字段
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // `format!` 宏创建格式化字符串
                // `.trim()` 去除首尾空白，返回 &str
                // `.to_string()` 将 &str 转换为拥有的 String
                let combined = format!("{}{}", stdout, stderr).trim().to_string();
                log_trace!(
                    "run_bash result | len={} | preview={}",
                    combined.len(),
                    combined.chars().take(200).collect::<String>()
                );

                if combined.is_empty() {
                    "(no output)".to_string()
                } else {
                    // `.chars()` 创建字符迭代器
                    // `.take(50000)` 只取前 50000 个字符
                    // `.collect()` 将迭代器收集为集合（这里是 String）
                    combined.chars().take(50000).collect()
                }
            }
            // `Ok(Err(e))`：命令执行失败
            Ok(Err(e)) => format!("Error: {}", e),
            // `Err(_)`：timeout 超时，`_` 是通配符，忽略具体值
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
        if let Some(parent) = fp.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Error: {e}"))?;
        }
        std::fs::write(&fp, content).map_err(|e| format!("Error: {e}"))?;
        Ok(format!("Wrote {} bytes", content.len()))
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

        std::fs::write(&fp, new_content).map_err(|e| format!("Error: {}", e))?;

        Ok(format!("Edited {}", path))
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
        Ok(md.chars().take(50000).collect())
    }

    // =========================================================================
    // 同步函数：递归搜索目录下匹配正则的文件内容
    // =========================================================================

    pub fn run_grep(dir: &str, pattern: &str) -> Result<String, String> {
        let dir = Self::safe_path(dir)?;
        log_trace!("run_grep | dir={:?} | pattern={}", dir, pattern);
        let re = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;

        let mut matches = Vec::new();
        Self::grep_recursive(&dir, &re, &mut matches)?;

        if matches.is_empty() {
            Ok("No matches found".to_string())
        } else {
            let result = matches.join("\n");
            Ok(result.chars().take(50000).collect())
        }
    }

    fn grep_recursive(
        dir: &std::path::Path,
        re: &regex::Regex,
        matches: &mut Vec<String>,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("Error reading dir: {}", e))? {
            let entry = entry.map_err(|e| format!("Error reading entry: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                Self::grep_recursive(&path, re, matches)?;
            } else if path.is_file() {
                // 忽略二进制文件：read_to_string 失败就跳过
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let relative = path
                        .strip_prefix(&workspace_dir())
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    for (line_num, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            matches.push(format!("{}:{}: {}", relative, line_num + 1, line));
                            if matches.len() >= 500 {
                                matches.push("... (too many matches)".to_string());
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
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
        let lines = BasicTool::run_read("src/tools/basic_tools.rs", Some(10000)).unwrap();
        assert_ne!(lines, "");
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
}
