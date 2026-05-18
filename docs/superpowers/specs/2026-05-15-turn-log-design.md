# Turn 级完整对话日志设计

> 设计日期：2026-05-15  
> 状态：待实现

---

## 1. 背景与目标

当前 fi-code 的日志系统以**流式**方式记录（`LogEntry` 单条日志），便于实时查看和调试，但无法完整还原一次 `run_one_turn` 的全貌。本设计目标是：

- 在每次 `run_one_turn` 结束后，将本轮的完整上下文（LLM 输出、工具调用、参数、返回结果、Token 消耗、Turn 状态等）以**结构化 JSON** 形式持久化
- 提供 CLI 命令，将 JSON 日志渲染为**人类可读的格式**输出到 stdout

---

## 2. 需求摘要

| 需求 | 说明 |
|------|------|
| 日志格式 | JSON Lines（每行一个独立 JSON 对象） |
| 日志粒度 | `run_one_turn` 级别（每次 LLM 调用 + 工具执行完成后记录一次） |
| 包含字段 | content_blocks、tool_results、messages_snapshot、token_usage、wave_marker、finish_reason、transition_reason、session_id、turn_index、timestamp、error |
| 写入方式 | 异步（通过 `tokio::sync::mpsc` 发送给后台写入任务，不阻塞 Agent 循环） |
| 日志路径 | `~/.config/fi-code/logs/turns.jsonl`（与项目现有 XDG 数据目录一致） |
| CLI 命令 | `fi-code-cli logs` |

---

## 3. 数据模型

### 3.1 `TurnLogEntry`

```rust
#[derive(Debug, Clone, Serialize)]
pub struct TurnLogEntry {
    /// ISO 8601 格式时间戳
    pub timestamp: String,
    /// 当前会话 ID
    pub session_id: String,
    /// 本轮序号（从 1 开始）
    pub turn_index: usize,
    /// LLM 停止原因
    pub finish_reason: Option<String>,
    /// 本轮 Token 使用量
    pub token_usage: TokenUsage,
    /// LLM 流式输出的所有 Part（Text / ToolUse / Reasoning）
    pub content_blocks: Vec<Part>,
    /// 工具执行结果（含名称、参数、返回内容、耗时、是否错误）
    pub tool_results: Vec<ToolResultLog>,
    /// 本轮结束时的 messages 数组快照（便于回放上下文）
    pub messages_snapshot: Vec<Message>,
    /// WaveMarker 元信息（Git 快照、step、时间戳）
    pub wave_marker: Option<Part>,
    /// Agent 状态迁移原因（如 "tool_result" / "direct_output" / "max_turns"）
    pub transition_reason: Option<String>,
    /// 若本轮执行出错，记录错误信息
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResultLog {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub content: String,
    pub duration_ms: u64,
    pub is_error: bool,
}
```

### 3.2 JSON 输出示例

```json
{
  "timestamp": "2026-05-15T10:30:00+08:00",
  "session_id": "01KRNCQ9452F546E4MK1KSZPQQ",
  "turn_index": 2,
  "finish_reason": "tool_use",
  "token_usage": {"prompt_tokens": 1200, "completion_tokens": 350},
  "content_blocks": [
    {"type": "text", "text": "我来执行命令查看文件列表。"},
    {"type": "tool_use", "id": "bash_1", "name": "bash", "arguments": {"command": "ls -la"}}
  ],
  "tool_results": [
    {
      "tool_call_id": "bash_1",
      "name": "bash",
      "arguments": {"command": "ls -la"},
      "content": "total 128\ndrwxr-xr-x  5 user user  4096 ...",
      "duration_ms": 120,
      "is_error": false
    }
  ],
  "messages_snapshot": [...],
  "wave_marker": {"type": "wave_marker", "step": 2, "git_snapshot": "abc1234", "timestamp_ms": 1234567890},
  "transition_reason": "tool_result",
  "error": null
}
```

---

## 4. 写入机制

### 4.1 写入时机

在 `crates/core/src/agent/agent.rs` 的 `run_one_turn` 函数末尾（`Ok(false)` 或 `Ok(true)` 返回前）调用 `TurnLogger::log_turn(entry)`。

无论本轮成功或失败，均记录。失败时 `error` 字段填充错误信息。

### 4.2 异步写入流程

```
run_one_turn()
  │
  ├─> 构造 TurnLogEntry
  │
  ├─> TurnLogger::log_turn(entry)  // 非阻塞，try_send
  │     └─> mpsc::Sender<TurnLogEntry>
  │
  └─> 返回

后台任务 (tokio::spawn)
  │
  └─> 从 mpsc::Receiver 接收 TurnLogEntry
       └─> serde_json::to_string(entry) + "\n"
       └─> tokio::fs::write 追加到 turns.jsonl
```

### 4.3 文件路径

```rust
let logs_dir = directories::ProjectDirs::from("", "", "fi-code")
    .map(|d| d.config_dir().join("logs"))
    .unwrap_or_else(|| PathBuf::from(".config/fi-code/logs"));
let turn_log_path = logs_dir.join("turns.jsonl");
```

---

## 5. CLI 命令 `fi-code-cli logs`

### 5.1 参数

| 参数 | 短选项 | 说明 | 默认值 |
|------|--------|------|--------|
| `--limit` | `-n` | 显示最近 N 轮 | 20 |
| `--follow` | `-f` | 实时跟踪新日志（类似 `tail -f`） | false |
| `--session` | | 按 session_id 过滤（支持前缀匹配） | 无 |
| `--tool` | | 按工具名过滤 | 无 |
| `--raw` | | 输出原始 JSON 而不是格式化文本 | false |

### 5.2 人类可读输出格式

```
═══════════════════════════════════════════════════
Turn #2 | Session: 01KR... | 2026-05-15 10:30:00
Finish: tool_use | Tokens: 1200↑ 350↓
───────────────────────────────────────────────────
[LLM]
我来执行命令查看文件列表。

[Tool] bash | ✅ 120ms
参数: {"command": "ls -la"}
结果: total 128
...
═══════════════════════════════════════════════════
```

### 5.3 错误处理

- 若日志文件不存在：输出 `"日志文件不存在: ~/.config/fi-code/logs/turns.jsonl"`（stderr），退出码 1
- 若某行 JSON 解析失败：跳过该行，输出警告到 stderr，继续处理后续行
- 若 `--follow` 模式下文件被删除：优雅退出

---

## 6. 模块划分

```
crates/core/src/
  agent/
    turn_logger.rs      # TurnLogEntry 定义 + TurnLogger 单例 + 后台写入任务
  utils/
    mod.rs              # pub mod turn_log_cli; (仅 re-export)
    turn_log_cli.rs     # CLI 格式化输出逻辑（供 cli crate 调用）

crates/cli/src/
  cli_args.rs           # 新增 LogsArgs subcommand
  main.rs / entry.rs    # 路由到 logs 子命令处理函数
```

---

## 7. 测试策略

| 测试层级 | 内容 |
|----------|------|
| 单元测试 | `TurnLogEntry` 序列化/反序列化；`ToolResultLog` 构造；格式化输出函数 |
| 集成测试 | 启动 Mock Provider → 触发 Agent 运行 → 验证 `turns.jsonl` 中存在预期记录 |
| CLI 测试 | `fi-code-cli logs --limit 5` 输出格式验证；`--raw` 模式验证；空文件/损坏文件处理 |

---

## 8. 安全与边界

- **输出截断**：`tool_results` 中的 `content` 字段保留原始内容（不截断），因为日志目的是完整还原，与 LLM 上下文截断不同
- **路径安全**：通过 `ProjectDirs` 解析标准配置目录，不支持自定义路径（避免路径遍历）
- **并发安全**：异步写入使用 `tokio::fs`，单线程后台任务顺序追加，天然避免竞争
- **磁盘空间**：JSONL 文件可能快速增长，未来可考虑自动轮转（如保留最近 30 天），但不在本阶段实现

---

## 9. 与现有系统的关系

| 现有系统 | 关系 |
|----------|------|
| `LogBroadcaster` / `LogFileWriter` | 独立运行，互不干扰。流式日志继续写入 `agent.log`，回合日志写入 `turns.jsonl` |
| `SessionManager` (JSONL 会话持久化) | 独立运行。会话文件保存对话历史供恢复，turn 日志供审计/调试/分析 |
| `LogStore` (内存环形缓冲区) | 独立运行。TUI 实时日志面板不受影响 |
