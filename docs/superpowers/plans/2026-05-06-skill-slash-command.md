# /skill 命令实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 TUI 中新增 `/skill` 交互式子菜单命令，列出所有可用 Skill 并支持选中加载。

**Architecture:** 复用 `Input` 组件已有的 `submenu_mode` 机制（与 `/theme` 一致），通过新增 `SubmenuKind` 枚举区分 Theme/Skill 两种子菜单类型，选中后调用 `skills::load_skill_content` 注入聊天区。

**Tech Stack:** Rust, ratatui, tokio, crossterm

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/tui/event.rs` | 修改 | 新增 `AppEvent::SelectSkill(String)` |
| `src/tui/components/input.rs` | 修改 | 新增 `SubmenuKind` 枚举，改造 `submenu_mode` 为带类型的状态机，区分 Theme/Skill 事件 |
| `src/tui/app.rs` | 修改 | `/theme` 调用适配新签名，新增 `/skill` 触发逻辑，新增 `SelectSkill` 事件处理 |
| `src/server/server.rs` | 修改 | 注册 `/skill` 命令到 `CommandRegistry` |
| `src/tui/components/input.rs` | 修改（测试区） | 补充 `SubmenuKind` 相关单元测试 |

---

### Task 1: 新增 AppEvent::SelectSkill

**Files:**
- Modify: `src/tui/event.rs`

- [ ] **Step 1: 在 AppEvent 枚举中新增 SelectSkill**

在 `SelectTheme(usize)` 下方插入新变体：

```rust
// src/tui/event.rs
pub enum AppEvent {
    // ... 现有变体 ...
    SelectTheme(usize),
    PreviewTheme(usize),
    SelectSkill(String),   // ← 新增
    CancelThemePreview,
    // ...
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check`
Expected: PASS（新增未使用的变体不会导致编译错误）

- [ ] **Step 3: Commit**

```bash
git add src/tui/event.rs
git commit -m "feat(tui): add AppEvent::SelectSkill variant"
```

---

### Task 2: Input 组件支持区分子菜单类型

**Files:**
- Modify: `src/tui/components/input.rs`

**背景：** 现有 `submenu_mode` 是 `bool`，只能服务 `/theme`。`/skill` 也要用子菜单，因此需要让 `Input` 知道当前子菜单属于哪种命令，从而返回不同的事件。

- [ ] **Step 1: 新增 SubmenuKind 枚举**

在 `Input` 结构体定义之前插入：

```rust
/// 子菜单类型，用于区分不同命令打开的交互式菜单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmenuKind {
    Theme,
    Skill,
}
```

- [ ] **Step 2: 修改 Input 结构体**

将 `submenu_mode: bool` 改为 `submenu_kind: Option<SubmenuKind>`：

```rust
pub struct Input {
    // ... 现有字段 ...
    submenu_kind: Option<SubmenuKind>,  // ← 替换原来的 submenu_mode: bool
    submenu_items: Vec<(String, String)>,
    submenu_selected: usize,
    submenu_loaded: bool,
}
```

- [ ] **Step 3: 修改 new() 初始化**

```rust
impl Input {
    pub fn new() -> Self {
        Self {
            // ... 现有字段 ...
            submenu_kind: None,  // ← 替换 submenu_mode: false
            submenu_items: Vec::new(),
            submenu_selected: 0,
            submenu_loaded: false,
        }
    }
```

- [ ] **Step 4: 修改 enter_submenu_mode 接受类型参数**

```rust
pub fn enter_submenu_mode(&mut self, kind: SubmenuKind) {
    self.submenu_kind = Some(kind);
    self.submenu_selected = 0;
    self.dropdown_visible = true;
}
```

- [ ] **Step 5: 修改 close_submenu**

```rust
pub fn close_submenu(&mut self) {
    self.submenu_kind = None;
    self.dropdown_visible = false;
}
```

- [ ] **Step 6: 修改 is_submenu_open**

```rust
pub fn is_submenu_open(&self) -> bool {
    self.submenu_kind.is_some() && self.dropdown_visible
}
```

- [ ] **Step 7: 修改 update_dropdown_area 中的 submenu_mode 判断**

将 `if self.submenu_mode {` 改为 `if self.submenu_kind.is_some() {`：

```rust
let items_len = if self.submenu_kind.is_some() {
    self.submenu_items.len() as u16
} else {
    self.dropdown_items.len() as u16
};
```

- [ ] **Step 8: 修改 handle_event 中的键盘事件分发**

将 `if self.submenu_mode {` 改为 `if self.submenu_kind.is_some() {`，并将内部 match 替换为按类型分发：

```rust
if self.dropdown_visible {
    if let Some(kind) = self.submenu_kind {
        match key.code {
            KeyCode::Up => {
                if self.submenu_selected > 0 {
                    self.submenu_selected -= 1;
                }
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::PreviewTheme(self.submenu_selected)),
                    SubmenuKind::Skill => None,
                };
            }
            KeyCode::Down => {
                if self.submenu_selected < self.submenu_items.len().saturating_sub(1) {
                    self.submenu_selected += 1;
                }
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::PreviewTheme(self.submenu_selected)),
                    SubmenuKind::Skill => None,
                };
            }
            KeyCode::Enter => {
                if self.submenu_selected < self.submenu_items.len() {
                    let idx = self.submenu_selected;
                    match kind {
                        SubmenuKind::Theme => {
                            self.close_submenu();
                            return Some(AppEvent::SelectTheme(idx));
                        }
                        SubmenuKind::Skill => {
                            let name = self.submenu_items[idx].0.clone();
                            self.close_submenu();
                            return Some(AppEvent::SelectSkill(name));
                        }
                    }
                }
            }
            KeyCode::Esc => {
                self.close_submenu();
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::CancelThemePreview),
                    SubmenuKind::Skill => None,
                };
            }
            _ => {
                self.close_submenu();
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::CancelThemePreview),
                    SubmenuKind::Skill => None,
                };
            }
        }
    } else {
        // ... 原有的非 submenu 下拉菜单逻辑，保持不变 ...
```

- [ ] **Step 9: 修改 handle_event 中的鼠标事件分发**

将鼠标事件处理中的 `if self.submenu_mode {` 改为 `if let Some(kind) = self.submenu_kind {`，并将 `return Some(AppEvent::PreviewTheme(...))` 和 `return Some(AppEvent::SelectTheme(...))` 按 kind 分发：

```rust
Event::Mouse(mouse) => {
    if let Some(kind) = self.submenu_kind {
        match mouse.kind {
            crossterm::event::MouseEventKind::ScrollUp => {
                if self.submenu_selected > 0 {
                    self.submenu_selected -= 1;
                }
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::PreviewTheme(self.submenu_selected)),
                    SubmenuKind::Skill => None,
                };
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                if self.submenu_selected < self.submenu_items.len().saturating_sub(1) {
                    self.submenu_selected += 1;
                }
                return match kind {
                    SubmenuKind::Theme => Some(AppEvent::PreviewTheme(self.submenu_selected)),
                    SubmenuKind::Skill => None,
                };
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                if let Some(area) = self.dropdown_area {
                    let mx = mouse.column;
                    let my = mouse.row;
                    if mx >= area.x && mx < area.x + area.width
                        && my >= area.y && my < area.y + area.height
                    {
                        let item_y = my.saturating_sub(area.y + 1);
                        let index = item_y as usize;
                        if index < self.submenu_items.len() {
                            self.submenu_selected = index;
                            let idx = self.submenu_selected;
                            match kind {
                                SubmenuKind::Theme => {
                                    self.close_submenu();
                                    return Some(AppEvent::SelectTheme(idx));
                                }
                                SubmenuKind::Skill => {
                                    let name = self.submenu_items[idx].0.clone();
                                    self.close_submenu();
                                    return Some(AppEvent::SelectSkill(name));
                                }
                            }
                        }
                    } else {
                        self.close_submenu();
                        return match kind {
                            SubmenuKind::Theme => Some(AppEvent::CancelThemePreview),
                            SubmenuKind::Skill => None,
                        };
                    }
                }
                None
            }
            _ => None,
        }
    } else {
        // ... 原有的非 submenu 鼠标逻辑，保持不变 ...
```

- [ ] **Step 10: 编译检查**

Run: `cargo check`
Expected: PASS（可能提示未使用的 `SubmenuKind` 导入，后续会被 app.rs 使用）

- [ ] **Step 11: Commit**

```bash
git add src/tui/components/input.rs
git commit -m "feat(tui): add SubmenuKind to distinguish Theme vs Skill submenu"
```

---

### Task 3: TuiApp 中 /skill 命令与事件处理

**Files:**
- Modify: `src/tui/app.rs`

- [ ] **Step 1: 更新 /theme 调用，传入 SubmenuKind::Theme**

找到 `handle_execute_slash_command` 中 `name == "theme"` 的分支，修改 `enter_submenu_mode()` 调用：

```rust
// src/tui/app.rs
fn handle_execute_slash_command(&mut self, name: &str, _args_hint: &Option<String>) {
    if name == "theme" {
        self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Theme);
        // ... 其余逻辑保持不变 ...
    }
```

- [ ] **Step 2: 新增 /skill 处理分支**

在同一个 `handle_execute_slash_command` 方法中，在 `name == "theme"` 分支之前（或之后）插入：

```rust
if name == "skill" {
    let registry = crate::skills::get_registry();
    if registry.entries.is_empty() {
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(AppEvent::ShowSystemMessage(
                "No skills available.".into(),
            )).await;
        });
        return;
    }
    self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Skill);
    let items: Vec<(String, String)> = registry
        .entries
        .iter()
        .map(|e| (e.metadata.name.clone(), e.metadata.description.clone()))
        .collect();
    self.input.set_submenu_items(items);
    return;
}
```

- [ ] **Step 3: 新增 AppEvent::SelectSkill 处理**

在 `handle_app_event` 方法中，找到 `AppEvent::SelectTheme(index)` 分支附近，插入新分支：

```rust
AppEvent::SelectSkill(ref name) => {
    self.input.close_submenu();
    match crate::skills::load_skill_content(name) {
        Ok(content) => {
            self.chat.add_system_message(&format!(
                "Skill '{}' loaded.\n\n{}",
                name, content
            ));
        }
        Err(e) => {
            self.chat.add_system_message(&format!(
                "Failed to load skill '{}': {}",
                name, e
            ));
        }
    }
}
```

- [ ] **Step 4: 编译检查**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat(tui): add /skill slash command with submenu and loading"
```

---

### Task 4: Server 端注册 /skill 命令

**Files:**
- Modify: `src/server/server.rs`

- [ ] **Step 1: 在 CommandRegistry 中注册 /skill**

找到 `Server::new` 中注册 `/clear` 和 `/theme` 的位置，在其后添加 `/skill`：

```rust
// src/server/server.rs
commands.register(
    CommandMeta {
        name: "skill".into(),
        description: "List and load available skills".into(),
        args_hint: None,
    },
    Box::new(SkillCommandHandler),
);
```

- [ ] **Step 2: 实现 SkillCommandHandler**

在同一文件（`src/server/server.rs`）中，找到其他 Handler 的实现位置，添加：

```rust
struct SkillCommandHandler;

#[async_trait]
impl CommandHandler for SkillCommandHandler {
    async fn execute(&self, _args: Option<String>, _ctx: &CommandContext) -> Result<CommandOutput> {
        Ok(CommandOutput {
            message: String::new(),
            r#type: OutputType::Silent,
            metadata: None,
        })
    }
}
```

> 说明：TUI 模式下 `/skill` 的交互完全在客户端处理，Server 端只需注册命令元数据使其出现在 `/` 下拉列表中。实际加载逻辑在 TUI 本地执行。

- [ ] **Step 3: 编译检查**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/server/server.rs
git commit -m "feat(server): register /skill command in CommandRegistry"
```

---

### Task 5: 补充单元测试

**Files:**
- Modify: `src/tui/components/input.rs`（测试区底部）

- [ ] **Step 1: 新增 SubmenuKind 相关测试**

在 `#[cfg(test)]` 模块末尾追加：

```rust
#[test]
fn test_submenu_kind_theme() {
    let mut input = Input::new();
    input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Theme);
    assert!(input.is_submenu_open());
    assert_eq!(input.submenu_kind, Some(crate::tui::components::input::SubmenuKind::Theme));
}

#[test]
fn test_submenu_kind_skill() {
    let mut input = Input::new();
    input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Skill);
    assert!(input.is_submenu_open());
    assert_eq!(input.submenu_kind, Some(crate::tui::components::input::SubmenuKind::Skill));
}

#[test]
fn test_close_submenu_clears_kind() {
    let mut input = Input::new();
    input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Skill);
    input.close_submenu();
    assert!(!input.is_submenu_open());
    assert_eq!(input.submenu_kind, None);
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test tui::components::input::tests`
Expected: ALL PASS

- [ ] **Step 3: 运行全部测试**

Run: `cargo test`
Expected: 106+ tests PASS, 0 FAIL

- [ ] **Step 4: Commit**

```bash
git add src/tui/components/input.rs
git commit -m "test(tui): add SubmenuKind unit tests"
```

---

## Self-Review

### Spec Coverage

| Spec 需求 | 对应 Task |
|-----------|-----------|
| 新增 `AppEvent::SelectSkill` | Task 1 |
| Input 子菜单支持区分 Theme/Skill | Task 2 |
| TUI 中 `/skill` 触发子菜单 | Task 3 |
| 选中后加载 Skill 内容到聊天区 | Task 3 |
| 空 Skill 列表时提示 | Task 3 |
| Server 端注册 `/skill` | Task 4 |
| 单元测试 | Task 5 |

### Placeholder Scan

- 无 TBD/TODO
- 所有代码块均为完整实现
- 无 "add appropriate error handling" 等模糊描述

### Type Consistency

- `SubmenuKind` 在 Task 2 中定义，Task 3 和 Task 5 中引用路径一致：`crate::tui::components::input::SubmenuKind`
- `AppEvent::SelectSkill(String)` 在 Task 1 中定义，Task 3 中匹配使用
