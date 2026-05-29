# Smart Grep Tool Design

## 目标

改进 fi-code 的 `grep` 工具，使其自动排除依赖目录、编译产物和隐藏文件；优先调用系统 `ripgrep`，无则回退到带过滤的 Rust 内置遍历。

## 背景

当前 `BasicTool::run_grep()` 使用递归 `read_dir` 遍历所有文件，无任何路径过滤，导致搜索结果包含 `target/`、`.cargo/registry/`、`.git/`、`.worktrees/` 等非源码内容。用户提供的参考实现（Python）展示了三层过滤（黑名单 + 隐藏文件 + .gitignore）和 ripgrep 优先策略，本设计将其移植到 Rust 端。

## 架构

```
run_grep(dir, pattern)
  ├── 检测 rg 是否可用
  │     ├── 是 → rg_smart_grep(dir, pattern) → 返回
  │     └── 否 → fallback_grep(dir, pattern)
  │
  rg_smart_grep
    └── Command::new("rg")
          .arg(pattern)
          .arg(dir)
          .arg("--line-number")
          .arg("--no-heading")
          .arg("--max-count").arg("3")
          .arg("--max-columns").arg("200")
          .arg("--glob").arg("!**/node_modules/**")
          .arg("--glob").arg("!**/target/**")
          ... (BLOCKED_DIRS 转为 glob)
          .arg("--no-hidden")
          .arg("--follow")
          .arg("--threads").arg("4")

  fallback_grep
    └── walkdir::WalkDir::new(dir)
          .filter_entry(|e| !is_blocked(e))
          .into_iter()
          .filter_map(|e| e.ok())
          .filter(|e| !is_hidden(e))
          → 现有 line-by-line 匹配逻辑
```

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 黑名单常量 | `BLOCKED_DIRS`（用户提供的列表） | 硬编码在 Rust 中，避免运行时配置复杂化 |
| rg 不可用时遍历库 | `walkdir`（项目已有依赖） | 无需新增 crate |
| .gitignore 支持 | rg 天然支持；fallback 不实现 | fallback 场景较少，保持简单 |
| 扩展名过滤 | fallback 中启用，rg 中不加 `--type` | 避免 rg 过滤掉 `.md`、`.toml` 等非代码文件 |
| 输出截断 | 保留现有 500 匹配 / 50KB 截断 | 与现有行为一致，防止 LLM 上下文爆炸 |
| 参数接口 | 不变，仍为 `{"dir": "...", "pattern": "..."}` | 零 breaking change |

## 黑名单（BLOCKED_DIRS）

```rust
const BLOCKED_DIRS: &[&str] = &[
    "node_modules", "__pycache__", ".git", ".svn", ".hg",
    "target", "dist", "build", ".cargo", ".rustup",
    "venv", ".venv", "env", ".env", ".tox",
    ".idea", ".vscode", ".worktrees",
    "vendor", "third_party", "deps",
    ".pytest_cache", ".mypy_cache", ".ruff_cache",
    "out", ".next", ".nuxt", "coverage", "htmlcov",
];
```

## 文件改动范围

- **`crates/core/src/tools/basic_tools.rs`**：
  - 新增 `BLOCKED_DIRS` 常量
  - 新增 `is_blocked_dir()` 辅助函数
  - 新增 `rg_smart_grep()` 函数
  - 修改 `run_grep()`：先检测 rg，再决定走哪条路径
  - 修改 `grep_recursive()`：加入 `filter_entry` 黑名单过滤
- **`crates/core/src/tools/mod.rs`**：无需改动（参数 schema 不变）
- **测试**：新增 rg 路径和 fallback 路径的过滤测试

## 边界情况

- **rg 存在但版本旧**：用 `--version` 检测，若不支持某些 flag 则降级
- **dir 参数是文件而非目录**：`safe_path` 已处理；rg 也支持搜单文件
- **pattern 非法 regex**：rg 会返回 stderr，fallback 保持现有 `Regex::new` 错误处理
- **Windows**：`rg` 有 Windows 构建，glob 语法一致
- **rg 未安装**：`which::which("rg")` 检测失败，平滑回退到 fallback

## 非目标（YAGNI）

- 不引入 `ignore` crate（ripgrep 的 Rust 库），因为方案 A 已通过系统 rg 解决
- 不实现 fallback 中的 `.gitignore` 解析（复杂度收益比低）
- 不新增 `include`/`exclude` 参数（保持 schema 不变）
