# TUI 消息渲染视觉增强设计

> 设计日期：2026-05-16
> 状态：待实现

---

## 1. 背景与目标

当前 TUI 聊天界面的视觉呈现存在以下可改进点：
- 用户消息与 AI 消息缺乏背景色区分，阅读长对话时难以快速定位角色
- Thinking（推理）块即使内容为空也会渲染一个带边框的占位区域
- 工具调用（ToolUse/ToolResult/ToolError）和 Thinking 均带有 `Borders::ALL` 边框，信息流显得拥挤
- 代码块和普通文本无差别展示，缺少行号和语法高亮

本设计目标是改善上述 4 个问题，提升 TUI 的可读性和视觉层次感。

---

## 2. 需求摘要

| 需求 | 说明 |
|------|------|
| 消息背景色区分 | 用户消息区域使用深色背景，AI 消息区域使用浅色背景，两者均有左边框（用户=橙色，AI=品牌色） |
| Think 空内容隐藏 | `Part::Reasoning` 的 `thinking` 字段为空或仅空白时，不占用高度、不渲染 |
| 去除信息流边框 | `ThinkingRenderer`、`ToolCallRenderer`、`ToolResultRenderer`、`ToolErrorRenderer` 均移除 `Borders::ALL` |
| 代码块行号 + 语法高亮 | Markdown 代码块（```language）和工具返回的已知代码文件使用 syntect 高亮，左侧显示行号 |

---

## 3. Theme 扩展

### 3.1 新增字段

在 `ThemePreset`（`crates/shared/src/dto.rs`）和 `Theme`（`crates/tui/src/theme.rs`）中新增：

```rust
pub bg_user_area: Color,  // 用户消息区域背景色
pub bg_ai_area: Color,    // AI 消息区域背景色
```

### 3.2 配色策略

全部 32 套内置主题统一补充这两个颜色：

- **暗色主题**：`bg_user_area` 比 `bg_surface` 再深 5-10%；`bg_ai_area` 接近或等于 `bg_base`
- **亮色主题**：`bg_user_area` 比 `bg_surface` 稍深（更灰）；`bg_ai_area` 接近 `bg_base`

**示例（deep_ocean）**：
- `bg_surface = 0x161b22` → `bg_user_area = 0x11161d`，`bg_ai_area = 0x0d1117`

### 3.3 左边框颜色

不使用新增字段，而是复用现有语义色：
- 用户消息左边框：`theme.user`（橙色）
- AI 消息左边框：`theme.brand`（品牌色/青色）

---

## 4. 消息区域背景渲染

### 4.1 渲染位置

在 `crates/tui/src/components/chat.rs` 的 `draw()` 方法中：

1. **用户消息区域**：前缀 "● You" + `turn.user_message` 内容整体包裹在一个带背景色的 `Block` 内
   - `Block::default().borders(Borders::LEFT).border_style(theme.user).style(Style::default().bg(theme.bg_user_area))`
   
2. **AI 消息区域**：`turn.parts` 中所有非工具序列的 Part，以及工具序列组，整体包裹在另一个带背景色的 `Block` 内
   - `Block::default().borders(Borders::LEFT).border_style(theme.brand).style(Style::default().bg(theme.bg_ai_area))`

### 4.2 高度计算

`total_height()` 需要同步计算背景 Block 的 padding（上下各 1 行）。

---

## 5. 空 Thinking 隐藏

### 5.1 条件判断

`ThinkingRenderer` 在 `height()` 和 `draw()` 入口处增加：

```rust
if thinking.trim().is_empty() {
    return 0; // height
    // 或 draw 中直接 return
}
```

### 5.2 去边框后的样式

去掉 `Borders::ALL` 和 `+2` 的高度补偿。保留标题行 "▼ Thinking"（`theme.text_muted` + `BOLD`），内容使用 `theme.style_primary()` 纯文本渲染。

---

## 6. 工具/信息流去边框

以下渲染器统一移除边框：

| 渲染器 | 当前边框 | 去边框后保留元素 |
|--------|---------|-----------------|
| `ToolCallRenderer` | 按工具类型彩色边框 | 标题行（图标+工具名，BOLD） |
| `ToolResultRenderer` | 绿色/红色边框 | 标题（✅/❌ Result）、底部 ⏱ 耗时行 |
| `ToolErrorRenderer` | 红色边框 | 标题 "❌ Tool Error" |
| `ThinkingRenderer` | 灰色边框 | 标题 "▼ Thinking" |

**高度计算同步**：所有上述渲染器的 `height()` 去掉 `+2` 的边框补偿。

---

## 7. CodeBlockRenderer（核心新增模块）

### 7.1 模块定位

`crates/tui/src/components/part_renderer/code_block.rs` — 不实现 `PartRenderer` trait，而是作为辅助模块被 `TextRenderer` 和 `ToolResultRenderer` 调用。

### 7.2 核心 API

```rust
pub struct CodeBlockRenderer;

impl CodeBlockRenderer {
    pub fn height(code: &str, available_width: u16) -> u16;
    pub fn draw(frame: &mut Frame, area: Rect, code: &str, language: Option<&str>, theme: &Theme, skip_lines: u16);
}
```

### 7.3 syntect 集成

- **依赖**：`syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes"] }`
- **语法集**：`SyntaxSet::load_defaults_newlines()`
- **高亮主题集**：`ThemeSet::load_defaults()`
- **主题选择策略**：根据 TUI `Theme.bg_base` 的亮度自动选择
  - 亮度 > 128（亮色）→ syntect `"base16-ocean.light"`
  - 否则 → syntect `"base16-ocean.dark"`
- **颜色映射**：syntect 的 RGB 直接映射为 `ratatui::style::Color::Rgb(r, g, b)`

### 7.4 行号处理

- 行号列宽度 = `代码总行数的位数 + 1`（右对齐 + 1 空格分隔）
- 行号颜色：`theme.text_muted`
- 代码内容区域宽度 = `available_width - 行号列宽度`
- 代码内容允许自动换行（`Paragraph::wrap(Wrap { trim: false })`）
- 代码块整体背景：`theme.bg_surface`

### 7.5 TextRenderer 改造

`Part::Text` 内容需要预处理，解析为 **纯文本段落** 和 **Markdown 代码块** 交替的片段：

```rust
enum TextFragment<'a> {
    Plain(&'a str),
    CodeBlock { language: Option<&'a str>, code: &'a str },
}
```

**代码块识别规则**：匹配 ```(\w*)\n([\s\S]*?)\n``` 模式，仅处理 fenced code block，不处理行内代码。

`height()` 和 `draw()` 遍历 fragments 累加/渲染。

### 7.6 ToolResult 代码文件高亮

**检测策略**：在 `chat.rs` 渲染 tool sequence 时，检查前一个 `ToolUse`：
- 如果 ToolUse 是 `read` / `read_file`，且 path 扩展名属于已知代码类型（.rs, .py, .js, .ts, .go, .java, .c, .cpp, .h, .sh, .json, .yaml, .toml 等）
- 则通过 `ToolResultRenderer` 的可选状态 `hinted_language` 传递语言标识

`ToolResultRenderer` 收到 hint 后，对多行内容使用 `CodeBlockRenderer` 渲染。

---

## 8. 模块划分

```
crates/tui/src/
  theme.rs                          # 新增 bg_user_area, bg_ai_area
  components/
    chat.rs                         # 用户/AI 区域背景 + 左边框
    part_renderer/
      mod.rs                        # 导出 code_block
      text.rs                       # 解析 markdown 代码块，分发渲染
      code_block.rs                 # 新建：syntect 高亮 + 行号
      thinking.rs                   # 空内容跳过，去边框
      tool_call.rs                  # 去边框
      tool_result.rs                # 去边框，支持代码块 hint
      tool_error.rs                 # 去边框

crates/shared/src/
  dto.rs                            # ThemePreset 新增字段
  preset_themes.json                # 32 套主题补充颜色

crates/tui/Cargo.toml               # 新增 syntect 依赖
```

---

## 9. 测试策略

| 测试层级 | 内容 |
|----------|------|
| 单元测试 | `code_block.rs`：测试 markdown 解析正则、行号宽度计算、空代码高度 |
| 单元测试 | `thinking.rs`：空 thinking 返回 height=0 且 draw 不渲染 |
| 单元测试 | `theme.rs`：新增字段在所有预设主题中可正确反序列化 |
| E2E 测试 | TUI flow：发送含代码块的消息，验证无 panic，输出包含高亮样式 |
| 视觉验证 | 人工检查：不同主题下用户/AI 背景对比度、代码高亮可读性 |

---

## 10. 安全与边界

- **syntect 体积**：使用 `default-features = false` 去除不需要的特性，减少二进制体积影响
- **不支持的语言**：syntect 找不到对应语法时 fallback 到纯文本 + 行号，不报错
- **超大代码块**：高度计算和渲染保持流式，不一次性加载全部到内存
- **主题兼容性**：所有新增颜色字段在 32 套预设主题中均有值，自定义旧主题缺失字段时 fallback 到 `bg_surface`

---

## 11. 与现有系统的关系

| 现有系统 | 关系 |
|----------|------|
| `PartRendererRegistry` | 不修改注册逻辑，`CodeBlockRenderer` 作为辅助模块，不注册为独立 renderer |
| `ThemePreset` / `preset_themes.json` | 新增两个字段，需要同步更新所有预设 JSON |
| `frontend` 主题系统 | `ThemePreset` 字段变更会自动同步到前端，但前端本次不涉及 TUI 渲染改动 |
| `chat.rs` 滚动/裁剪逻辑 | 背景 Block 的 padding 需要同步到 `total_height()`，确保滚动范围正确 |
