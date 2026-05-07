# /skill 命令设计文档

> 日期：2026-05-06
> 状态：设计中

## 1. 需求概述

新增 `/skill` 斜杠命令，在 TUI 模式下触发交互式子菜单，列出当前所有可用的 Skill。用户通过方向键选择，回车确认后直接加载该 Skill（相当于调用 `use_skill` 工具），将 Skill 内容注入当前会话上下文。

CLI 模式下暂不提供 `/skill` 支持。

## 2. 架构设计

### 2.1 Skill 数据流

```
SkillRegistry (全局 LazyLock)
  │
  ├── 启动时 init_skills() 扫描多来源目录
  │
  └── get_registry() ──► TuiApp (读取 entries 构建菜单项)

TUI
  ├── 输入 "/skill" ──► 子菜单显示所有 Skill 名称+描述
  ├── 方向键移动 → 高亮选中项
  └── 回车确认 → AppEvent::SelectSkill(name)
      │
      ▼
  TuiApp 调用 skills::load_skill_content(&name)
      │
      ▼
  将内容作为系统消息追加到 Chat 区
```

### 2.2 与 /theme 命令的对比

| 维度 | /theme | /skill |
|------|--------|--------|
| 触发方式 | `/theme` 或 Ctrl+T | `/skill` |
| 菜单类型 | 子菜单（submenu_mode） | 子菜单（submenu_mode） |
| 方向键行为 | 上下移动 + 实时预览主题 | 上下移动 + 高亮选中 |
| 确认行为 | 回车 → HTTP 切换主题并固定 | 回车 → 加载 Skill 内容到聊天区 |
| Esc 行为 | 恢复原来主题并关闭菜单 | 直接关闭菜单 |
| 数据来源 | Server 端 ThemePreset 列表 | 本地 SkillRegistry |

## 3. 核心数据结构

### 3.1 SkillEntry（已有）

```rust
// src/skills/skill_type.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,                    // 格式: "{scope}-{name}"
    pub scope: String,
    pub source_type: SkillSourceType,
    pub symlink_path: PathBuf,
    pub target_path: PathBuf,
    pub metadata: SkillMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}
```

### 3.2 新增 AppEvent

```rust
// src/tui/event.rs
pub enum AppEvent {
    // ... 现有事件 ...
    SelectSkill(String),    // 确认加载指定 Skill
}
```

### 3.3 命令元数据

```rust
CommandMeta {
    name: "skill",
    description: "List and load available skills",
    args_hint: None,
}
```

## 4. 交互流程

### 4.1 触发子菜单

```
用户输入框键入 "/skill" 并回车
    │
    ▼
Input::handle_event ──► AppEvent::ExecuteSlashCommand { name: "skill" }
    │
    ▼
TuiApp::handle_execute_slash_command("skill")
    │
    ├── 检查是否有可用 Skill（SkillRegistry.entries 非空）
    ├── 若为空 → 发送 AppEvent::ShowSystemMessage("No skills available.")
    ├── 若非空：
    │   ├── input.enter_submenu_mode()
    │   ├── 遍历 SkillRegistry.entries 构造 Vec<(name, description)>
    │   └── input.set_submenu_items(items)
    │
    ▼
TUI 渲染子菜单（复用现有 draw_scrollable_dropdown）
```

### 4.2 选择并加载

```
用户在子菜单中 ↑/↓ 移动
    │
    ▼
Input 更新 submenu_selected
    │
    ▼
用户按 Enter
    │
    ▼
Input 返回 AppEvent::SelectSkill(name)
    │
    ▼
TuiApp::handle_app_event(AppEvent::SelectSkill(name))
    │
    ├── input.close_submenu()
    ├── 调用 skills::load_skill_content(&name)
    │   └── 读取 SKILL.md + REFERENCE.md + examples/*.md
    ├── 若成功 → chat.add_system_message(content)
    └── 若失败 → chat.add_system_message(format!("Failed to load skill '{}': {}", name, err))
    │
    ▼
聊天区显示系统消息，提示 Skill 已加载
```

## 5. 接口设计

### 5.1 Server 端命令注册

在 `src/server/server.rs` 的 `Server::new` 中注册 `/skill`：

```rust
commands.register(
    CommandMeta {
        name: "skill".into(),
        description: "List and load available skills".into(),
        args_hint: None,
    },
    Box::new(SkillCommandHandler),
);
```

`SkillCommandHandler` 在 Server 端不需要实际执行逻辑（因为 TUI 模式下交互完全在客户端处理），但需返回 `CommandOutput { type: Silent }`，避免聊天区重复输出。

### 5.2 TuiApp 处理逻辑

```rust
// src/tui/app.rs
fn handle_execute_slash_command(&mut self, name: &str, _args_hint: &Option<String>) {
    if name == "skill" {
        let registry = crate::skills::get_registry();
        if registry.entries.is_empty() {
            // 发送系统消息提示无可用 Skill
            let _ = self.event_tx.try_send(AppEvent::ShowSystemMessage(
                "No skills available.".into(),
            ));
            return;
        }
        self.input.enter_submenu_mode();
        let items: Vec<(String, String)> = registry
            .entries
            .iter()
            .map(|e| (e.metadata.name.clone(), e.metadata.description.clone()))
            .collect();
        self.input.set_submenu_items(items);
        return;
    }
    // ... 其他命令处理 ...
}
```

```rust
// src/tui/app.rs
async fn handle_app_event(&mut self, event: AppEvent) {
    match event {
        // ... 现有分支 ...
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
        // ...
    }
}
```

### 5.3 Input 组件事件映射

```rust
// src/tui/components/input.rs
// submenu_mode 下方向键已支持，Enter 逻辑需扩展：
KeyCode::Enter => {
    if self.submenu_selected < self.submenu_items.len() {
        let name = self.submenu_items[self.submenu_selected].0.clone();
        self.close_submenu();
        return Some(AppEvent::SelectSkill(name));
    }
}
```

## 6. 错误处理

| 场景 | 处理方式 |
|------|----------|
| 无可用 Skill | 子菜单不打开，聊天区提示 "No skills available." |
| Skill 加载失败（文件被删除等） | 聊天区提示失败原因，不中断会话 |
| 子菜单打开后按 Esc | 直接关闭子菜单，无其他副作用 |
| 子菜单打开后输入其他字符 | 关闭子菜单并回到普通输入模式（复用现有逻辑） |

## 7. 测试策略

### 7.1 单元测试

- **Input 组件**：验证 `/skill` 触发后 `submenu_mode` 为 true，菜单项数量与 `SkillRegistry.entries` 一致
- **AppEvent 路由**：验证 `AppEvent::SelectSkill(name)` 能正确调用 `load_skill_content` 并追加系统消息
- **空 Skill 列表**：验证空列表时发送 `ShowSystemMessage` 而非打开子菜单

### 7.2 集成测试

- **端到端**：在临时目录创建 `.skills/test/SKILL.md`，启动 TUI 后输入 `/skill`，确认菜单中出现该 Skill，选择后聊天区出现系统消息

## 8. 未来扩展

- CLI 模式下支持 `/skill` 纯文本列表输出
- 子菜单中支持按 Tag 过滤 Skill
- 子菜单右侧增加 Skill 内容预览面板（类似方案 B）
