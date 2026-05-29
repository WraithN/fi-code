# Skill 检索目录配置化 + 热重载设计规格书

**日期：** 2026-05-29
**模块：** `fi-code-core`（Config + Skills）
**范围：** 将 Skill 扫描目录从硬编码改为 Config 驱动，并支持热重载

---

## 1. 背景与动机

当前 `crates/core/src/skills/scanner.rs` 中的 `scan_sources()` 函数硬编码了 5 个 Skill 扫描位置（`.skills/`、`.opencode/skills/`、`~/.config/fi-code/skills/` 等），分散在多个 `if` 语句中。用户无法自定义额外的 Skill 目录，且修改后需要重启才能生效。

设计目标：
1. 系统默认路径集中定义，不分散在代码逻辑中
2. 配置文件 `config.json` / `config.jsonc` 支持自定义额外扫描目录
3. 修改配置后自动热重载 Skills，无需重启

---

## 2. Config 结构扩展

### 2.1 新增 `SkillConfig`

**文件：** `crates/core/src/config/models.rs`

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SkillConfig {
    #[serde(default)]
    pub directories: Vec<String>,
}
```

### 2.2 Config 主结构

在 `Config` 中添加：

```rust
pub struct Config {
    // ... existing fields ...
    #[serde(default)]
    pub skills: Option<SkillConfig>,
}
```

### 2.3 配置示例

```json
{
  "skills": {
    "directories": [
      "/home/user/my-skills",
      "/opt/team-skills"
    ]
  }
}
```

---

## 3. Skills Scanner 改造

### 3.1 系统默认路径集中定义

**文件：** `crates/core/src/skills/scanner.rs`

提取 `default_skill_sources()` 函数，将原来的 5 个硬编码来源集中管理：

```rust
fn default_skill_sources(workspace: &Path) -> Vec<(PathBuf, String, SkillSourceType)> {
    let mut sources = Vec::new();

    // 1. 工作区 .skills/
    let ws_skills = workspace.join(".skills");
    if ws_skills.is_dir() { ... }

    // 2. 工作区 .opencode/skills/
    let opencode = workspace.join(".opencode").join("skills");
    if opencode.is_dir() { ... }

    // 3. ~/.config/fi-code/skills/
    // 4. ~/.config/agent/skills/
    // 5. ~/.claude/skills/

    sources
}
```

### 3.2 scan_sources 签名改造

```rust
pub fn scan_sources(
    workspace: &Path,
    extra_dirs: Option<&[String]>,
) -> Vec<(PathBuf, String, SkillSourceType)>
```

先调用 `default_skill_sources(workspace)`，再追加用户自定义目录（scope = `"custom"`）。

### 3.3 scan_and_build_registry 签名改造

```rust
pub fn scan_and_build_registry(
    workspace: &Path,
    extra_dirs: Option<&[String]>,
) -> SkillRegistry
```

---

## 4. SKILL_REGISTRY 改为 RwLock

**文件：** `crates/core/src/skills/mod.rs`

当前使用 `LazyLock<SkillRegistry>`，不可替换。改为 `RwLock<SkillRegistry>`：

```rust
static SKILL_REGISTRY: RwLock<SkillRegistry> = RwLock::new(SkillRegistry::new());

pub fn init_skills(extra_dirs: Option<&[String]>) {
    let workspace = workspace_dir();
    let registry = scanner::scan_and_build_registry(&workspace, extra_dirs);
    let mut lock = SKILL_REGISTRY.write().unwrap();
    *lock = registry;
}

pub fn get_registry() -> SkillRegistry {
    SKILL_REGISTRY.read().unwrap().clone()
}

pub fn rescan_skills(extra_dirs: Option<&[String]>) {
    init_skills(extra_dirs);
}
```

> 需要为 `SkillRegistry` 和 `SkillEntry` 添加 `#[derive(Clone)]`。

---

## 5. 初始化顺序调整

### 5.1 CLI 模式（`crates/cli/src/entry.rs`）

**调整前：**
```rust
set_workspace(workspace.clone());
fi_code_core::skills::init_skills();  // 先初始化（此时无 config）
let config = Arc::new(RwLock::new(Config::load()?));  // 后加载 config
```

**调整后：**
```rust
set_workspace(workspace.clone());

// 1. 先加载 config
let config = Arc::new(RwLock::new(Config::load()?));
let _watcher = spawn_watcher(Arc::clone(&config))?;

// 2. 用 config 中的自定义目录初始化 skills
{
    let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
    let extra = cfg.skills.as_ref().map(|s| s.directories.as_slice());
    fi_code_core::skills::init_skills(extra);
}
```

### 5.2 TUI / Server 模式

TUI 和 Server 独立模式也需要在启动时传入 config 到 `init_skills`。但 TUI/Server 的 skills 目前是懒加载的（通过 `LazyLock`），改为 `RwLock` 后，`init_skills` 需要在首次访问前被调用。

**处理方式：** 在 TUI 和 Server 的启动代码中，加载 config 后立即调用 `init_skills`。

---

## 6. 热重载

**文件：** `crates/core/src/config/config.rs`

在 `try_reload_config` 中，加载新 config 后比较 `skills.directories`：

```rust
let old_extra: Option<Vec<String>> = cfg.skills.as_ref().map(|s| s.directories.clone());
*cfg = new_config;
let new_extra: Option<Vec<String>> = cfg.skills.as_ref().map(|s| s.directories.clone());

if old_extra != new_extra {
    let extra = new_extra.as_deref();
    crate::skills::rescan_skills(extra);
    log_info!("Skills 已根据新配置重新扫描");
}
```

---

## 7. 相关文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/core/src/config/models.rs` | 修改 | 添加 `SkillConfig` 和 `Config.skills` |
| `crates/core/src/config/config.rs` | 修改 | `try_reload_config` 增加 skills 变化检测和 rescan |
| `crates/core/src/skills/scanner.rs` | 修改 | 提取 `default_skill_sources`，改造 `scan_sources` 和 `scan_and_build_registry` 签名 |
| `crates/core/src/skills/skill_type.rs` | 修改 | 为 `SkillRegistry`、`SkillEntry` 添加 `Clone` derive |
| `crates/core/src/skills/mod.rs` | 修改 | `LazyLock` → `RwLock`，改造 `init_skills` / `get_registry` / `rescan_skills` |
| `crates/cli/src/entry.rs` | 修改 | 调整初始化顺序：先 Config::load，后 init_skills |
| `crates/tui/src/lib.rs` | 修改 | 加载 config 后调用 `init_skills` |
| `crates/server/src/main.rs` | 修改 | 加载 config 后调用 `init_skills` |

---

## 8. 风险评估

| 风险 | 缓解措施 |
|------|---------|
| `get_registry()` 改为 clone 可能影响性能 | `SkillRegistry` 通常只有几十条 entries，clone 开销可忽略 |
| RwLock 在并发场景下的死锁 | `get_registry()` 读锁后立即 clone 释放，`load_skill_content` 同理，不会长期持有 |
| TUI/Server 未调用 `init_skills` 导致首次访问 panic | 在 TUI/Server 启动代码中显式调用 `init_skills` |
| 自定义目录不存在或不可读 | `scan_sources` 中用 `path.is_dir()` 过滤，不存在的目录静默跳过 |
| 热重载时用户正在使用 skill | `rescan_skills` 会替换 registry，但 `get_registry()` 每次读取都 clone，不会读到半旧半新的状态 |
