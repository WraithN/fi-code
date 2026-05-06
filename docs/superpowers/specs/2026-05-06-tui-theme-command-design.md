# /theme 命令与 Server 端主题管理设计文档

> 日期：2026-05-06
> 状态：已评审，待实现

## 1. 需求概述

新增 `/theme` 斜杠命令，用户选择后上拉菜单切换为可用主题列表。支持方向键/鼠标滚轮选择，选中时**实时预览**主题变化，回车或鼠标左键**确认生效**，Esc **取消并恢复**原来主题。

主题配置由 Server 端统一管理，TUI 通过 HTTP 获取。

## 2. 架构设计

### 2.1 主题数据流

```
Server (AppState)
  │ 持有 Vec<ThemePreset>
  │
  ├── GET /api/themes ──► TUI (启动时缓存主题列表)
  │
  └── POST /api/commands/theme/execute ──► 切换 current_theme

TUI
  ├── 从 ThemePreset 构建 Theme (Theme::from_preset)
  ├── /theme 子菜单显示 ThemePreset 列表
  ├── 方向键移动 → PreviewTheme → 临时应用主题
  └── 回车确认 → HTTP 执行 /theme → 主题固定
```

## 3. 核心数据结构

### 3.1 ThemePreset（共享模块）

```rust
// src/theme/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePreset {
    pub name: String,
    pub description: String,
    pub bg_base: u32,
    pub bg_surface: u32,
    pub bg_overlay: u32,
    pub border: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_placeholder: u32,
    pub brand: u32,
    pub user: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
    pub selection_bg: u32,
    pub selection_fg: u32,
    pub accent_hover: u32,
}

impl ThemePreset {
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self { name: "deep_ocean".into(), description: "Deep Ocean Dark".into(), ... },
            Self { name: "github_dark".into(), description: "GitHub Dark".into(), ... },
        ]
    }
}
```

### 3.2 Theme::from_preset

```rust
// src/tui/theme.rs
impl Theme {
    pub fn from_preset(preset: &crate::theme::ThemePreset) -> Self {
        Self {
            bg_base: Color::from_u32(preset.bg_base),
            // ... 映射所有字段
        }
    }
}
```

## 4. Server 端变更

### 4.1 AppState

```rust
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
    pub themes: Vec<crate::theme::ThemePreset>,
    pub current_theme: Arc<RwLock<String>>,
}
```

### 4.2 HTTP 端点

- `GET /api/themes` → `ApiResponse<Vec<ThemePreset>>`

### 4.3 /theme 命令注册

```rust
commands.register(
    CommandMeta {
        name: "theme".into(),
        description: "Switch theme".into(),
        args_hint: Some("[theme_name]".into()),
    },
    Box::new(ThemeCommandHandler { current_theme: current_theme.clone() }),
);
```

## 5. TUI 交互流程

### 5.1 子菜单模式

`Input` 组件新增 `submenu_mode` 状态：
- `None`：正常命令列表
- `ThemeList`：主题选择子菜单

### 5.2 事件流

```
用户输入 /
  → Input 显示命令列表（含 /theme）
  → 用户选择 /theme
  → Input 进入 SubmenuMode::ThemeList
  → Input 显示主题列表（从 TuiApp.themes 获取）

用户在主题列表中：
  → 方向键移动
    → AppEvent::PreviewTheme(index)
    → TuiApp 临时切换 theme = themes[index]
  → 回车/点击
    → AppEvent::SelectTheme(index)
    → TuiApp 发送 HTTP /api/commands/theme/execute
    → Server 更新 current_theme
    → Input 退出子菜单模式，清空输入框
  → Esc
    → TuiApp 恢复 original_theme
    → Input 回到命令列表（SubmenuMode::None）
```

### 5.3 TuiApp 主题状态

```rust
pub struct TuiApp {
    // ... 现有字段 ...
    themes: Vec<Arc<Theme>>,           // 从 ThemePreset 构建的 Theme 列表
    theme_presets: Vec<ThemePreset>,   // 原始预设（用于子菜单显示）
    original_theme: Option<Arc<Theme>>, // 预览前保存的原主题
    original_theme_index: usize,        // 预览前的原主题索引
}
```

## 6. 边界情况

| 场景 | 处理 |
|------|------|
| Server 未启动，无法获取主题列表 | TUI 使用本地默认主题列表（deep_ocean, github_dark） |
| 输入 `/theme unknown` | Server 返回错误，TUI 显示错误消息 |
| 预览中按 Esc | 恢复 original_theme，回到命令列表 |
| 预览中输入其他字符 | 关闭菜单，恢复 original_theme，按正常文本处理 |

## 7. 文件变更清单

| 文件 | 变更 |
|------|------|
| `src/theme/mod.rs` | 新建：ThemePreset 共享结构 |
| `src/tui/theme.rs` | 增加 `from_preset` 方法 |
| `src/server/server.rs` | AppState 增加 themes/current_theme；新增 /api/themes 路由；注册 /theme 命令 |
| `src/tui/client.rs` | 新增 `list_themes()` |
| `src/tui/app.rs` | 缓存主题列表；处理 PreviewTheme/SelectTheme；保存/恢复主题 |
| `src/tui/components/input.rs` | 支持子菜单模式；主题列表渲染 |
| `src/tui/event.rs` | 新增 `PreviewTheme(usize)` |
