# TUI 消息渲染视觉增强实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 TUI 聊天界面中实现用户/AI 消息背景色区分、去除工具/Thinking 边框、空 Thinking 隐藏、以及 Markdown 代码块和工具返回代码文件的 syntect 语法高亮与行号展示。

**Architecture:** 扩展 Theme 新增 bg_user_area/bg_ai_area，在 chat.rs 中为消息区域套带左边框的背景 Block；改造 Thinking/Tool 渲染器去除边框；新建 CodeBlockRenderer 辅助模块集成 syntect 高亮，TextRenderer 解析 markdown 代码块后分发渲染。

**Tech Stack:** Rust, ratatui, syntect, crossterm

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `crates/tui/Cargo.toml` | 修改 | 新增 syntect 依赖 |
| `crates/shared/src/dto.rs` | 修改 | ThemePreset 新增 `bg_user_area`、`bg_ai_area` |
| `crates/shared/src/preset_themes.json` | 修改 | 32 套主题补充两个颜色值 |
| `crates/tui/src/theme.rs` | 修改 | Theme 新增字段、style 方法、from_preset 映射 |
| `crates/tui/src/components/chat.rs` | 修改 | 用户/AI 消息区域背景色 + 左边框 |
| `crates/tui/src/components/part_renderer/thinking.rs` | 修改 | 空内容跳过，去边框 |
| `crates/tui/src/components/part_renderer/tool_call.rs` | 修改 | 去边框 |
| `crates/tui/src/components/part_renderer/tool_result.rs` | 修改 | 去边框，新增 `hinted_language` 支持代码块 |
| `crates/tui/src/components/part_renderer/tool_error.rs` | 修改 | 去边框 |
| `crates/tui/src/components/part_renderer/code_block.rs` | **新建** | syntect 语法高亮 + 行号核心模块 |
| `crates/tui/src/components/part_renderer/text.rs` | 修改 | 解析 markdown 代码块，分发到 CodeBlockRenderer |
| `crates/tui/src/components/part_renderer/mod.rs` | 修改 | 导出 code_block 模块 |
| `tests/e2e/tui_flow_e2e.rs` | 修改 | 补充代码块渲染 E2E 场景 |

---

## Task 1: 添加 syntect 依赖

**Files:**
- Modify: `crates/tui/Cargo.toml`

- [ ] **Step 1: 在 Cargo.toml 中添加 syntect**

在 `[dependencies]` 段落后追加：

```toml
syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes"] }
```

- [ ] **Step 2: 编译验证依赖可解析**

Run: `cargo check -p fi-code-tui`
Expected: 通过（首次可能耗时较长以下载编译 syntect）

---

## Task 2: Theme 扩展（新增 bg_user_area / bg_ai_area）

**Files:**
- Modify: `crates/shared/src/dto.rs`
- Modify: `crates/shared/src/preset_themes.json`
- Modify: `crates/tui/src/theme.rs`

- [ ] **Step 1: ThemePreset 新增字段**

在 `crates/shared/src/dto.rs` 的 `ThemePreset` 结构体中，`accent_hover` 字段后追加：

```rust
pub bg_user_area: u32,
pub bg_ai_area: u32,
```

- [ ] **Step 2: Theme 新增字段和方法**

在 `crates/tui/src/theme.rs` 的 `Theme` 结构体中，`accent_hover` 后追加：

```rust
pub bg_user_area: Color,
pub bg_ai_area: Color,
```

同步更新 `impl Theme`：
1. `deep_ocean()` 中新增：`bg_user_area: Color::from_u32(0x11161d), bg_ai_area: Color::from_u32(0x0d1117)`
2. `from_preset()` 中新增两行映射：`bg_user_area: Color::from_u32(preset.bg_user_area)` 等

- [ ] **Step 3: 为全部 32 套预设主题补充颜色值**

修改 `crates/shared/src/preset_themes.json`，每个主题对象在末尾追加：

```json
"bg_user_area": <值>,
"bg_ai_area": <值>
```

**配色规则**（以 JSON 十进制 u32 写入）：
- 暗色主题：`bg_user_area` 比 `bg_surface` 深约 5-10%（RGB 各减 0x05，不低于 `bg_base`）；`bg_ai_area` 等于 `bg_base`
- 亮色主题：`bg_user_area` 比 `bg_surface` 深约 3-5%；`bg_ai_area` 等于 `bg_base`

deep_ocean 示例：
- `bg_surface = 1448738 = 0x161b22` → `bg_user_area = 0x11161d = 1124125`
- `bg_base = 856343 = 0x0d1117` → `bg_ai_area = 856343`

- [ ] **Step 4: 编译验证**

Run: `cargo check -p fi-code-shared && cargo check -p fi-code-tui`
Expected: 通过

---

## Task 3: chat.rs 消息区域背景色 + 左边框

**Files:**
- Modify: `crates/tui/src/components/chat.rs`

### 关键设计：BgFill Widget

由于 AI 的 parts 渲染分散在多个 renderer 中，无法简单套一个 Paragraph Block。采用自定义 Widget 在 Buffer 层直接填充背景和左边框。

在 `chat.rs` 中新增辅助 Widget：

```rust
use ratatui::{buffer::Buffer, widgets::Widget};

struct BgFill {
    bg: Color,
}

impl Widget for BgFill {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(self.bg);
                }
            }
        }
    }
}
```

- [ ] **Step 1: 添加 BgFill Widget 和左边框绘制辅助函数**

在 `Chat` impl 之前或 `chat.rs` 底部添加 `BgFill` 定义，以及：

```rust
fn draw_left_border(buf: &mut Buffer, area: Rect, color: Color) {
    for y in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_fg(color);
            cell.set_char('│');
        }
    }
}
```

- [ ] **Step 2: 改造用户消息渲染（前缀 + 内容）**

替换现有用户消息渲染代码（约 line 435-458）为：

```rust
// 用户消息：前缀 + 内容，整体套背景和左边框
let user_text = Text::from(vec![
    Line::from(vec![
        Span::styled("● ", theme.style_user()),
        Span::styled("You", theme.style_user().add_modifier(Modifier::BOLD)),
    ]),
    Line::from(turn.user_message.clone()),
]);
let user_para = Paragraph::new(user_text)
    .wrap(Wrap { trim: true })
    .style(Style::default().bg(theme.bg_user_area));

// 基于 inner.width - 1 计算高度（左边框占 1 列）
let user_height = user_para.line_count(inner.width.saturating_sub(1)).max(1) as u16;

if let Some((rect, skip_lines)) = clip_rect(current_y, user_height) {
    // 1. 填充背景
    frame.render_widget(BgFill { bg: theme.bg_user_area }, rect);
    // 2. 画左边框
    draw_left_border(frame.buffer_mut(), rect, theme.user);
    // 3. 在 rect 内部（x+1, width-1）渲染内容
    let content_rect = Rect {
        x: rect.x + 1,
        y: rect.y,
        width: rect.width.saturating_sub(1),
        height: rect.height,
    };
    if content_rect.width > 0 {
        frame.render_widget(user_para.scroll((skip_lines, 0)), content_rect);
    }
}
current_y += user_height + 1; // +1 spacing
```

**注意**：`total_height()` 中用户消息高度计算也要同步改为 `inner.width.saturating_sub(1)`。

- [ ] **Step 3: 改造 AI Parts 区域渲染**

在 `draw()` 的 turn 循环中，将 AI parts 的渲染整体包裹在背景和左边框中。

由于 AI parts 渲染分散在 `while part_idx < turn.parts.len()` 循环中，且包含工具序列统计和摘要行，需要先计算 AI 区域总高度，再统一画背景和边框，最后在内部渲染。

**简化实现**：在现有 parts 渲染循环中，每次渲染 Part / group 标题 / 摘要行之前，先检查当前坐标是否处于 AI 区域的可见范围内。但由于 AI 区域是所有 parts 的连续区域，更简单的做法是：

在 turn 循环中，把现有 `while part_idx < turn.parts.len()` 的代码块整体提取为一个闭包，在该闭包外部包裹背景和左边框。

具体步骤：
1. 在 `for (turn_idx, turn) in self.turns.iter().enumerate()` 循环中，用户消息渲染后
2. 计算 AI 区域高度（复用 `total_height()` 中相似的逻辑，或直接遍历 parts 累加）
3. 用 `clip_rect` 获取 AI 区域 rect
4. 渲染 `BgFill` 和左边框
5. 在内部区域（x+1, width-1）中，用局部 `ai_current_y` 渲染所有 parts

由于代码较长，这里给出关键框架：

```rust
// === AI 区域 ===
// 1. 计算 AI 区域总高度
let ai_height = {
    let mut h = 0u16;
    let mut pidx = 0usize;
    let mut tool_count = 0usize;
    while pidx < turn.parts.len() {
        let part = &turn.parts[pidx];
        if matches!(part, Part::ToolUse { .. }) {
            // ... 工具序列高度累加（与 total_height 相同逻辑）
        } else {
            if let Some(renderer) = self.renderer_registry.get(part) {
                h += renderer.height(part, inner.width.saturating_sub(1));
                h += 1; // spacing
            }
            pidx += 1;
        }
    }
    // 执行摘要行
    if tool_count > 0 { h += 1 + 1; }
    h
};

// 2. 渲染 AI 背景 + 左边框
let ai_block_rect = if let Some((rect, _)) = clip_rect(current_y, ai_height) {
    frame.render_widget(BgFill { bg: theme.bg_ai_area }, rect);
    draw_left_border(frame.buffer_mut(), rect, theme.brand);
    Some(Rect {
        x: rect.x + 1,
        y: rect.y,
        width: rect.width.saturating_sub(1),
        height: rect.height,
    })
} else {
    None
};

// 3. 在 ai_block_rect 内部渲染所有 parts
// 需要一个新的 clip_rect 闭包，基于 ai_inner 坐标
let ai_clip_rect = |cy: u16, h: u16| -> Option<(Rect, u16)> {
    let ai_inner = ai_block_rect?;
    // 计算相对于 ai_inner 的视口裁剪
    // ...（与现有 clip_rect 逻辑类似，但基于 ai_inner 的坐标系）
};
```

由于 `draw()` 中已有 `clip_rect` 闭包基于 `inner` 坐标，重构为支持 AI 区域需要仔细处理。这里建议在实现时直接修改 `draw()` 方法，将 `inner` 在 AI 区域阶段临时替换为 `ai_inner`（x+1, width-1），然后复用现有的 `clip_rect` 和 parts 渲染逻辑。

- [ ] **Step 4: 同步更新 total_height()**

用户消息的 `content_height` 计算需要基于 `inner.width.saturating_sub(1)`（左边框占 1 列）。AI parts 的高度计算也要基于 `inner.width.saturating_sub(1)`。

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 4: ThinkingRenderer 空内容跳过 + 去边框

**Files:**
- Modify: `crates/tui/src/components/part_renderer/thinking.rs`

- [ ] **Step 1: 修改 height()**

```rust
fn height(&self, part: &Part, width: u16) -> u16 {
    if let Part::Reasoning { thinking, .. } = part {
        if thinking.trim().is_empty() {
            return 0;
        }
        let lines: Vec<&str> = thinking.lines().collect();
        let mut h = 0u16;
        for line in lines {
            let w = line.width() as u16;
            h += (w / width.max(1)).max(0) + 1;
        }
        h.max(1) + 1 // +1 for title line only (no borders)
    } else {
        0
    }
}
```

- [ ] **Step 2: 修改 draw()**

```rust
fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme, skip_lines: u16) {
    if let Part::Reasoning { thinking, .. } = part {
        if thinking.trim().is_empty() {
            return;
        }
        // 标题行单独渲染在顶部
        let title = Line::from("▼ Thinking")
            .style(theme.style_muted().add_modifier(Modifier::BOLD));
        if skip_lines == 0 && area.height > 0 {
            frame.render_widget(Paragraph::new(title), Rect {
                x: area.x, y: area.y, width: area.width, height: 1,
            });
        }
        // 内容渲染在标题下方
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };
        if content_area.height > 0 {
            let para = Paragraph::new(thinking.as_str())
                .wrap(Wrap { trim: true })
                .style(theme.style_primary())
                .scroll((skip_lines.saturating_sub(1), 0));
            frame.render_widget(para, content_area);
        }
    }
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 5: Tool 渲染器去边框

**Files:**
- Modify: `crates/tui/src/components/part_renderer/tool_call.rs`
- Modify: `crates/tui/src/components/part_renderer/tool_result.rs`
- Modify: `crates/tui/src/components/part_renderer/tool_error.rs`

- [ ] **Step 1: ToolCallRenderer 去边框**

去掉 `Block::default().borders(Borders::ALL)`，保留标题行（带图标和工具名，BOLD），内容直接展示。

`height()` 去掉 `+2` 边框补偿，改为 `h.max(1) + 1`（+1 标题行）。

- [ ] **Step 2: ToolResultRenderer 去边框**

同上，去掉边框 Block。`height()` 去掉 `+2` 补偿。

保留底部耗时 footer（`duration_ms`），标题行保留 "✅ Result" / "❌ Result"。

- [ ] **Step 3: ToolErrorRenderer 去边框**

同上，去掉边框 Block。`height()` 去掉 `+2` 补偿。

保留标题行 "❌ Tool Error"。

- [ ] **Step 4: 编译验证**

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 6: CodeBlockRenderer 核心模块

**Files:**
- **Create**: `crates/tui/src/components/part_renderer/code_block.rs`
- Modify: `crates/tui/src/components/part_renderer/mod.rs`

- [ ] **Step 1: 新建 code_block.rs**

文件内容包含：
1. `CodeBlockRenderer` 结构体（无状态）
2. `height(code, available_width) -> u16`：计算代码块高度（含行号列）
3. `draw(frame, area, code, language, theme, skip_lines)`：语法高亮 + 行号渲染
4. `detect_language_from_extension(path: &str) -> Option<&'static str>`：扩展名到语言映射
5. `syntect_theme_for_bg(base_color: Color) -> &'static str`：根据背景亮度选择 syntect 主题

核心实现使用 `syntect::highlighting::HighlightLines` 和 `syntect::parsing::SyntaxSet`。

- [ ] **Step 2: 导出模块**

在 `mod.rs` 中 `pub mod code_block;`。

- [ ] **Step 3: 编译验证**

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 7: TextRenderer 改造（接入 CodeBlockRenderer）

**Files:**
- Modify: `crates/tui/src/components/part_renderer/text.rs`

- [ ] **Step 1: 解析 Markdown 代码块**

在 `text.rs` 中定义 `TextFragment` 枚举和解析函数：

```rust
enum TextFragment<'a> {
    Plain(&'a str),
    CodeBlock { language: Option<&'a str>, code: &'a str },
}

fn parse_fragments(text: &str) -> Vec<TextFragment> {
    // 使用正则匹配 ```(\w*)\n([\s\S]*?)\n```
    // 返回交替的 Plain 和 CodeBlock 片段
}
```

- [ ] **Step 2: 改造 height()**

遍历 `parse_fragments(text)`：
- `Plain`：用 `Paragraph::line_count(width)`
- `CodeBlock`：用 `CodeBlockRenderer::height(code, width)`
- 累加所有片段高度

- [ ] **Step 3: 改造 draw()**

遍历 fragments，按顺序渲染：
- `Plain`：现有 Paragraph 方式
- `CodeBlock`：调用 `CodeBlockRenderer::draw()`

每个片段渲染后更新 y 偏移。

- [ ] **Step 4: 编译验证**

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 8: ToolResultRenderer 增强（代码文件 hint）

**Files:**
- Modify: `crates/tui/src/components/part_renderer/tool_result.rs`
- Modify: `crates/tui/src/components/chat.rs`

- [ ] **Step 1: ToolResultRenderer 新增 hinted_language 字段**

```rust
pub struct ToolResultRenderer {
    pub hinted_language: Option<String>,
}

impl ToolResultRenderer {
    pub fn new() -> Self {
        Self { hinted_language: None }
    }
}
```

在 `draw()` 中，如果 `hinted_language.is_some()` 且内容是多行文本，使用 `CodeBlockRenderer` 渲染；否则保持现有纯文本渲染。

- [ ] **Step 2: chat.rs 传递 hint**

在渲染 tool sequence 时，遇到 `Part::ToolUse { name, arguments }` 且 name 为 `read`/`read_file` 时，从 arguments 提取 path，通过扩展名检测语言，设置到 `ToolResultRenderer.hinted_language`。

- [ ] **Step 3: 编译验证**

Run: `cargo check -p fi-code-tui`
Expected: 通过

---

## Task 9: 单元测试

**Files:**
- Modify: `crates/tui/src/components/part_renderer/code_block.rs`
- Modify: `crates/tui/src/components/part_renderer/thinking.rs`
- Modify: `crates/tui/src/theme.rs`

- [ ] **Step 1: CodeBlockRenderer 测试**

添加 `#[cfg(test)]` 模块：
- `test_parse_code_blocks`：测试 markdown 代码块解析
- `test_line_number_width`：测试行号列宽度计算
- `test_empty_code_height`：空代码返回高度 0

- [ ] **Step 2: ThinkingRenderer 测试**

添加测试：
- `test_empty_thinking_height_zero`：空 thinking 返回 0
- `test_empty_thinking_draw_nop`：空 thinking draw 不操作 buffer

- [ ] **Step 3: Theme 测试**

添加测试：
- `test_all_presets_have_bg_fields`：所有预设反序列化后 bg_user_area 和 bg_ai_area 不为 0

- [ ] **Step 4: 运行测试**

Run: `cargo test -p fi-code-tui`
Expected: 新增测试通过，原有测试不受影响

---

## Task 10: E2E 测试补充

**Files:**
- Modify: `tests/e2e/tui_flow_e2e.rs`

- [ ] **Step 1: 添加代码块渲染场景**

在现有 TUI flow 测试中，增加一个发送含 Markdown 代码块消息的步骤，验证 TUI 无 panic 且输出包含预期样式。

- [ ] **Step 2: 运行 E2E 测试**

Run: `cargo test --test tui_flow_e2e`
Expected: 通过

---

## Task 11: 编译与格式检查

- [ ] **Step 1: 全量编译**

Run: `cargo build --bin fi-code-tui`
Expected: 通过

- [ ] **Step 2: 格式化**

Run: `cargo fmt`

- [ ] **Step 3: Clippy**

Run: `cargo clippy -p fi-code-tui`
Expected: 无新增 warning

- [ ] **Step 4: 全量测试**

Run: `cargo test`
Expected: 全部通过

---

## Self-Review

**1. Spec coverage:**
- ✅ 消息背景色区分 — Task 2 (Theme) + Task 3 (chat.rs 背景 + 左边框)
- ✅ Think 空内容隐藏 — Task 4
- ✅ 工具/信息流去边框 — Task 5
- ✅ 代码块行号 + 语法高亮 — Task 6 (CodeBlockRenderer) + Task 7 (TextRenderer) + Task 8 (ToolResult)

**2. Placeholder scan:**
- ✅ 无 TBD/TODO
- ✅ 所有步骤包含具体文件路径和代码框架
- ✅ 编译验证命令明确

**3. Type consistency:**
- ✅ `ThemePreset` 和 `Theme` 字段名一致：`bg_user_area`, `bg_ai_area`
- ✅ `CodeBlockRenderer` API 在 Task 6 定义，Task 7/8 调用时签名一致
