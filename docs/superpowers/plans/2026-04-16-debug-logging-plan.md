# 分层调试日志系统 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `shun-code` 引入 `debug` / `trace` 两级日志体系，统一格式并提升可读性，同时在 release 构建中编译期移除所有日志，防止敏感信息泄露。

**Architecture:** 扩展 `src/utils/log.rs` 为层级日志系统（`Off / Info / Debug / Trace`），提供 `log_info!` / `log_debug!` / `log_trace!` / `log_block!` 四个宏。在 `cfg(debug_assertions)` 下保留 `--log` 参数和日志功能；在 `not(debug_assertions)` 下将 `--log` 参数和所有日志宏编译为空。然后按设计文档在关键模块中分层插入日志打印。

**Tech Stack:** Rust standard library (`std::sync::atomic`, `std::time`), `chrono` (for timestamp formatting), `colored` (existing)

---

## File Mapping

| File | Responsibility |
|------|----------------|
| `src/utils/log.rs` | 核心日志基础设施：`LogLevel`、全局级别存储、`log_debug!` / `log_trace!` / `log_block!` 宏、编译期裁剪 |
| `src/utils/cli.rs` | CLI 参数定义；`--log` 字段加 `#[cfg(debug_assertions)]` |
| `src/main.rs` | 程序入口；日志初始化加编译条件；启动信息 `log_info!` 打印 |
| `src/agent/agent.rs` | Agent 循环；`debug` 打印回合摘要，`trace` 打印消息/Part 详情，首次 system prompt 用 `log_block!` |
| `src/provider/client/openapi_client.rs` | OpenAI 客户端；`debug` 打印请求摘要，`trace` 打印完整请求体和 SSE 原始数据 |
| `src/provider/client/anthropic_client.rs` | Anthropic 客户端；同上分层的日志分配 |
| `src/provider/base_client.rs` | HTTP 重试装饰器；`trace` 打印每次重试状态和退避时间 |
| `src/tools/mod.rs` | 工具执行入口；`debug` 打印工具名和结果摘要，`trace` 打印原始参数和输出 |
| `src/tools/basic_tools.rs` | 底层工具实现；`trace` 打印 bash/read/write/edit/grep/web_fetch 的原始 IO |
| `src/permission/permission.rs` | 权限检查；`debug` 打印检查结果（Allow/Ask/Deny + 原因） |
| `src/session/session.rs` | 会话管理；`debug` 打印 create/load/save 摘要 |

---

### Task 1: Add `chrono` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `chrono` dependency**

```toml
# Under [dependencies], add:
chrono = "0.4"
```

- [ ] **Step 2: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add chrono for timestamp formatting"
```

---

### Task 2: Build the tiered logging infrastructure

**Files:**
- Modify: `src/utils/log.rs`

- [ ] **Step 1: Write failing test for `LogLevel` parsing**

Replace the existing `src/utils/log.rs` test section with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("off"), LogLevel::Off);
        assert_eq!(LogLevel::from_str("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("trace"), LogLevel::Trace);
    }

    #[test]
    fn test_log_level_enabled() {
        assert!(LogLevel::Debug.enabled(LogLevel::Debug));
        assert!(LogLevel::Trace.enabled(LogLevel::Debug));
        assert!(!LogLevel::Info.enabled(LogLevel::Debug));
    }
}
```

Run: `cargo test utils::log::tests`
Expected: FAIL (types/functions not yet defined)

- [ ] **Step 2: Implement `src/utils/log.rs`**

Replace the entire contents of `src/utils/log.rs` with:

```rust
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Off = 0,
    Info = 1,
    Debug = 2,
    Trace = 3,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => LogLevel::Off,
            "debug" => LogLevel::Debug,
            "trace" => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }

    pub fn enabled(self, required: LogLevel) -> bool {
        self >= required
    }
}

static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn current_log_level() -> LogLevel {
    match LOG_LEVEL.load(Ordering::Relaxed) {
        0 => LogLevel::Off,
        2 => LogLevel::Debug,
        3 => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

fn log_prefix(level: &str, module: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("{} [{:<5}] [{:<30}]", now, level, module)
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Info) {
                eprintln!("{} {}", $crate::utils::log::log_prefix("INFO", module_path!()), format!($($arg)*));
            }
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Debug) {
                eprintln!("{} {}", $crate::utils::log::log_prefix("DEBUG", module_path!()), format!($($arg)*));
            }
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Trace) {
                eprintln!("{} {}", $crate::utils::log::log_prefix("TRACE", module_path!()), format!($($arg)*));
            }
        }
    };
}

#[macro_export]
macro_rules! log_block {
    ($level:expr, $title:expr, $content:expr) => {
        #[cfg(debug_assertions)]
        {
            let enabled = $crate::utils::log::current_log_level().enabled($level);
            if enabled {
                let prefix = $crate::utils::log::log_prefix(
                    match $level {
                        $crate::utils::log::LogLevel::Debug => "DEBUG",
                        $crate::utils::log::LogLevel::Trace => "TRACE",
                        _ => "INFO",
                    },
                    module_path!()
                );
                let sep_width = 50;
                eprintln!("{} {:=^sep_width$}", prefix, format!(" {} ", $title));
                for line in $content.lines() {
                    eprintln!("{} {}", prefix, line);
                }
                eprintln!("{} {:=^sep_width$}", prefix, "");
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("off"), LogLevel::Off);
        assert_eq!(LogLevel::from_str("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("trace"), LogLevel::Trace);
    }

    #[test]
    fn test_log_level_enabled() {
        assert!(LogLevel::Debug.enabled(LogLevel::Debug));
        assert!(LogLevel::Trace.enabled(LogLevel::Debug));
        assert!(!LogLevel::Info.enabled(LogLevel::Debug));
    }
}
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test utils::log::tests`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/utils/log.rs
git commit -m "feat(logging): add tiered log macros with compile-time release stripping"
```

---

### Task 3: Conditionally compile `--log` CLI argument

**Files:**
- Modify: `src/utils/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Modify `src/utils/cli.rs` to add `#[cfg(debug_assertions)]` on `--log`**

Replace the `log_level` field in `Args` with:

```rust
    /// Enable debug logging (debug|trace|info|off, default: info)
    #[cfg(debug_assertions)]
    #[arg(short = 'l', long = "log", value_name = "LEVEL", default_value = "info")]
    pub log_level: String,
```

- [ ] **Step 2: Modify `src/main.rs` to conditionally initialize log level**

Find the existing line:
```rust
    set_debug(args.log_level.eq_ignore_ascii_case("debug"));
```

Replace it with:

```rust
#[cfg(debug_assertions)]
    {
        use utils::log::{set_log_level, LogLevel};
        set_log_level(LogLevel::from_str(&args.log_level));
        log_info!("shun-code starting | log_level={}", args.log_level);
    }
```

Also remove the now-unused import `use utils::log::set_debug;` from `src/main.rs`.

- [ ] **Step 3: Run `cargo test` to ensure no compilation errors**

Run: `cargo test`
Expected: PASS (all existing tests)

- [ ] **Step 4: Commit**

```bash
git add src/utils/cli.rs src/main.rs
git commit -m "feat(logging): conditionally compile --log arg and init"
```

---

### Task 4: Instrument `agent::agent.rs` with tiered logging

**Files:**
- Modify: `src/agent/agent.rs`

- [ ] **Step 1: Replace logging in `run_one_turn` with new macros and add system prompt block logic**

Replace the contents of `src/agent/agent.rs` with the following. Key changes:
- Import `log_trace!` and `log_block!` in addition to `log_debug!`
- Use `static std::sync::Once` for first-time system prompt logging
- Add `trace`-level message dumps

```rust
// =============================================================================
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================
// 本模块定义了对话中使用的核心数据结构与 agent 循环：
// - `LoopState`：agent 循环的运行时状态
// - `run_one_turn` / `agent_loop`：单轮/多轮对话驱动逻辑
//
// 消息类型（Message / Part / Role / ImageSource）已从本模块迁移到
// 独立的 `message` 模块，供 session、provider、tools 等多个模块共享。

use anyhow::Result;

use crate::agent::PromptBuilder;
use crate::log_block;
use crate::log_debug;
use crate::log_trace;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
use crate::provider::execute_tool_calls;
use crate::session::message::{Message, Part, Role};
use crate::tools::tool_schema;

// =============================================================================
// 对话循环状态（LoopState）
// =============================================================================

#[derive(Debug)]
pub struct LoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition_reason: Option<String>,
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
        }
    }
}

static PROMPT_LOGGED_ONCE: std::sync::Once = std::sync::Once::new();

// =============================================================================
// 异步函数：运行一轮对话
// =============================================================================

pub async fn run_one_turn<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    let system_prompt = PromptBuilder::new().build(&tool_schema());

    PROMPT_LOGGED_ONCE.call_once(|| {
        log_block!(
            crate::utils::log::LogLevel::Debug,
            "SYSTEM PROMPT (first)",
            &system_prompt
        );
    });
    log_block!(
        crate::utils::log::LogLevel::Trace,
        "SYSTEM PROMPT",
        &system_prompt
    );

    log_debug!(
        "run_one_turn start | turn={} | messages={}",
        state.turn_count,
        state.messages.len()
    );

    for (idx, msg) in state.messages.iter().enumerate() {
        let preview: String = msg
            .parts
            .iter()
            .map(|p| format!("{:?}", p))
            .collect::<String>()
            .chars()
            .take(150)
            .collect();
        log_debug!("message[{}] | role={:?} | preview={}", idx, msg.role, preview);
        log_trace!(
            "message[{}] | role={:?} | parts={:?}",
            idx,
            msg.role,
            msg.parts
        );
    }

    log_trace!(
        "tools_schema | {}",
        serde_json::to_string_pretty(&tool_schema()).unwrap_or_default()
    );

    client
        .stream_message(
            &system_prompt,
            &state.messages,
            &tool_schema(),
            &mut |chunk| {
                match chunk.content {
                    ChunkContent::Text(text) => {
                        if let Some(Part::Text { text: last }) = content_blocks.last_mut() {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Text { text });
                        }
                    }
                    ChunkContent::Think(text) => {
                        if let Some(Part::Reasoning { thinking: last, .. }) =
                            content_blocks.last_mut()
                        {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Reasoning {
                                thinking: text,
                                signature: None,
                            });
                        }
                    }
                    ChunkContent::ToolUse(ref tool) => {
                        if let Part::ToolUse { id, name, arguments } = tool {
                            log_debug!(
                                "LLM tool_use | id={} | name={} | args={}",
                                id, name, arguments
                            );
                        }
                        content_blocks.push(tool.clone());
                    }
                    ChunkContent::Finish(ref reason) => {
                        log_debug!("LLM finish_reason={:?}", reason);
                        finish_reason = Some(reason.clone());
                    }
                }
            },
        )
        .await?;

    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    log_debug!(
        "assistant message appended | blocks={}",
        content_blocks.len()
    );
    for (idx, block) in content_blocks.iter().enumerate() {
        let preview = format!("{:?}", block).chars().take(200).collect::<String>();
        log_debug!("assistant block[{}] | {}", idx, preview);
        log_trace!("assistant block[{}] | {:?}", idx, block);
    }
    state.messages.push(Message::new(
        session_id.clone(),
        Role::Assistant,
        content_blocks.clone(),
    ));

    if finish_reason != Some(FinishReason::ToolUse) {
        state.transition_reason = None;
        log_debug!("run_one_turn end | no tool use");
        return Ok(false);
    }

    let tool_results = execute_tool_calls(&content_blocks);
    if tool_results.is_empty() {
        state.transition_reason = None;
        log_debug!("run_one_turn end | tool_use finish but no results");
        return Ok(false);
    }

    log_debug!(
        "pushing tool_results back to LLM | results={}",
        tool_results.len()
    );
    for (idx, tr) in tool_results.iter().enumerate() {
        log_trace!("tool_result[{}] | {:?}", idx, tr);
    }

    state
        .messages
        .push(Message::new(session_id, Role::User, tool_results));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());

    log_debug!("run_one_turn end | will continue next turn");
    Ok(true)
}

// =============================================================================
// 异步函数：代理主循环
// =============================================================================

pub async fn agent_loop<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<()> {
    while run_one_turn(client, state).await? {}
    Ok(())
}
```

- [ ] **Step 2: Run `cargo test` to verify compilation**

Run: `cargo test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/agent/agent.rs
git commit -m "feat(logging): add tiered logs to agent loop and system prompt blocks"
```

---

### Task 5: Instrument network clients with debug summaries and trace details

**Files:**
- Modify: `src/provider/client/openapi_client.rs`
- Modify: `src/provider/client/anthropic_client.rs`
- Modify: `src/provider/base_client.rs`

- [ ] **Step 1: Update `src/provider/base_client.rs` to add trace logging in `send_with_retry`**

Add `use crate::log_trace;` at the top of `src/provider/base_client.rs`.

In `send_with_retry`, inside the `loop`, add trace logs before each retry:

After the `Ok(resp)` branch where a retryable status is detected (before `eprintln!`):
```rust
log_trace!(
    "send_with_retry | attempt={} | status={} | backoff={:?}",
    attempt + 1,
    status,
    backoff
);
```

After the `Err(err)` branch where a retryable error is detected (before `eprintln!`):
```rust
log_trace!(
    "send_with_retry | attempt={} | error={} | backoff={:?}",
    attempt + 1,
    err,
    backoff
);
```

- [ ] **Step 2: Update `src/provider/client/openapi_client.rs` to split logs by level**

Replace existing `log_debug!` usage as follows:

Keep these as `log_debug!` (request summary):
```rust
log_debug!(
    "OpenAI request | url={} | model={} | messages={} | tools_count={}",
    url,
    self.model_name,
    openai_messages.len(),
    body.get("tools").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
);
```

Change the request body preview to `log_trace!` with **full body** (no char limit):
```rust
log_trace!("OpenAI request body | {}", serde_json::to_string_pretty(&body).unwrap_or_default());
```

In `parse_openai_sse`, change `log_debug!` for raw SSE data and deltas to `log_trace!`:
- `"OpenAI SSE raw | ..."` → `log_trace!`
- `"OpenAI SSE text_delta | ..."` → `log_trace!`
- `"OpenAI SSE tool_call_delta | ..."` → `log_trace!`
- `"OpenAI assembled tool_call | ..."` → `log_debug!` (keep as debug — it's a complete business event)
- `"OpenAI finish_reason={:?}"` → `log_debug!` (keep as debug)
- `"OpenAI SSE [DONE]"` → `log_debug!` (keep as debug)

- [ ] **Step 3: Update `src/provider/client/anthropic_client.rs` similarly**

Keep request summary as `log_debug!`.
Change request body preview to `log_trace!` with full body.
In `parse_anthropic_sse`, change raw SSE data and all deltas (`text_delta`, `thinking_delta`, `input_json_delta`) to `log_trace!`.
Keep assembled tool_call and finish_reason as `log_debug!`.
Keep `[DONE]` as `log_debug!`.

- [ ] **Step 4: Run `cargo test` to verify compilation**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/provider/base_client.rs src/provider/client/openapi_client.rs src/provider/client/anthropic_client.rs
git commit -m "feat(logging): split provider logs into debug summaries and trace details"
```

---

### Task 6: Instrument tools layer with debug summaries and trace IO

**Files:**
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/basic_tools.rs`

- [ ] **Step 1: Update `src/tools/mod.rs`**

Add `use crate::log_trace;` at the top.

In `execute_tool_calls`, keep existing `log_debug!` lines for tool name/args and success/error summary.
Add `log_trace!` for raw output before truncation:

Before `println!("{}", &output[..output.len().min(200)]);`, add:
```rust
log_trace!("execute_tool_call raw output | name={} | output={}", name, output);
```

Before the error branch's `eprintln!`, add:
```rust
log_trace!("execute_tool_call raw error | name={} | err={}", name, e);
```

- [ ] **Step 2: Update `src/tools/basic_tools.rs`**

Add imports:
```rust
use crate::log_debug;
use crate::log_trace;
```

Add trace logs inside each tool method:

In `run_read` (after `safe_path`):
```rust
log_trace!("run_read | path={:?} | limit={:?}", path, limit);
```

In `run_bash` (at the top):
```rust
log_trace!("run_bash | command={}", command);
```
And before returning:
```rust
log_trace!("run_bash result | len={} | preview={}", combined.len(), combined.chars().take(200).collect::<String>());
```

In `run_write` (after `safe_path`):
```rust
log_trace!("run_write | path={:?} | content_len={}", fp, content.len());
```

In `run_edit` (after `safe_path`):
```rust
log_trace!("run_edit | path={:?} | old_len={} | new_len={}", fp, old_text.len(), new_text.len());
```

In `run_web_fetch` (at the top):
```rust
log_trace!("run_web_fetch | url={}", url);
```

In `run_grep` (after `safe_path`):
```rust
log_trace!("run_grep | dir={:?} | pattern={}", dir, pattern);
```

- [ ] **Step 3: Run `cargo test` to verify compilation and existing tool tests**

Run: `cargo test tools::`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tools/mod.rs src/tools/basic_tools.rs
git commit -m "feat(logging): add trace-level IO logs to tools layer"
```

---

### Task 7: Instrument permission and session layers

**Files:**
- Modify: `src/permission/permission.rs`
- Modify: `src/session/session.rs`

- [ ] **Step 1: Update `src/permission/permission.rs`**

Add imports:
```rust
use crate::log_debug;
```

In `PermissionAction::match_action`, at the very end before returning, add:
```rust
log_debug!(
    "permission check | tool={} | action={:?} | risk={:?} | reason={}",
    tool_name, action, risk, reason
);
```

In `PermissionChecker::check`, inside each match arm, add a debug log:

For `Deny`:
```rust
log_debug!("permission denied | tool={} | reason={}", tool_name, reason);
```

For `Allow`:
```rust
log_debug!("permission allowed | tool={}", tool_name);
```

For `Ask`:
```rust
log_debug!("permission ask | tool={} | risk={:?} | reason={}", tool_name, risk, reason);
```

- [ ] **Step 2: Update `src/session/session.rs`**

Add import:
```rust
use crate::log_debug;
```

Add debug logs:

In `create_session` (after `fs::create_dir_all`):
```rust
log_debug!("session created | id={} | model={} | path={}", id, model, project_path);
```

In `save_session` (at the top):
```rust
log_debug!("session saved | id={} | messages={}", session.id, session.messages.len());
```

In `append_message` (at the top):
```rust
log_debug!("message appended | session_id={} | message_id={}", session_id, message.id);
```

In `load_session` (after successful parse, before returning):
```rust
if let Some(ref s) = session {
    log_debug!("session loaded | id={} | messages={}", s.id, s.messages.len());
}
```

- [ ] **Step 3: Run `cargo test` to verify compilation and all tests**

Run: `cargo test`
Expected: PASS (all 29+ tests)

- [ ] **Step 4: Commit**

```bash
git add src/permission/permission.rs src/session/session.rs
git commit -m "feat(logging): add debug logs to permission and session layers"
```

---

### Task 8: Add main.rs startup logs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add `log_info!` / `log_debug!` startup logs in `main`**

After `set_workspace(workspace);`, add:

```rust
    log_info!("shun-code started | mode={} | workspace={:?}",
        if args.interactive { "interactive" } else if args.command.is_some() { "command" } else if args.session.is_some() { "session" } else { "none" },
        workspace
    );
```

In `run_single_command` (at the top), add:
```rust
    log_debug!("run_single_command | query_len={}", query.len());
```

In `run_interactive` (before the loop), add:
```rust
    log_debug!("run_interactive | session_id={}", session.id);
```

- [ ] **Step 2: Run `cargo test` to verify compilation**

Run: `cargo test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(logging): add startup and mode logs in main"
```

---

### Task 9: Manual verification

**Files:** None (verification only)

- [ ] **Step 1: Verify debug build help shows `--log` flag**

Run: `cargo run -- --help`
Expected output contains:
```
  -l, --log <LEVEL>          Enable debug logging (debug|trace|info|off, default: info)
```

- [ ] **Step 2: Verify debug build with `--log debug` prints startup and key nodes**

Run: `cargo run -- --log debug -c "hello"`
Expected: stderr shows timestamped `[INFO]` and `[DEBUG]` lines, including `shun-code started`, `run_single_command`, `run_one_turn start`, and **one** `SYSTEM PROMPT (first)` block.

- [ ] **Step 3: Verify debug build with `--log trace` prints system prompt every turn and network details**

Run: `cargo run -- --log trace -c "hello"`
Expected: stderr includes `[TRACE]` lines with `SYSTEM PROMPT` block, message dumps, and provider request/response details.

- [ ] **Step 4: Verify release build removes `--log` argument**

Run: `cargo build --release`
Then: `./target/release/shun-code --log debug --help`
Expected: `error: unexpected argument '--log' found`

Also verify release build runs normally without `--log`:
Run: `./target/release/shun-code -c "hello"`
Expected: normal execution, no timestamped logs on stderr.

- [ ] **Step 5: Run full test suite one last time**

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 6: Final commit**

```bash
git commit --allow-empty -m "feat(logging): complete tiered logging system with release stripping"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** All requirements from `2026-04-16-debug-logging-design.md` are mapped to tasks.
- [x] **Placeholder scan:** No TBD, TODO, or vague steps.
- [x] **Type consistency:** Macros `log_debug!`, `log_trace!`, `log_block!` are consistently used. `LogLevel::from_str` and `set_log_level` signatures match across all tasks.
- [x] **Release safety:** `#[cfg(debug_assertions)]` is applied to CLI arg, init code, and all log macros.
