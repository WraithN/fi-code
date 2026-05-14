# 重构记录：系统提示词模板化

**处理时间**：2026-05-14 21:00
**模块**：`crates/core/src/agent/prompt.rs`
**相关 Commit**：(待填充)

---

## 重构动机

原来的 `PromptBuilder` 将系统提示词的所有内容（Identity、Core Rules、Git Status Awareness、Skills、AGENTS.md、.rules/）硬编码在 Rust 代码中：

- 每段提示词都是 `String::from("...")` 形式的硬编码字符串
- 静态内容和动态内容混杂在一起，难以阅读和修改
- 非 Rust 开发者（如产品经理、提示词工程师）无法直接编辑提示词
- 提示词文本的修改需要重新编译整个项目

---

## 具体改动

### 1. 新增模板文件 `prompt_template.md`

将系统提示词的静态内容提取到独立的 Markdown 文件中：

```markdown
# System Prompt for FiCode

## 1. Identity
...

## 2. Core Rules
...

## 3. Git Status Awareness
...

---
...
---

{{SKILLS}}

{{AGENTS_MD}}

{{RULES}}
```

动态部分使用占位符表示：
- `{{SKILLS}}` — 可用 Skills 列表
- `{{AGENTS_MD}}` — 项目 AGENTS.md 内容
- `{{RULES}}` — .rules/ 目录下的 .md 文件

### 2. 使用 `include_str!` 编译时嵌入模板

```rust
const PROMPT_TEMPLATE: &str = include_str!("prompt_template.md");
```

模板文件在编译时嵌入二进制，无需运行时文件 I/O，也不依赖文件系统路径。

### 3. 重构 `PromptBuilder::build`

原逻辑：按固定顺序调用 `build_identity()`、`build_core_rules()` 等 6 个方法，手动拼接。

新逻辑：
1. 读取模板
2. 调用 `build_skills()` 生成动态内容
3. 调用 `replace_placeholder("{{SKILLS}}", content)` 替换占位符
4. 从缓存读取 AGENTS.md 和 .rules/ 内容
5. 替换 `{{AGENTS_MD}}` 和 `{{RULES}}`
6. 如果某部分为空，移除占位符并清理多余空行

### 4. 删除的代码

- `build_identity()` — 硬编码的 Identity 文本
- `build_core_rules()` — 硬编码的 Core Rules 文本
- `build_git_status()` — 硬编码的 Git Status Awareness 文本

这些静态内容全部迁移到 `prompt_template.md` 中。

### 5. 保留的代码

- `build_skills()` — Skills 列表生成逻辑（依赖 `SkillRegistry`）
- `build_agents_md_inner()` — AGENTS.md 文件读取逻辑
- `build_rules_dir_inner()` — .rules/ 目录扫描逻辑
- `PROJECT_CONTEXT_CACHE` — 项目级上下文缓存

---

## 预期收益

1. **提示词可独立编辑**：非开发者可以直接修改 `prompt_template.md` 来调整系统提示词
2. **职责分离**：静态文案在 `.md` 文件中，动态逻辑在 `.rs` 代码中
3. **可读性提升**：Markdown 格式的提示词比 Rust 字符串拼接更易读
4. **编译时安全**：`include_str!` 保证模板文件存在，编译失败比运行时 panic 更安全
5. **易于扩展**：新增占位符只需在模板中添加 `{{XXX}}`，在 `build()` 中添加替换逻辑

---

## 验证

- `cargo build --workspace`：编译成功，0 错误，0 警告
- `cargo test --workspace`：全部 249 个测试通过，0 失败
- 原有 8 个 prompt 模块单元测试全部通过，输出内容结构与重构前一致
