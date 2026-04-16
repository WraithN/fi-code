# 分层调试日志系统设计文档

> 日期：2026-04-16  
> 主题：为 `shun-code` 引入 `debug` / `trace` 两级日志体系，并在 release 构建中编译期移除日志，防止提示词与敏感信息泄露。

---

## 1. 背景与目标

当前项目中已散布少量 `log_debug!` 宏调用，但存在以下问题：

1. **无分层**：只有一个布尔开关，无法区分"业务关键节点"与"底层网络/工具细节"。
2. **格式不统一**：日志输出风格各异，缺少时间戳和模块前缀，不利于快速定位问题。
3. **安全顾虑**：若将 `shun-code` 打包为正式版发布，现有的 `--log` 选项可能泄露 system prompt、消息内容等敏感信息。

### 目标

- 提供 `debug` 和 `trace` 两个日志层级，输出到 `stderr`，格式统一、易读。
- `debug` 打印各关键节点的摘要信息；`trace` 打印底层网络流与工具调用的完整细节。
- **首次**运行时在 `debug` 层级打印完整 system prompt；后续每轮在 `trace` 层级打印完整 system prompt。
- 在 **release 构建**中通过编译期条件移除 `--log` 参数和所有日志代码，实现零运行时开销，并彻底杜绝信息泄露。

---

## 2. 设计概览

### 2.1 日志级别

```rust
pub enum LogLevel {
    Off = 0,
    Info = 1,
    Debug = 2,
    Trace = 3,
}
```

- **Info**：常规运行信息（未来可能用于非调试状态的用户提示）。
- **Debug**：业务流关键节点（启动、每轮对话起止、工具执行摘要、权限检查、会话操作）。
- **Trace**：底层细节（完整请求体、SSE 原始数据行、每个 delta、工具原始 IO）。

### 2.2 统一日志格式

所有日志统一输出到 `stderr`：

```
2026-04-16 15:32:01 [DEBUG] [agent::agent] run_one_turn start | turn=1 | messages=3
2026-04-16 15:32:01 [TRACE] [provider::openapi_client] SSE raw | data={"id":"chatcmpl-xxx", ...}
```

对于大段内容（如 system prompt），使用 `log_block!` 宏包裹，输出视觉分隔：

```
2026-04-16 15:32:01 [DEBUG] [agent::prompt] ========== SYSTEM PROMPT (first) ==========
You are an autonomous coding assistant...
...完整提示词...
2026-04-16 15:32:01 [DEBUG] [agent::prompt] ===========================================
```

### 2.3 编译时安全裁剪（release 构建）

利用 `cfg(debug_assertions)`：

- **Debug 构建**：`Args` 保留 `-l, --log <LEVEL>` 参数，日志系统完整可用。
- **Release 构建**：
  - `Args` 中的 `--log` 字段通过 `#[cfg(debug_assertions)]` 完全移除；`main` 中的日志初始化代码也加相同条件。
  - 所有日志宏在 `not(debug_assertions)` 下展开为空代码块（零运行时开销）。
  - 用户在 release 版传入 `--log` 时，`clap` 会直接报错 `unexpected argument`，彻底防止信息泄露。

---

## 3. 各层级打印内容明细

### 3.1 Debug 层级

| 模块 | 打印内容 |
|------|---------|
| `main` | 程序启动；工作目录；运行模式（interactive / command / session） |
| `agent::agent` | 每轮 `run_one_turn` 开始/结束；turn 数；消息数；finish reason；**首次**打印完整 `system_prompt`（`log_block!`） |
| `tools::mod` | 每次工具执行：名称、参数摘要、结果长度或错误信息 |
| `permission::permission` | 每次权限检查：结果（Allow / Ask / Deny）、风险等级、原因 |
| `session::session` | 会话创建、加载、保存的摘要（session_id、model、message_count） |

### 3.2 Trace 层级

| 模块 | 打印内容 |
|------|---------|
| `agent::agent` | **每轮**完整 `system_prompt`；每条 `Message` 的完整内容；每个 `Part` 块详情 |
| `provider::openapi_client` | 完整请求体（含消息和 tools）；每次 SSE 原始数据行；每个 `text_delta`、`tool_call_delta`；拼装后的完整 tool_call；finish_reason |
| `provider::anthropic_client` | 完整请求体；每次 SSE 原始数据行；`text_delta`、`thinking_delta`、`input_json_delta`；finish_reason |
| `provider::base_client` | `send_with_retry` 每次尝试的状态码/错误、退避延迟时间 |
| `tools::mod` | 每个工具调用的原始输入 JSON、原始输出内容（截断前） |
| `tools::basic_tools` | `bash` 执行的原始命令和输出；`read` 的文件路径和行数；`write/edit/grep/web_fetch` 的入参和结果 |

---

## 4. 关键实现细节

### 4.1 日志宏定义

在 `src/utils/log.rs` 中定义：

```rust
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { ... };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => { ... };
}

#[macro_export]
macro_rules! log_block {
    ($level:expr, $title:expr, $content:expr) => { ... };
}
```

在 `not(debug_assertions)` 下，所有宏展开为空块。

### 4.2 system prompt 的首次打印逻辑

在 `agent::agent::run_one_turn` 中引入一个 `static std::sync::Once`：

```rust
static LOGGED_PROMPT_ONCE: std::sync::Once = std::sync::Once::new();

// 在 run_one_turn 内部
LOGGED_PROMPT_ONCE.call_once(|| {
    log_block!(LogLevel::Debug, "SYSTEM PROMPT (first)", system_prompt);
});

// trace 层级始终打印
log_block!(LogLevel::Trace, "SYSTEM PROMPT", system_prompt);
```

### 4.3 CLI 参数的条件编译

`src/utils/cli.rs`：

```rust
#[cfg(debug_assertions)]
#[arg(short = 'l', long = "log", value_name = "LEVEL", default_value = "info")]
pub log_level: String,
```

`src/main.rs`：

```rust
#[cfg(debug_assertions)]
{
    set_log_level(&args.log_level);
}
```

---

## 5. 影响范围

以下文件需要修改：

- `src/utils/log.rs`：重构为层级日志系统
- `src/utils/cli.rs`：`--log` 字段加 `#[cfg(debug_assertions)]`
- `src/main.rs`：日志初始化加编译条件；补充启动信息打印
- `src/agent/agent.rs`：细化 `debug`/`trace` 打印；增加 system prompt 首次打印逻辑
- `src/agent/prompt.rs`：无需大改
- `src/provider/client/openapi_client.rs`：按层级重新分配日志
- `src/provider/client/anthropic_client.rs`：按层级重新分配日志
- `src/provider/base_client.rs`：`send_with_retry` 增加 `trace` 日志
- `src/tools/mod.rs`：工具调用拆分为 `debug`（摘要）和 `trace`（原始 IO）
- `src/tools/basic_tools.rs`：各工具底层实现增加 `trace` 日志
- `src/permission/permission.rs`：增加权限检查结果的 `debug` 日志
- `src/session/session.rs`：增加会话操作的 `debug` 日志

---

## 6. 测试策略

1. **单元测试**：
   - 在 `src/utils/log.rs` 的 `#[cfg(test)]` 中测试 `LogLevel` 解析和宏的条件编译行为。
2. **手动验证**：
   - `cargo run -- --log debug -c "hello"`：验证关键节点信息正确输出，system prompt 只打印一次。
   - `cargo run -- --log trace -c "hello"`：验证每轮都打印 system prompt，且网络和工具细节完整。
   - `cargo build --release && ./target/debug/shun-code --log debug`：验证 `--log` 参数已被移除并报错。
3. **回归测试**：
   - 运行 `cargo test`，确保现有 29 个测试全部通过。

---

## 7. 风险与回滚

- **风险**：如果编译条件使用不当，可能导致 release 构建失败或 debug 构建日志缺失。
- **缓解**：所有 `#[cfg(debug_assertions)]` 都集中在 `cli.rs`、`main.rs` 和 `log.rs` 三个文件中，修改边界清晰。
- **回滚**：若出现问题，可直接回滚 `log.rs` 到旧版本，并移除 `main.rs` 中的编译条件即可恢复原有行为。
