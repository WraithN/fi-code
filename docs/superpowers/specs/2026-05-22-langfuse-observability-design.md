# Langfuse Observability 接入设计

> 状态：草案（待用户 review）
> 作者：fi-code contributors
> 创建时间：2026-05-22
> 关联：废弃 `crates/core/src/agent/turn_logger.rs` 与 `crates/core/src/utils/turn_log_cli.rs`

---

## 0. 背景与目标

### 0.1 现状

- `crates/core/src/agent/turn_logger.rs`（268 行）通过 `tokio::sync::mpsc` 将 `TurnLogEntry` 异步写入 `~/.config/fi-code/logs/turns.jsonl`，是当前唯一的"结构化业务追踪"通道。
- `crates/core/src/utils/turn_log_cli.rs`（168 行）支撑 `fi-code-cli logs` 子命令，读取 `turns.jsonl` 做过滤与格式化展示。
- 项目当前**无任何分布式追踪或外部可观测对接**（无 `tracing`、无 OpenTelemetry、无 Langfuse）。
- 运维日志走另一条独立路径：`log_info!` / `log_warn!` / `log_error!` 宏 → `LogFileWriter` → `~/.config/fi-code/logs/<date>.log`。

### 0.2 目标

接入 [Langfuse](https://langfuse.com/) 作为外部 LLM 可观测后端，覆盖 Agent 全链路（chat / turn / llm.generation / tool / compression），并满足以下约束：

1. **单一事件源**：不允许出现"业务代码同时调用 `TurnLogger` 和 `OTel`"的双写。废弃 `TurnLogger`。
2. **本地兜底**：可观测数据必须永远在本地存一份（即使 Langfuse 不可达），且这份本地数据就是后续重发的数据源（不另起 pending 目录）。
3. **零阻塞**：所有上报走后台批量队列，主路径不感知网络耗时。
4. **可降级**：缺少配置 / 凭证错误 / 网络不通 → 静默降级到"只写本地"。
5. **可重发**：进程启动时扫描本地文件，把上次失败的 spans 重发到 Langfuse。
6. **接入范围**：仅 `fi-code-core`；CLI / TUI / Server / Desktop 通过依赖自动继承。
7. **凭证脱敏**：上报内容中的 API Key / Bearer Token / Basic Auth / password 等敏感字段必须打码后再写入 attribute（本地与远端都受影响）。

### 0.3 非目标

- 不实现 Langfuse Prompts / Datasets / Evaluations 的对接。
- 不接入 `tracing` crate（项目历史决策）。
- 不引入 gRPC / `tonic`（Langfuse OTLP 端点仅支持 HTTP）。
- 不做日志轮转（`spans.jsonl` 不分片，磁盘管理由用户自理）。
- 不引入 WAL：进程被 `kill -9` 时队列中未 flush 的 spans 视为丢失（建议正常 SIGTERM 退出，`shutdown()` 会 drain）。

---

## 1. 架构概览

```
┌───────────────────────────────────────────────────────────────────────┐
│                         fi-code-core (Agent)                          │
│                                                                       │
│   agent_loop / run_one_turn / execute_tool_calls / compression        │
│                              │                                        │
│              (统一调用：otel::chat_span / llm_generation /            │
│                          tool_span / compression_span)                │
│                              │                                        │
│                              ▼                                        │
│       ┌──────────────────────────────────────────────────────┐        │
│       │  observability::otel (唯一可观测 facade)             │        │
│       │   • 基于 opentelemetry_sdk::trace::TracerProvider    │        │
│       │   • 凭证脱敏 / 属性映射 (gen_ai.* + langfuse.*)      │        │
│       └──────────────────────────────────────────────────────┘        │
│                              │                                        │
└──────────────────────────────┼────────────────────────────────────────┘
                               │
                               ▼  BatchSpanProcessor (后台 tokio task)
            ┌──────────────────────────────────────────────┐
            │     CompositeSpanExporter                    │
            │     fan-out 到两个下游：                     │
            │                                              │
            │  ┌─ LocalJsonlExporter  ──► ~/.config/      │
            │  │   (必须成功，是兜底)       fi-code/logs/  │
            │  │                            spans.jsonl    │
            │  │                                           │
            │  └─ OtlpHttpExporter   ──► Langfuse OTLP    │
            │      (可失败，不影响        /api/public/    │
            │       本地落盘)              otel/v1/traces  │
            └──────────────────────────────────────────────┘
                               │
                               ▼
                  启动时 ResendDaemon 扫描 spans.jsonl
                  把 lf_status=pending 行重放到 OTLP，
                  成功后追加 status_patch 行
```

### 1.1 核心设计要点

- **OTel facade**：业务代码只调 `otel::chat_span()` / `otel::llm_generation()` 等，不感知 OTLP/HTTP 协议。
- **BatchSpanProcessor**：直接复用官方 SDK 的批量队列、定时 flush、shutdown drain。
- **Composite Exporter**：自定义实现，fan-out 到 LocalJsonl + OTLP。LocalJsonl 必成功，OTLP 失败被吞掉。
- **重发 = 启动期 daemon**：扫描本地文件，找出 `lf_status="pending"` 的 spans 重发。
- **本地文件即 pending 队列**：不另起目录，append-only + status_patch 行实现状态修改。

### 1.2 与现有日志系统的关系

- `log_info!` / `log_warn!` / `log_error!` 宏继续走 `LogFileWriter` 写 `logs/<date>.log`，用途为运维排障，**不进入 OTel pipeline**。
- 废弃的 `TurnLogger` 不再被任何代码调用；`turns.jsonl` 历史文件留给用户自行处理（启动时若检测到 `turns.jsonl` 存在，打印一行 warn 提示用户备份后删除）。

---

## 2. 模块拆分

### 2.1 新增

```
crates/core/src/observability/
├── mod.rs                              对外 re-export + init/shutdown
├── tracer.rs                           全局 TracerProvider 初始化、配置加载
├── attrs.rs                            属性 helper：gen_ai.* / langfuse.* 常量与构造
├── redact.rs                           凭证脱敏：API Key / Bearer / sk-xxx / password
├── exporter/
│   ├── mod.rs                          CompositeSpanExporter
│   ├── local_jsonl.rs                  LocalJsonlExporter：OTLP→JSONL 序列化、必成功
│   └── otlp_http.rs                    薄封装 opentelemetry-otlp::HttpExporterBuilder
├── resend.rs                           启动时扫 spans.jsonl 重发 pending 行
├── facade.rs                           业务侧 ergonomic API
└── cli_view.rs                         迁移自 utils/turn_log_cli.rs，改读 spans.jsonl
```

### 2.2 删除

- `crates/core/src/agent/turn_logger.rs`（整个文件，268 行）
- `crates/core/src/utils/turn_log_cli.rs`（迁移至 `observability/cli_view.rs`，168 行）

### 2.3 改动

- `crates/core/src/agent/mod.rs`：删除 `pub use turn_logger::*;`
- `crates/core/src/agent/agent.rs`：6 处 `TurnLogger::global().log_turn(...)` 全部替换为对应的 `otel::*` 调用
- `crates/core/src/agent/runner.rs`：同上
- `crates/core/src/utils/mod.rs`：删除 `pub mod turn_log_cli;`
- `crates/cli/src/entry.rs:120`：`use fi_code_core::utils::turn_log_cli::*` 改为 `use fi_code_core::observability::cli_view::*`
- `crates/cli/src/cli_args.rs`：`logs` 子命令文档更新为 "View structured Agent traces (from spans.jsonl)"
- `crates/core/src/lib.rs`：新增 `pub mod observability;`
- `crates/core/src/server/server.rs` / `crates/cli/src/entry.rs` / `crates/tui/src/main.rs`：在启动阶段调用 `observability::init(&config)`；在 SIGINT/SIGTERM 处理或正常退出处调用 `observability::shutdown()`

### 2.4 新增依赖

`crates/core/Cargo.toml`：

```toml
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio", "trace"] }
opentelemetry-otlp = { version = "0.27", default-features = false, features = ["http-proto", "reqwest-client", "trace"] }
opentelemetry-semantic-conventions = "0.27"
base64 = "0.22"
```

**显式不引入**：`tonic`、`grpc`、`tracing`、`tracing-opentelemetry`、`opentelemetry-stdout`。

### 2.5 配置 Schema

`~/.config/fi-code/config.json` 新增 `observability` 节点：

```jsonc
{
  "model": "openai/kimi-k2.5",
  "provider": { /* 现有 */ },
  "observability": {
    "langfuse": {
      "enabled": true,
      "host": "https://cloud.langfuse.com",
      "public_key": "{env:LANGFUSE_PUBLIC_KEY}",
      "secret_key": "{env:LANGFUSE_SECRET_KEY}",
      "environment": "dev",
      "release": "0.1.0"
    }
  }
}
```

**环境变量优先级**（与现有 Provider 一致）：

| 字段 | 环境变量 |
|---|---|
| `enabled` | 若 `LANGFUSE_PUBLIC_KEY` 与 `LANGFUSE_SECRET_KEY` 都存在则视为 `true` |
| `host` | `LANGFUSE_HOST`（默认 `https://cloud.langfuse.com`）|
| `public_key` | `LANGFUSE_PUBLIC_KEY` |
| `secret_key` | `LANGFUSE_SECRET_KEY` |
| `environment` | `LANGFUSE_ENVIRONMENT` |
| `release` | `LANGFUSE_RELEASE` |

若环境变量与 config.json 同时存在，**环境变量胜出**。若 `public_key` 或 `secret_key` 缺失，整个 OTLP exporter 不装载，仅 LocalJsonl 工作。

---

## 3. 数据流

### 3.1 关键路径（一次 /chat，模型调 1 个 read 工具，2 轮）

```
[server::handle_chat_endpoint]
  │
  ├─ otel::start_chat_span(session_id, user_message, agent_type)
  │   ├─ trace_id 自动生成（一次 /chat = 一个 trace）
  │   ├─ span.name = "chat.request"
  │   ├─ attrs: langfuse.user.id="local"
  │   ├─ attrs: langfuse.session.id=<session_id>
  │   ├─ attrs: langfuse.trace.name="chat.request"
  │   ├─ attrs: langfuse.observation.input=<user_message 脱敏>
  │   └─ baggage 注入 session_id（传播给所有子 span）
  │
  ├─ [agent::agent_loop]
  │   ├─ [run_one_turn #1]  otel::start_turn_span(parent=chat_ctx, turn_index=1)
  │   │   ├─ span.name = "agent.turn"
  │   │   ├─ attrs: langfuse.observation.type="span"
  │   │   ├─ attrs: fi_code.turn.index=1
  │   │   │
  │   │   ├─ [provider.send_with_retry]
  │   │   │   └─ otel::start_llm_generation(parent=turn_ctx, model, provider, messages_json)
  │   │   │       ├─ span.name = "llm.generation"
  │   │   │       ├─ attrs: langfuse.observation.type="generation"
  │   │   │       ├─ attrs: gen_ai.system="openai" | "anthropic"
  │   │   │       ├─ attrs: gen_ai.request.model=<model>
  │   │   │       ├─ attrs: langfuse.observation.input=<messages JSON 脱敏>
  │   │   │       │
  │   │   │       │  (流式响应到达)
  │   │   │       │
  │   │   │       ├─ attrs: langfuse.observation.output=<assistant text + tool_use 脱敏>
  │   │   │       ├─ attrs: gen_ai.usage.input_tokens, output_tokens, total_tokens
  │   │   │       ├─ attrs: langfuse.observation.usage_details={"input":..,"output":..}
  │   │   │       ├─ attrs: gen_ai.response.finish_reasons=["tool_use"]
  │   │   │       └─ span.end()
  │   │   │
  │   │   ├─ [execute_tool_calls]
  │   │   │   └─ otel::start_tool_span(parent=turn_ctx, "read", tool_call_id, args_json)
  │   │   │       ├─ span.name = "tool.read"
  │   │   │       ├─ attrs: fi_code.tool.name="read"
  │   │   │       ├─ attrs: fi_code.tool.call_id=<id>
  │   │   │       ├─ attrs: langfuse.observation.input=<args JSON 脱敏>
  │   │   │       │
  │   │   │       │  (工具执行)
  │   │   │       │
  │   │   │       ├─ attrs: langfuse.observation.output=<tool_result 截断 50KB 脱敏>
  │   │   │       ├─ attrs: langfuse.observation.level="DEFAULT" | "ERROR"
  │   │   │       └─ span.end()
  │   │   │
  │   │   └─ turn span.end()
  │   │
  │   └─ [run_one_turn #2]  (LLM 把 read 结果总结成文本，无更多工具)
  │       └─ start_llm_generation → ... → end
  │
  ├─ chat_span.set_output(&final_text)
  └─ chat_span.end()
       │
       │  (业务路径到此结束。下面是 OTel SDK 后台行为)
       ▼
  [BatchSpanProcessor 后台 task]
       │
       ├─ 队列累积 ≥ 512 spans 或距离上次 flush ≥ 5s
       │
       └─ CompositeSpanExporter.export(spans)
            ├─ LocalJsonlExporter.export(spans)
            │   ├─ 每个 SpanData 序列化为一行 JSON（OTLP 标准 schema + fi_code 扩展字段）
            │   ├─ 每行尾加 fi_code 字段：{"lf_status":"pending"}
            │   ├─ append 到 spans.jsonl（O_APPEND，单进程内 Mutex 保证不交错）
            │   └─ 必成功；失败 → log_error! 但不冒泡到主路径
            │
            └─ OtlpHttpExporter.export(spans)
                ├─ 序列化 OTLP protobuf
                ├─ POST https://<host>/api/public/otel/v1/traces
                │   Authorization: Basic base64(pk:sk)
                │   x-langfuse-ingestion-version: 4
                ├─ 成功 → Composite 调 LocalJsonl.append_status_patch(span_ids, "sent")
                ├─ 失败 → log_warn! + 保留 pending 状态
                └─ Composite 整体始终返回 Ok（OTLP 失败不影响业务）
```

### 3.2 启动期

```
fi-code 进程启动
  │
  ├─ observability::init(&config)
  │   ├─ 解析 config + env，构造 ObservabilityConfig
  │   ├─ 创建 LocalJsonlExporter
  │   ├─ 若 langfuse keys 完整 → 创建 OtlpHttpExporter
  │   ├─ 组装 CompositeSpanExporter
  │   ├─ 创建 BatchSpanProcessor 包裹 Composite
  │   ├─ 创建 TracerProvider，注册到 opentelemetry::global
  │   │
  │   └─ tokio::spawn(resend_daemon())
  │        ├─ 若 spans.jsonl 不存在 → 立即退出
  │        ├─ 读取末尾 10000 行
  │        ├─ 倒序聚合 status_patch → 重建 span_id → lf_status 映射
  │        ├─ 收集 lf_status="pending" 且 timestamp 距今 < 7 天 的 SpanData
  │        ├─ 按 trace_id 分组（避免 trace 不完整）
  │        ├─ 逐 trace 调 OtlpHttpExporter.export
  │        └─ 成功 → 通过 LocalJsonlExporter.append_status_patch 追加 sent
  │
  └─ 进入 server.run() / cli::repl() / tui::run()
```

### 3.3 进程退出期

```
SIGINT/SIGTERM 或正常退出
  │
  └─ observability::shutdown()
      └─ tracer_provider.shutdown()
          └─ BatchSpanProcessor flush 队列剩余 spans
              ├─ LocalJsonl 全部写盘成功（同步等待）
              └─ OTLP 尽力上传，失败由下次启动的 daemon 补
```

### 3.4 spans.jsonl 行格式

**Span 行**（写盘时 `lf_status` 永远为 `"pending"`；后续由 status_patch 行覆盖）：

```json
{
  "trace_id": "0123456789abcdef0123456789abcdef",
  "span_id": "0123456789abcdef",
  "parent_span_id": "fedcba9876543210",
  "name": "llm.generation",
  "kind": "INTERNAL",
  "start_time_unix_nano": 1747838400000000000,
  "end_time_unix_nano": 1747838403500000000,
  "status": {"code": "OK", "message": ""},
  "attributes": {
    "langfuse.observation.type": "generation",
    "gen_ai.system": "openai",
    "gen_ai.request.model": "kimi-k2.5",
    "gen_ai.usage.input_tokens": 1024,
    "gen_ai.usage.output_tokens": 256,
    "langfuse.observation.input": "[{\"role\":\"user\",...}]",
    "langfuse.observation.output": "..."
  },
  "events": [],
  "resource": {
    "service.name": "fi-code",
    "service.version": "0.1.0",
    "deployment.environment": "dev"
  },
  "lf_status": "pending"
}
```

**Status patch 行**：

```json
{
  "type": "status",
  "span_ids": ["0123456789abcdef", "fedcba9876543210"],
  "lf_status": "sent",
  "patched_at_unix_nano": 1747838405000000000
}
```

读取时：先扫一遍构建 `span_id → 最新 lf_status` 映射，再线性遍历过滤。

---

## 4. 错误处理与边界

### 4.1 启动期失败矩阵

| 场景 | 行为 | 用户感知 |
|---|---|---|
| 配置 `enabled=false` 或缺少 keys | `init()` 跳过 OTLP exporter，只装 LocalJsonl | 无；本地 `spans.jsonl` 照常写 |
| 配置 `enabled=true` 但 keys 缺失 | `init()` 内部 `log_warn!` 后自行降级为只装 LocalJsonl，**返回 Ok** | 启动日志一行 warn |
| `host` URL 不合法 | 同上，init 内部降级 disabled，返回 Ok | warn 一行 |
| `~/.config/fi-code/logs/` 不可写 | `init()` 返回 `Err`，调用方（main）panic | 进程退出（与 `LogFileWriter` 一致）|
| Langfuse host 不可达（DNS/TLS 失败）| init 不主动探活，等运行期第一次 export 失败再处理 | 无 |
| 检测到旧 `turns.jsonl` | 打印一行 warn 提示用户备份后删除，不阻塞启动 | warn 一行 |

### 4.2 运行期失败矩阵

| 场景 | LocalJsonl | OTLP | Composite 总行为 |
|---|---|---|---|
| 磁盘满 / 文件权限丢失 | 写失败，`log_error!` | 不影响 | 返回 Err（OTel SDK 重试一次仍失败则丢弃）|
| Langfuse 4xx（401/403 凭证错）| 成功写盘 pending | 失败 `log_warn!`，不重试 | Ok |
| Langfuse 5xx / 网络超时 | 成功写盘 pending | 失败 `log_warn!`，不重试（交给启动期 daemon）| Ok |
| OTLP 成功但 status_patch 写盘失败 | 已写过原始行 | 已成功 | 退化为下次启动重发 → Langfuse 按 span_id 去重（OTLP 接收幂等）|
| 进程崩溃前队列还有 spans | 来不及 flush | 同左 | 下次启动无法补救（数据已丢）。缓解：`shutdown()` 在 SIGTERM 时同步 drain |
| span attribute 超 50KB | 写盘正常 | 上传正常 | 主动截 50KB（与现有工具输出截断一致）|

### 4.3 重发 daemon 边界

| 场景 | 处理 |
|---|---|
| `spans.jsonl` 不存在 | 直接退出（首次启动）|
| 文件超过 100MB | 只读末尾 10000 行（按平均 1KB/span 估算）|
| status_patch 行 JSON 损坏 | 跳过该行，继续 |
| pending span 超过 7 天 | 视为放弃，不再重发 |
| 重发批次本身又失败 | `log_warn!` 后退出 daemon；下次进程启动再试 |
| 文件正在被主程序追加写 | daemon 只读 O_RDONLY；status_patch 由主流程 exporter 写入，避免并发冲突 |

### 4.4 凭证脱敏正则

应用于**所有进入 attribute 的字符串**（input / output / messages / tool args / tool result）。模式：

```
sk-[A-Za-z0-9_\-]{20,}                               → sk-***REDACTED***
pk-lf-[A-Za-z0-9_\-]{20,}                            → pk-lf-***REDACTED***
sk-lf-[A-Za-z0-9_\-]{20,}                            → sk-lf-***REDACTED***
ANTHROPIC_API_KEY\s*[:=]\s*\S+                        → ANTHROPIC_API_KEY=***REDACTED***
OPENAI_API_KEY\s*[:=]\s*\S+                           → OPENAI_API_KEY=***REDACTED***
Bearer\s+[A-Za-z0-9._\-]{20,}                         → Bearer ***REDACTED***
Authorization:\s*Basic\s+[A-Za-z0-9+/=]{20,}           → Authorization: Basic ***REDACTED***
password["']?\s*[:=]\s*["']?\S+                       → password=***REDACTED***
```

**边界规则**：

- 脱敏只对 UTF-8 字符串生效。
- 一次 attribute set 流程：先截断到 50KB → 再脱敏。这样保证 token 不会被截断到一半导致正则失配。
- 正则在 `redact.rs` 中以 `once_cell::sync::Lazy<Vec<(Regex, &str)>>` 编译一次复用。

### 4.5 性能/容量边界

| 项 | 默认值 | 是否可配 |
|---|---|---|
| BatchSpanProcessor 队列容量 | 2048 spans | 否 |
| 单批次大小 | 512 spans | 否 |
| flush 间隔 | 5000ms | 否 |
| 单 attribute 最大字节 | 50KB | 否 |
| `spans.jsonl` 单文件最大 | 不滚动 | —— |
| OTLP HTTP 请求超时 | 10s | 否 |

队列满时 OTel SDK 默认行为：**丢弃新 span 并打印警告**（不阻塞业务）。

### 4.6 与现有系统的边界

- `log_info!` / `log_warn!` / `log_error!` 不进入 OTel pipeline。
- 权限 ASK 事件不单独建 span，而是在父 `tool_span` 上以 `span.add_event("permission_ask", attrs)` 记录。
- `compression` 触发时新建 `compression_span` 作为 `chat_span` 的子 span。

---

## 5. Facade API

### 5.1 Guard 类型

所有 guard struct 实现 `Drop`，在离开作用域时自动调用 `inner.end()`。业务代码无需手动 end，但可调用 `.finish()` 显式提前结束。

```rust
pub struct ChatSpan { inner: BoxedSpan, cx: Context }
pub struct TurnSpan { inner: BoxedSpan, cx: Context }
pub struct LlmGeneration { inner: BoxedSpan }
pub struct ToolSpan { inner: BoxedSpan }
pub struct CompressionSpan { inner: BoxedSpan }
```

### 5.2 初始化 / 关闭

```rust
/// 进程启动时调用。失败时降级 disabled，不 panic（仅 logs 目录不可写才 panic）。
pub fn init(config: &Config) -> anyhow::Result<()>;

/// 进程退出时调用。drain 队列 + flush 到两个 exporter。
pub fn shutdown();

/// 是否已启用（用于条件性跳过昂贵的 attribute 构造）。
pub fn is_enabled() -> bool;
```

### 5.3 业务侧调用接口

```rust
// ── Chat 级 ──

pub fn start_chat_span(
    session_id: &str,
    user_message: &str,
    agent_type: AgentType,
) -> ChatSpan;

impl ChatSpan {
    pub fn set_output(&self, text: &str);
    pub fn set_tags(&self, tags: &[&str]);
    pub fn record_error(&self, message: &str);
    pub fn trace_id(&self) -> String;
    pub fn context(&self) -> Context;
}

// ── Turn 级 ──

pub fn start_turn_span(
    parent: Option<&Context>,
    turn_index: usize,
) -> TurnSpan;

impl TurnSpan {
    pub fn set_transition_reason(&self, reason: &str);
    pub fn context(&self) -> Context;
}

// ── LLM Generation ──

pub fn start_llm_generation(
    parent: Option<&Context>,
    model: &str,
    provider: &str,           // "openai" | "anthropic"
    messages_json: &str,      // 脱敏后写入 langfuse.observation.input
) -> LlmGeneration;

impl LlmGeneration {
    pub fn record_output(&self, completion: &str);
    pub fn record_usage(&self, input_tokens: u32, output_tokens: u32, total_tokens: u32);
    pub fn record_finish_reason(&self, reason: &str);
}

// ── Tool ──

pub fn start_tool_span(
    parent: Option<&Context>,
    tool_name: &str,
    tool_call_id: &str,
    args_json: &str,
) -> ToolSpan;

impl ToolSpan {
    pub fn record_result(&self, output: &str, is_error: bool);
    pub fn add_permission_event(&self, action: &str, approved: bool, duration_ms: u64);
}

// ── Compression ──

pub fn start_compression_span(
    parent: Option<&Context>,
) -> CompressionSpan;

impl CompressionSpan {
    pub fn record_ratio(&self, before_tokens: u32, after_tokens: u32);
}
```

### 5.4 Context 传播规则

- **不使用 thread-local / 全局变量**（tokio 异步下不可靠）。
- 所有子 span 创建函数接受 `parent: Option<&Context>`。`None` 时使用 `Context::current()`（在 root span 创建时即为空 context，子调用若有 parent 必须显式传）。
- `ChatSpan::context()` / `TurnSpan::context()` 返回当前 span 关联的 `Context`，可直接传递给下游函数参数。
- 旧调用点可暂时全传 `None`，渐进迁移；这种情况下 spans 仍能写入，但不会形成正确的 parent/child 关系树。

### 5.5 agent.rs 改造对照表

| 原代码（伪代码） | 新代码 |
|---|---|
| `TurnLogger::global().log_turn(TurnLogEntry { finish_reason, token_usage, .. })` | `gen.record_finish_reason(reason); gen.record_usage(in, out, total);` |
| `TurnLogger::global().log_turn(TurnLogEntry { error: Some(e), .. })` | `chat_span.record_error(&e);` |
| `TurnLogger::global().log_turn(TurnLogEntry { messages_snapshot, .. })` | `chat_span.set_attribute("fi_code.messages_snapshot", json)` |
| `TurnLogger::global().log_turn(TurnLogEntry { tool_results, .. })` | 删除（每个工具调用已由独立 `ToolSpan` 记录）|

`messages_snapshot` 默认上报（无开关），用户依赖这份数据做离线分析。

### 5.6 禁用时的零开销

- `is_enabled()` 返回 `false` 时，业务代码可主动跳过 attribute 构造（如序列化 `messages_snapshot`）。
- guard struct 仍创建，但 OTel SDK 在 `NoopTracerProvider` 下所有操作 no-op，无堆分配、无 mpsc 发送。
- 建议在大型 attribute 构造前主动判断：

```rust
if otel::is_enabled() {
    let json = serde_json::to_string(&messages_snapshot)?;
    chat_span.set_attribute("fi_code.messages_snapshot", json);
}
```

---

## 6. 测试策略

### 6.1 单元测试

| 模块 | 测试用例 | 验证点 |
|---|---|---|
| `redact.rs` | `test_redact_openai_key` | `sk-test1234567890abcdef1234` → `sk-***REDACTED***` |
| `redact.rs` | `test_redact_anthropic_key` | `sk-ant-xxx...` 模式 |
| `redact.rs` | `test_redact_langfuse_keys` | `pk-lf-*` / `sk-lf-*` |
| `redact.rs` | `test_redact_bearer` | `Bearer xxx` |
| `redact.rs` | `test_redact_basic_auth` | `Authorization: Basic xxx` |
| `redact.rs` | `test_redact_password` | `password=xxx` / `password: xxx` |
| `redact.rs` | `test_no_false_positive_on_plain_text` | 普通文本不误伤 |
| `redact.rs` | `test_redact_within_50kb_truncation` | 先截 50KB 再脱敏，token 完整性 |
| `attrs.rs` | `test_gen_ai_attribute_keys` | 与 Langfuse 文档 attribute key 一致 |
| `attrs.rs` | `test_langfuse_attribute_keys` | 同上 |
| `exporter/local_jsonl.rs` | `test_export_writes_jsonl_with_pending_status` | 每条 span 写一行，含 `lf_status="pending"` |
| `exporter/local_jsonl.rs` | `test_export_appends_status_patch_after_otlp_success` | status_patch 行格式正确 |
| `exporter/local_jsonl.rs` | `test_export_handles_disk_full` | 写失败返回 Err 而不 panic |
| `exporter/otlp_http.rs` | `test_otlp_export_basic_auth_header` | `Authorization: Basic base64(pk:sk)` (wiremock) |
| `exporter/otlp_http.rs` | `test_otlp_export_4xx_returns_err` | 401/403 不重试 |
| `exporter/otlp_http.rs` | `test_otlp_export_5xx_returns_err` | 5xx 不重试 |
| `exporter/mod.rs` | `test_composite_local_must_succeed_otlp_may_fail` | OTLP Err 时 Composite Ok |
| `resend.rs` | `test_resend_parses_pending_and_skips_sent` | status_patch 聚合正确 |
| `resend.rs` | `test_resend_skips_spans_older_than_7_days` | 7 天过期 |
| `resend.rs` | `test_resend_handles_corrupted_jsonl_line` | 单行损坏不影响其余 |
| `resend.rs` | `test_resend_only_reads_tail_10000_lines` | 大文件只看末尾 |
| `tracer.rs` | `test_init_disabled_when_keys_missing` | 缺 keys 静默 disabled |
| `tracer.rs` | `test_init_fails_if_logs_dir_unwritable` | logs 不可写 panic |
| `facade.rs` | `test_chat_span_context_propagates_to_children` | trace_id 一致 |
| `facade.rs` | `test_noop_when_disabled` | disabled 路径零分配 |

### 6.2 集成测试（`tests/e2e-web/python/test_web_observability.py`）

复用现有 `fi_code_server` fixture，通过环境变量注入 Langfuse mock 端点：

| 用例 | 验证点 |
|---|---|
| `test_spans_jsonl_created_after_chat` | 真实 /chat 后 spans.jsonl 出现 chat_span + llm_generation |
| `test_spans_have_trace_hierarchy` | 同一 /chat 所有 span trace_id 一致，parent_span_id 链接正确 |
| `test_tool_call_produces_tool_span` | 触发 read 工具后出现 `tool.read` span |
| `test_credentials_redacted_in_spans` | prompt 含 `sk-test-1234...` → spans.jsonl 中为 `sk-***REDACTED***` |
| `test_status_patch_appended_on_otlp_success` | mock 返回 200 → spans.jsonl 末尾出现 status_patch sent 行 |
| `test_local_logs_survive_otlp_failure` | mock 返回 500 → spans.jsonl 完整写入，lf_status 保持 pending |
| `test_resend_daemon_replays_pending_on_restart` | 第一次 mock 500 → 第二次启动 mock 200 → 收到第一次 spans |

mock Langfuse 端点用 `aiohttp` 起 mini server 监听 `/api/public/otel/v1/traces`，记录请求体并按场景返回 200/4xx/5xx。

### 6.3 BDD 测试（`tests/bdd/features/observability.feature`）

```gherkin
Feature: OpenTelemetry observability

  Scenario: 用户一次 /chat 产生完整 trace
    Given fi-code-server 已启动且 Langfuse 已配置
    When 用户通过 /chat 发送 "1+1=?"
    Then spans.jsonl 至少有一个 chat.request span
    And 该 trace 至少包含一个 llm.generation span
    And chat span 的 langfuse.observation.output 不为空

  Scenario: Langfuse 不可达时本地日志保留
    Given fi-code-server 已启动但 Langfuse 配置为 http://127.0.0.1:1
    When 用户通过 /chat 发送 "1+1=?"
    Then spans.jsonl 写入了完整 spans
    And 所有 spans 的 lf_status 为 "pending"
```

### 6.4 不测的部分

- **Langfuse 真实云端**：CI 不连真实云（无凭证、有费用），只在本地手动验证一次
- **OTel SDK 内部 retry**：信任官方 SDK
- **opentelemetry-otlp protobuf 序列化**：信任官方 crate

### 6.5 验收清单

- [ ] 全部新增单元测试通过（`cargo test -p fi-code-core observability::`）
- [ ] `tests/e2e-web/python/test_web_observability.py` 全部通过（mock Langfuse）
- [ ] `cargo test --test bdd` 新增 scenario 通过
- [ ] 手动连真实 Langfuse Cloud：一次 /chat 在 UI 上能看到完整 trace 树（chat → turn → llm.generation + tool.read）
- [ ] 手动验证 disabled 路径：删除 config 中 langfuse 节点 + 不设环境变量，spans.jsonl 仍正常写、OTLP 不发送
- [ ] `cargo clippy` 无新增警告
- [ ] 编译时间增量参考值 ≤ 30s（从干净 target 测量；机器差异大，仅作参考非硬性验收）
- [ ] 旧 `turns.jsonl` 检测 warn 正确触发

---

## 7. 影响面与回滚

### 7.1 影响面

- **新增**：`crates/core/src/observability/` 整个模块（约 1000 行）+ 5 个新依赖
- **删除**：`turn_logger.rs`（268 行）+ `turn_log_cli.rs`（168 行）
- **改动**：`agent.rs` / `runner.rs` / `cli/entry.rs` 等 ~6 个文件，约 50 处调用点替换
- **配置**：用户 `~/.config/fi-code/config.json` 可选新增 `observability` 节点
- **数据**：用户 `~/.config/fi-code/logs/turns.jsonl` 不再被写入；新增 `spans.jsonl`

### 7.2 回滚策略

如发现严重问题，可通过两级回滚：

1. **配置级回滚**：用户在 config 中设置 `observability.langfuse.enabled=false` 或移除环境变量。此时仅写本地 spans.jsonl，无任何网络请求。
2. **代码级回滚**：`git revert` 整个 feature commit。由于 `TurnLogger` 已删除，回滚后用户的 `~/.config/fi-code/logs/turns.jsonl` 会被重新创建（spans.jsonl 保留作为历史归档，不再更新）。

---

## 8. 待实现清单（移交 writing-plans）

1. 新增 `crates/core/src/observability/` 模块全部文件
2. 在 `Cargo.toml` 中增加 5 个依赖
3. 替换 `agent.rs` / `runner.rs` 中所有 `TurnLogger::global().log_turn(...)` 调用
4. 删除 `turn_logger.rs` 与 `turn_log_cli.rs`
5. 更新 `cli/entry.rs` 与 `cli_args.rs` 中 `logs` 子命令实现
6. 在 `server/cli/tui` 入口处加 `observability::init` / `shutdown` 调用
7. 编写全部新增单元测试
8. 编写 `tests/e2e-web/python/test_web_observability.py`
9. 编写 `tests/bdd/features/observability.feature` + steps
10. 在 `AGENTS.md` 中追加章节描述可观测体系
11. 在 `docs/refactor/refactor-2026-05-22.md` 中记录本次重构（按 AGENTS.md 第 8 节规范）
