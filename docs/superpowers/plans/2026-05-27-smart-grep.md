# Smart Grep Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 改进 `grep` 工具，自动排除黑名单目录和隐藏文件，优先使用系统 ripgrep，否则回退到带过滤的 Rust 内置遍历。

**Architecture:** 在 `BasicTool` 中新增 `BLOCKED_DIRS` 常量和路径过滤辅助函数；`run_grep` 先检测 `rg` 可用性，可用则通过 `std::process::Command` 调用 ripgrep 并传黑名单 glob，不可用则回退到改造后的 `grep_recursive`（使用 `walkdir::filter_entry` 跳过黑名单目录）。

**Tech Stack:** Rust, `walkdir`（已有依赖）, `regex`（已有依赖）, 系统 `ripgrep`（可选外部依赖）

---

### Task 1: 添加黑名单常量和路径过滤辅助函数

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs:1-50`（在 `BasicTool` impl 上方添加常量）

- [ ] **Step 1: 添加 `BLOCKED_DIRS` 常量和辅助函数**

在 `crates/core/src/tools/basic_tools.rs` 中，找到 `pub struct BasicTool;` 之后、`impl BasicTool` 之前的位置，添加：

```rust
// 智能 grep 黑名单：这些目录永远不会被搜索
const BLOCKED_DIRS: &[&str] = &[
    "node_modules", "__pycache__", ".git", ".svn", ".hg",
    "target", "dist", "build", ".cargo", ".rustup",
    "venv", ".venv", "env", ".env", ".tox",
    ".idea", ".vscode", ".worktrees",
    "vendor", "third_party", "deps",
    ".pytest_cache", ".mypy_cache", ".ruff_cache",
    "out", ".next", ".nuxt", "coverage", "htmlcov",
];

/// 检查路径是否包含黑名单中的目录段
fn is_blocked_path(path: &std::path::Path) -> bool {
    path.components().any(|comp| {
        if let Some(name) = comp.as_os_str().to_str() {
            BLOCKED_DIRS.contains(&name)
        } else {
            false
        }
    })
}

/// 检查是否为隐藏文件/目录（以点开头的名称）
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
```

- [ ] **Step 2: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: `Finished dev profile`

---

### Task 2: 实现 ripgrep 优先搜索

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs:341-355`（替换 `run_grep` 内部逻辑）

- [ ] **Step 1: 在 `BasicTool` impl 中添加 `rg_smart_grep` 函数**

在 `run_grep` 函数上方（仍在 `impl BasicTool` 内）添加：

```rust
    /// 使用系统 ripgrep 进行智能搜索，自动排除黑名单目录和隐藏文件
    fn rg_smart_grep(dir: &std::path::Path, pattern: &str) -> Result<String, String> {
        let mut cmd = std::process::Command::new("rg");
        cmd.arg(pattern)
            .arg(dir)
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("--max-count").arg("3")
            .arg("--max-columns").arg("200")
            .arg("--no-hidden")
            .arg("--follow")
            .arg("--threads").arg("4");

        // 将 BLOCKED_DIRS 转为 rg 的 glob 排除规则
        for blocked in BLOCKED_DIRS {
            cmd.arg("--glob").arg(format!("!**/{}/**", blocked));
        }

        let output = cmd.output().map_err(|e| format!("rg execution failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // rg 返回 1 表示无匹配，这是正常情况
            if output.status.code() == Some(1) && stderr.is_empty() {
                return Ok("No matches found".to_string());
            }
            return Err(format!("rg error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = stdout.chars().take(OUTPUT_TRUNCATE_LENGTH).collect();
        Ok(result)
    }
```

- [ ] **Step 2: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: `Finished dev profile`

---

### Task 3: 修改 fallback 遍历逻辑并接入 rg 路由

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs:341-398`

- [ ] **Step 1: 重写 `run_grep` 做 rg 检测和路由**

将现有 `run_grep`（line 341-355）替换为：

```rust
    pub fn run_grep(dir: &str, pattern: &str) -> Result<String, String> {
        let dir = Self::safe_path(dir)?;
        log_trace!("run_grep | dir={:?} | pattern={}", dir, pattern);

        // 优先检测系统是否安装 ripgrep
        let rg_available = std::process::Command::new("rg")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if rg_available {
            log_debug!("run_grep | using ripgrep");
            return Self::rg_smart_grep(&dir, pattern);
        }

        log_debug!("run_grep | using fallback recursive grep");
        let re = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
        let mut matches = Vec::new();
        Self::grep_recursive(&dir, &re, &mut matches)?;

        if matches.is_empty() {
            Ok("No matches found".to_string())
        } else {
            let result = matches.join("\n");
            Ok(result.chars().take(OUTPUT_TRUNCATE_LENGTH).collect())
        }
    }
```

- [ ] **Step 2: 重写 `grep_recursive` 加入 walkdir + 黑名单过滤**

将现有 `grep_recursive`（line 383-398）替换为：

```rust
    fn grep_recursive(
        dir: &std::path::Path,
        re: &regex::Regex,
        matches: &mut Vec<String>,
    ) -> Result<(), String> {
        use walkdir::WalkDir;

        let walker = WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                // 跳过黑名单目录和隐藏目录
                if e.file_type().is_dir() {
                    if is_blocked_path(e.path()) {
                        return false;
                    }
                    if is_hidden(e) {
                        return false;
                    }
                }
                true
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log_trace!("walkdir error: {}", e);
                    continue;
                }
            };

            if !entry.file_type().is_file() {
                continue;
            }

            // 跳过隐藏文件
            if is_hidden(&entry) {
                continue;
            }

            Self::grep_file(entry.path(), re, matches)?;

            if matches.len() >= 500 {
                matches.push("... (too many matches)".to_string());
                return Ok(());
            }
        }
        Ok(())
    }
```

- [ ] **Step 3: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: `Finished dev profile`

- [ ] **Step 4: 运行现有测试确认不破坏原有行为**

Run: `cargo test -p fi-code-core --lib tools::tests::test_tool_call_grep`
Expected: 所有 grep 相关测试通过

---

### Task 4: 编写新增测试

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs`（在 `#[cfg(test)]` 模块中添加测试）

先找到现有测试的位置：

```bash
grep -n "cfg(test)" crates/core/src/tools/basic_tools.rs
```

- [ ] **Step 1: 添加黑名单过滤测试**

在 `#[cfg(test)]` mod 中添加：

```rust
    #[test]
    fn test_grep_skips_blocked_dirs() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // 正常文件应该被搜到
        fs::write(root.join("main.rs"), "fn main() {\n    let x = 42;\n}\n").unwrap();

        // target/ 目录下的文件不应该被搜到
        let target_dir = root.join("target");
        fs::create_dir(&target_dir).unwrap();
        fs::write(target_dir.join("cached.rs"), "let x = 42;\n").unwrap();

        // .git/ 目录下的文件不应该被搜到
        let git_dir = root.join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("config"), "let x = 42;\n").unwrap();

        // node_modules/ 目录下的文件不应该被搜到
        let node_dir = root.join("node_modules");
        fs::create_dir(&node_dir).unwrap();
        fs::write(node_dir.join("index.js"), "let x = 42;\n").unwrap();

        let result = BasicTool::run_grep(root.to_str().unwrap(), "let x = 42").unwrap();
        assert!(result.contains("main.rs"), "Should find match in main.rs");
        assert!(!result.contains("cached.rs"), "Should skip target/ directory");
        assert!(!result.contains(".git/config"), "Should skip .git/ directory");
        assert!(!result.contains("index.js"), "Should skip node_modules/ directory");
    }

    #[test]
    fn test_grep_skips_hidden_files() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // 正常文件应该被搜到
        fs::write(root.join("visible.rs"), "let secret = 42;\n").unwrap();

        // 隐藏文件不应该被搜到
        fs::write(root.join(".hidden.rs"), "let secret = 42;\n").unwrap();

        let result = BasicTool::run_grep(root.to_str().unwrap(), "let secret = 42").unwrap();
        assert!(result.contains("visible.rs"), "Should find match in visible.rs");
        assert!(!result.contains(".hidden.rs"), "Should skip hidden files");
    }
```

- [ ] **Step 2: 运行新增测试**

Run: `cargo test -p fi-code-core --lib basic_tools::tests::test_grep_skips_blocked_dirs`
Run: `cargo test -p fi-code-core --lib basic_tools::tests::test_grep_skips_hidden_files`
Expected: 两个测试都 PASS

- [ ] **Step 3: 运行全部工具测试做回归**

Run: `cargo test -p fi-code-core --lib tools::`
Expected: 所有测试通过

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/tools/basic_tools.rs
git commit -m "feat: smart grep with ripgrep priority and blocked directory filtering

- Add BLOCKED_DIRS constant to exclude common dependency/build directories
- Detect system ripgrep and use it when available (faster, respects .gitignore)
- Fallback to walkdir-based recursive grep with filter_entry for blocked dirs
- Skip hidden files and directories in both rg and fallback paths
- Add tests for blocked directory and hidden file filtering"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ 黑名单常量 → Task 1 Step 1
- ✅ ripgrep 优先检测 → Task 3 Step 1
- ✅ rg 调用带 glob 排除 → Task 2 Step 1
- ✅ fallback 回退逻辑 → Task 3 Step 2
- ✅ 隐藏文件过滤 → Task 3 Step 2 + Task 4 Step 1
- ✅ 输出截断保留 → Task 2/3 中 `take(OUTPUT_TRUNCATE_LENGTH)` 保留
- ✅ 零 breaking change → 参数接口未改

**2. Placeholder scan:**
- ✅ 无 TBD/TODO
- ✅ 每步都有具体代码
- ✅ 每步都有具体命令和预期输出

**3. Type consistency:**
- ✅ `run_grep` 签名不变：`Result<String, String>`
- ✅ `rg_smart_grep` 签名与 `run_grep` 内部一致
- ✅ `is_blocked_path` / `is_hidden` 为独立函数，不依赖 `BasicTool` self
