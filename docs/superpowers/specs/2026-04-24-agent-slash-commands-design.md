# Agent 指令系统设计文档

> 为 fi-code 的 REPL 和单命令模式添加 `/init` 和 `/model` 两个 slash 指令。

---

## 1. 背景与目标

### 1.1 现状

fi-code 目前通过 `rustyline` 读取用户输入，所有非空输入都会直接封装为 `Message` 发送给 LLM。用户无法在不修改配置文件的情况下：
- 动态切换当前使用的模型
- 让 LLM 自动分析项目并生成 `AGENTS.md`

### 1.2 目标

1. **`/model`**：允许用户在运行时查看和切换模型，模型列表来自配置文件中的 `models` 字段。
2. **`/init`**：触发 LLM 对当前工作目录进行项目分析，自动生成或覆盖 `AGENTS.md` 文件，并在后续对话中自动将 `AGENTS.md` 内容注入系统提示词。
3. **架构优化**：将 `main.rs` 中的业务逻辑迁移到 `entry.rs`，使 `main.rs` 仅保留入口调用。

---

## 2. 架构设计

### 2.1 模块划分

```
src/
├── main.rs          # 程序入口（仅调用 entry::run）
├── entry.rs         # 从 main.rs 迁移：交互/单命令模式、会话选择、TaskManager 调度
├── commands/
│   ├── mod.rs       # 模块声明与导出
│   └── slash.rs     # 指令解析、分发与执行
├── provider/
│   └── provider.rs  # 新增：运行时模型切换、模型列表枚举
├── agent/
│   └── prompt.rs    # 修改：支持 AGENTS.md 内容注入
```

### 2.2 新增模块说明

#### `src/commands/slash.rs`

核心 slash 指令系统，负责：
- 解析用户输入，识别以 `/` 开头的指令
- 定义 `SlashCommand` 枚举：`Model(Option<String>)`、`Init`、`Unknown(String)`
- 定义 `SlashCommandHandler`，持有 `Provider` 和 `Config` 的引用
- 执行指令并返回 `SlashCommandResult`

#### `src/entry.rs`

从 `main.rs` 迁移以下函数和逻辑：
- `run_single_command`
- `run_interactive`
- `choose_or_create_session`
- `extract_task_plan_result`
- `print_task_plan`
- `SUBAGENT_SYSTEM_PROMPT`

在 `run_interactive` 和 `run_single_command` 中，于创建 `Message` 之前插入 slash 指令拦截逻辑。

### 2.3 修改模块说明

#### `src/provider/provider.rs`

新增方法：
- `set_model(model_name: &str, config: &Config) -> Result<()>`：运行时切换模型
- `list_models(config: &Config) -> Vec<(String, String)>`：枚举所有可用模型（返回 `(model_key, display_name)`）

#### `src/agent/prompt.rs`

修改 `PromptBuilder::build()`：
- 内部调用 `crate::utils::workspace::workspace_dir()` 获取工作目录
- 检查 `<workspace>/AGENTS.md` 是否存在
- 若存在，读取内容并在系统提示词末尾追加 `# Project Context (AGENTS.md)` 段落
- 此逻辑对调用方完全透明，无需修改函数签名

---

## 3. 指令行为定义

### 3.1 `/model` — 模型选择

#### 无参数 (`/model`)

读取配置文件中的所有模型，格式化为列表输出：

```text
可用模型列表：
  [1] gpt-4o — OpenAI GPT-4o (context: 128000, output: 4096)
  [2] claude-3-7-sonnet — Anthropic Claude 3.7 Sonnet (context: 200000, output: 65536)
```

- 方括号中的数字仅为展示序号，用户仍通过 `model_key`（如 `gpt-4o`）进行切换
- 如果配置文件中没有任何模型，输出：`❌ 配置文件中未找到任何模型`

#### 带参数 (`/model <model_key>`)

1. 在 `config.provider.*.models` 中查找 `model_key`
2. **找到**：调用 `provider.set_model(model_key, config)`，输出 `✅ 已切换模型: <model_key>`
3. **未找到**：输出 `❌ 没有此模型: <model_key>`，并展示完整可用列表

#### 会话影响

- 切换模型后，当前会话的历史消息**完全保留**
- 下一次 `agent_loop` 调用时，`provider.get_client()` 会返回新模型对应的客户端

### 3.2 `/init` — 项目分析与 AGENTS.md 生成

#### 执行流程

1. 用户输入 `/init`
2. 系统输出：`🔍 正在分析项目结构，生成 AGENTS.md...`
3. 构造一次性系统提示词 + 用户消息（**不加入会话历史**）：
   ```
   System: 你是一个项目文档助手。请深入分析当前项目的结构、
   技术栈、代码风格和重要约定，生成一份 AGENTS.md 文件。
   AGENTS.md 的目标是帮助 AI 编程助手快速理解项目背景。
   你可以使用 read、grep、bash 等工具来探索代码库。

   User: 请为当前项目生成 AGENTS.md，保存路径为: <workspace>/AGENTS.md
   ```
4. 创建临时的 `LoopState`（独立对象，不与当前 session 共享）
5. 调用 `agent_loop(provider.get_client()?, &mut temp_state)`
6. LLM 自主使用工具探索项目并写入 `AGENTS.md`
7. 完成后输出：`✅ AGENTS.md 已生成: <workspace>/AGENTS.md`

#### 覆盖策略

- 如果 `<workspace>/AGENTS.md` 已存在，**直接覆盖**，不询问确认
- 利用现有 `BasicTool::write` 的实现，自动受 `safe_path` 保护（不会写出工作目录）

#### 后续对话注入

- 每次 `PromptBuilder::build()` 被调用时，检查 `<workspace>/AGENTS.md` 是否存在
- 若存在，读取内容并通过 `agents_md_content` 参数注入系统提示词
- 注入格式：
  ```markdown
  # Project Context (AGENTS.md)
  <AGENTS.md 的完整内容>
  ```

---

## 4. 数据流

### 4.1 `/model` 数据流

```
用户输入: "/model gpt-4o"
        │
        ▼
┌─────────────────┐
│ entry.rs        │  检测以 '/' 开头
│ run_interactive │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ slash::parse()  │  → SlashCommand::Model(Some("gpt-4o"))
└────────┬────────┘
         │
         ▼
┌─────────────────────────┐
│ SlashCommandHandler     │
│ ├─ 验证 model_key       │  在 config.provider.*.models 中查找
│ ├─ provider.set_model() │  更新 Provider 内部的 Model 实例
│ └─ 返回 Handled         │
└────────┬────────────────┘
         │
         ▼
    "✅ 已切换模型: gpt-4o"
         │
         ▼
┌─────────────────────────┐
│ 下一次用户查询          │
│ ├─ provider.get_client()│  返回 gpt-4o 对应的新客户端
│ └─ agent_loop()         │  保留完整历史，新模型继续对话
└─────────────────────────┘
```

### 4.2 `/init` 数据流

```
用户输入: "/init"
        │
        ▼
┌─────────────────┐
│ entry.rs        │  检测以 '/' 开头
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ slash::parse()  │  → SlashCommand::Init
└────────┬────────┘
         │
         ▼
┌──────────────────────────────┐
│ SlashCommandHandler          │
│ ├─ 输出 "🔍 分析项目中..."   │
│ ├─ 构造 init 专用提示词      │
│ ├─ 创建临时 LoopState        │  不加入 session 历史
│ ├─ provider.get_client()     │
│ └─ agent_loop(temp_state)    │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ LLM 在 agent_loop 内部       │
│ ├─ read(Cargo.toml)          │
│ ├─ read(README.md)           │
│ ├─ bash(find src -type f)    │
│ ├─ read(关键文件)            │
│ └─ write(AGENTS.md, content) │
└────────┬─────────────────────┘
         │
         ▼
    "✅ AGENTS.md 已生成"
         │
         ▼
┌──────────────────────────────┐
│ 后续 agent_loop 调用         │
│ ├─ PromptBuilder::build()    │
│ ├─ 检测 workspace/AGENTS.md  │
│ └─ 注入系统提示词            │
└──────────────────────────────┘
```

---

## 5. 错误处理

| 场景 | 行为 |
|------|------|
| `/model` 传入无效的模型名 | 输出 `❌ 没有此模型: <name>`，并展示可用列表 |
| `/model` 但配置文件中无任何模型 | 输出 `❌ 配置文件中未找到任何模型` |
| `/init` 但未设置工作目录 | 输出 `❌ 未设置工作目录` |
| `/init` 但 LLM 调用失败 | 输出 `❌ AGENTS.md 生成失败: <error>` |
| `/init` 但 write 工具被拒绝 | 输出 `❌ 写入 AGENTS.md 被拒绝，请检查权限` |
| 未知的 slash 指令 | 输出 `❌ 未知命令: <cmd>。可用命令: /init, /model` |

---

## 6. 测试策略

### 6.1 单元测试（`src/commands/slash.rs`）

- `test_parse_model_no_args` → 解析为 `Model(None)`
- `test_parse_model_with_args` → 解析为 `Model(Some("gpt-4o"))`
- `test_parse_init` → 解析为 `Init`
- `test_parse_unknown` → 解析为 `Unknown("foo")`
- `test_model_validation_valid` → 验证存在的模型名返回成功
- `test_model_validation_invalid` → 验证不存在的模型名返回失败

### 6.2 单元测试（`src/provider/provider.rs`）

- `test_set_model` — 验证模型切换后 `model_name()` 返回新值
- `test_list_models` — 验证枚举返回所有已配置的模型

### 6.3 单元测试（`src/agent/prompt.rs`）

- `test_prompt_with_agents_md` — 验证 AGENTS.md 内容被正确注入系统提示词
- `test_prompt_without_agents_md` — 验证无 AGENTS.md 时提示词不变

### 6.4 回归测试

- 现有 `agent_loop` 测试不受影响
- 现有 `main.rs` 中的 `run_interactive` / `run_single_command` 逻辑迁移到 `entry.rs` 后，行为保持一致

---

## 7. 实现顺序

1. **创建 `src/entry.rs`** — 从 `main.rs` 迁移业务逻辑，保持 `main.rs` 最小化
2. **实现 `src/commands/slash.rs`** — 指令解析与执行框架
3. **修改 `src/provider/provider.rs`** — 添加 `set_model` 和 `list_models`
4. **修改 `src/agent/prompt.rs`** — 添加 AGENTS.md 注入支持
5. **在 `entry.rs` 中集成 slash 指令拦截** — 在 `run_interactive` 和 `run_single_command` 中调用
6. **编写单元测试** — 覆盖新增模块
7. **运行 `cargo test`** — 确保所有测试通过

---

## 8. 兼容性说明

- **配置文件格式不变**：`/model` 指令读取的模型完全来自现有的 `config.json` / `config.jsonc` 结构
- **会话格式不变**：`Session` 和 `Message` 数据结构无需修改
- **向后兼容**：未使用 slash 指令的用户完全不受影响
