# Agent Skills 加载能力实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 shun-code 实现 Agent Skills 扫描、注册、按需加载能力，支持通过 `use_skill` 工具将 skill 内容注入当前对话回合。

**Architecture:** 新增 `src/skills/` 模块负责 Skill 的扫描（scanner）、注册表管理（registry）和内容加载（loader）。SkillRegistry 通过 `LazyLock` 全局初始化，PromptBuilder 从中读取摘要注入 system prompt，`use_skill` 工具通过全局 registry 查找并返回 skill 完整内容作为 ToolResult。

**Tech Stack:** Rust, serde_yaml, std::os::unix::fs::symlink, tokio::task::spawn_blocking

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `src/skills/skill_type.rs` | Skill 核心数据结构（SkillMetadata, SkillSourceType, SkillEntry, SkillRegistry） |
| `src/skills/loader.rs` | 解析 SKILL.md（分离 YAML front matter 与 Markdown 正文），读取 REFERENCE.md 和 examples |
| `src/skills/scanner.rs` | 遍历来源目录、识别有效 skill、创建软链、构建 Registry |
| `src/skills/registry.rs` | Registry 持久化（JSON 读写）、按 name/id 查询、失效清理 |
| `src/skills/mod.rs` | 模块聚合、全局 `LazyLock` 初始化、公共导出 |
| `src/agent/prompt.rs` | 修改：追加 Available Skills 摘要段 |
| `src/agent/agent.rs` | 修改：`run_one_turn` 和 `agent_loop` 传递 `&SkillRegistry` |
| `src/tools/mod.rs` | 修改：注册 `use_skill` 工具 |
| `src/main.rs` | 修改：启动时触发 SkillRegistry 初始化 |
| `Cargo.toml` | 新增 `serde_yaml` 依赖 |

---

## Task 1: 添加 serde_yaml 依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 在 Cargo.toml 中添加 serde_yaml**

```toml
[dependencies]
# ... 保留现有依赖 ...
serde_yaml = "0.9"
```

- [ ] **Step 2: 运行 cargo check 确认依赖可用**

```bash
cargo check
```

Expected: 编译成功，无错误。

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "deps: add serde_yaml for SKILL.md front matter parsing"
```

---

## Task 2: 创建 Skill 核心类型

**Files:**
- Create: `src/skills/skill_type.rs`

- [ ] **Step 1: 编写 skill_type.rs**

```rust
use std::path::PathBuf;

/// Skill 元数据（从 SKILL.md 的 YAML front matter 解析）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Skill 来源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SkillSourceType {
    Project,
    Global,
    Agent,
    Claude,
}

/// Registry 中的单个 Skill 条目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub scope: String,
    #[serde(rename = "type")]
    pub source_type: SkillSourceType,
    pub symlink_path: PathBuf,
    pub target_path: PathBuf,
    pub metadata: SkillMetadata,
}

/// 完整 Registry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRegistry {
    pub version: String,
    pub entries: Vec<SkillEntry>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            entries: Vec::new(),
        }
    }

    /// 按 id 精确查找
    pub fn find_by_id(&self, id: &str) -> Option<&SkillEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// 按 name 查找，返回最后一个匹配（优先级最高的，因为后加载覆盖前者）
    pub fn find_by_name(&self, name: &str) -> Option<&SkillEntry> {
        self.entries.iter().rev().find(|e| e.metadata.name == name)
    }

    /// 按 name 或 id 查找
    pub fn find(&self, name_or_id: &str) -> Option<&SkillEntry> {
        self.find_by_id(name_or_id)
            .or_else(|| self.find_by_name(name_or_id))
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/skills/skill_type.rs
git commit -m "feat(skills): add core skill data types"
```

---

## Task 3: 创建 Skill 内容加载器

**Files:**
- Create: `src/skills/loader.rs`

- [ ] **Step 1: 编写 loader.rs**

```rust
use std::fs;
use std::path::Path;

/// 从 SKILL.md 中分离 YAML front matter 和 Markdown 正文
///
/// 格式要求：文件以 `---` 开头，紧接着是 YAML，再以 `---` 结束，之后是 Markdown 正文。
/// 返回 (front_matter_yaml_string, markdown_body)
pub fn parse_skill_md(content: &str) -> Result<(String, String), String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err("SKILL.md must start with YAML front matter delimiter '---'".to_string());
    }

    // 找到第二个 ---
    let after_first = &trimmed[3..];
    if let Some(end_idx) = after_first.find("\n---") {
        let yaml_part = after_first[..end_idx].trim();
        let body_part = after_first[end_idx + 4..].trim_start();
        Ok((yaml_part.to_string(), body_part.to_string()))
    } else {
        Err("YAML front matter not properly closed with '---'".to_string())
    }
}

/// 读取目录中的 SKILL.md，解析元数据和正文
pub fn load_skill_metadata_and_body(dir: &Path) -> Result<(crate::skills::SkillMetadata, String), String> {
    let skill_md_path = dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    let (yaml_str, body) = parse_skill_md(&content)?;
    let metadata: crate::skills::SkillMetadata = serde_yaml::from_str(&yaml_str)
        .map_err(|e| format!("Failed to parse YAML front matter: {}", e))?;

    if metadata.name.trim().is_empty() {
        return Err("SKILL.md missing 'name' field in front matter".to_string());
    }

    Ok((metadata, body))
}

/// 读取 skill 目录的完整内容（SKILL.md 正文 + REFERENCE.md + examples/*.md）
pub fn load_skill_full_content(dir: &Path) -> Result<String, String> {
    let (_, body) = load_skill_metadata_and_body(dir)?;

    let mut parts = Vec::new();
    parts.push(body);

    // 读取 REFERENCE.md
    let reference_path = dir.join("REFERENCE.md");
    if reference_path.exists() {
        if let Ok(content) = fs::read_to_string(&reference_path) {
            if !content.trim().is_empty() {
                parts.push(format!("\n\n--- Reference ---\n{}", content));
            }
        }
    }

    // 读取 examples/*.md
    let examples_dir = dir.join("examples");
    if examples_dir.exists() && examples_dir.is_dir() {
        let mut example_files: Vec<_> = fs::read_dir(&examples_dir)
            .map_err(|e| format!("Failed to read examples dir: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                path.is_file() && path.extension().map(|ext| ext == "md").unwrap_or(false)
            })
            .map(|e| e.path())
            .collect();

        example_files.sort();

        if !example_files.is_empty() {
            let mut examples_content = Vec::new();
            for path in example_files {
                if let Ok(content) = fs::read_to_string(&path) {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    examples_content.push(format!("\n### {}\n{}", file_name, content));
                }
            }
            if !examples_content.is_empty() {
                parts.push(format!("\n\n--- Examples ---{}", examples_content.join("")));
            }
        }
    }

    Ok(parts.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_skill_md_valid() {
        let content = "---\nname: commit\ndescription: test desc\n---\n\n## Steps\n1. Do something\n";
        let (yaml, body) = parse_skill_md(content).unwrap();
        assert!(yaml.contains("name: commit"));
        assert!(body.contains("## Steps"));
    }

    #[test]
    fn test_parse_skill_md_missing_delimiter() {
        let content = "name: commit\n\n## Steps\n";
        assert!(parse_skill_md(content).is_err());
    }

    #[test]
    fn test_parse_skill_md_no_closing() {
        let content = "---\nname: commit\n\n## Steps\n";
        assert!(parse_skill_md(content).is_err());
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test skills::loader::tests
```

Expected: 3 个测试全部 PASS。

- [ ] **Step 3: Commit**

```bash
git add src/skills/loader.rs
git commit -m "feat(skills): add skill content loader with YAML front matter parser"
```

---

## Task 4: 创建 Registry 持久化与查询

**Files:**
- Create: `src/skills/registry.rs`

- [ ] **Step 1: 编写 registry.rs**

```rust
use crate::skills::skill_type::{SkillEntry, SkillRegistry};
use std::path::PathBuf;

/// Registry 持久化路径
pub fn registry_path() -> PathBuf {
    let config_dir = directories::ProjectDirs::from("", "", "shun-code")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config/shun-code"));
    config_dir.join("registry-skills.json")
}

/// 缓存目录路径（软链存放处）
pub fn cache_skills_dir() -> PathBuf {
    let cache_dir = directories::ProjectDirs::from("", "", "shun-code")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".cache/shun-code"));
    cache_dir.join("skills")
}

/// 从磁盘加载 Registry，若文件不存在则返回空 Registry
pub fn load_registry() -> SkillRegistry {
    let path = registry_path();
    if !path.exists() {
        return SkillRegistry::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(registry) => registry,
            Err(e) => {
                eprintln!("Warning: failed to parse registry-skills.json: {}. Starting fresh.", e);
                SkillRegistry::new()
            }
        },
        Err(e) => {
            eprintln!("Warning: failed to read registry-skills.json: {}. Starting fresh.", e);
            SkillRegistry::new()
        }
    }
}

/// 将 Registry 持久化到磁盘
pub fn save_registry(registry: &SkillRegistry) -> Result<(), String> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize registry: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write registry: {}", e))?;
    Ok(())
}

/// 清理失效的软链和 registry 条目
pub fn cleanup_stale_entries(registry: &mut SkillRegistry) {
    let before = registry.entries.len();
    registry.entries.retain(|entry| {
        // 检查软链是否仍然有效
        if let Ok(target) = std::fs::read_link(&entry.symlink_path) {
            if target.exists() {
                return true;
            }
        }
        // 删除失效软链
        let _ = std::fs::remove_file(&entry.symlink_path);
        false
    });
    let removed = before - registry.entries.len();
    if removed > 0 {
        eprintln!("Cleaned up {} stale skill entries", removed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::skill_type::{SkillMetadata, SkillSourceType};

    #[test]
    fn test_save_and_load_registry() {
        let mut registry = SkillRegistry::new();
        registry.entries.push(SkillEntry {
            id: "test-skill".to_string(),
            scope: "test".to_string(),
            source_type: SkillSourceType::Global,
            symlink_path: PathBuf::from("/tmp/test-skill"),
            target_path: PathBuf::from("/tmp/original"),
            metadata: SkillMetadata {
                name: "test".to_string(),
                description: "test desc".to_string(),
                tags: vec![],
            },
        });

        // 使用临时路径测试（但这里用真实路径，测试后清理）
        let test_path = std::env::temp_dir().join("shun-code-test-registry.json");
        // 临时替换 registry_path 行为不可行，改为测试序列化/反序列化
        let json = serde_json::to_string_pretty(&registry).unwrap();
        let loaded: SkillRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].id, "test-skill");
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test skills::registry::tests
```

Expected: 1 个测试 PASS。

- [ ] **Step 3: Commit**

```bash
git add src/skills/registry.rs
git commit -m "feat(skills): add registry persistence and cleanup"
```

---

## Task 5: 创建 Skill 扫描器

**Files:**
- Create: `src/skills/scanner.rs`

- [ ] **Step 1: 编写 scanner.rs**

```rust
use crate::skills::loader::load_skill_metadata_and_body;
use crate::skills::registry::{cache_skills_dir, cleanup_stale_entries, save_registry};
use crate::skills::skill_type::{SkillEntry, SkillMetadata, SkillRegistry, SkillSourceType};
use std::path::{Path, PathBuf};

/// 定义扫描来源
fn scan_sources(workspace: &Path) -> Vec<(PathBuf, String, SkillSourceType)> {
    let mut sources = Vec::new();

    // 1. 项目级 .skills/
    let project_skills = workspace.join(".skills");
    if project_skills.exists() && project_skills.is_dir() {
        let scope = workspace
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        sources.push((project_skills, scope, SkillSourceType::Project));
    }

    // 2. ~/.config/shun-code/skills/
    if let Some(dirs) = directories::ProjectDirs::from("", "", "shun-code") {
        let global_skills = dirs.config_dir().join("skills");
        if global_skills.exists() && global_skills.is_dir() {
            sources.push((global_skills, "shun-code".to_string(), SkillSourceType::Global));
        }
    }

    // 3. ~/.config/agent/skills/
    if let Some(home) = dirs::home_dir() {
        let agent_skills = home.join(".config/agent/skills");
        if agent_skills.exists() && agent_skills.is_dir() {
            sources.push((agent_skills, "agent".to_string(), SkillSourceType::Agent));
        }
    }

    // 4. ~/.claude/skills/
    if let Some(home) = dirs::home_dir() {
        let claude_skills = home.join(".claude/skills");
        if claude_skills.exists() && claude_skills.is_dir() {
            sources.push((claude_skills, "claude".to_string(), SkillSourceType::Claude));
        }
    }

    sources
}

/// 扫描单个来源目录下的所有 skill
fn scan_source_dir(
    source_dir: &Path,
    scope: &str,
    source_type: SkillSourceType,
    cache_dir: &Path,
    registry: &mut SkillRegistry,
) {
    let entries = match std::fs::read_dir(source_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Warning: failed to read skills dir {:?}: {}", source_dir, e);
            return;
        }
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        match load_skill_metadata_and_body(&path) {
            Ok((metadata, _)) => {
                let id = format!("{}-{}", scope, metadata.name);
                let symlink_path = cache_dir.join(&id);

                // 创建或更新软链
                if symlink_path.exists() {
                    let _ = std::fs::remove_file(&symlink_path);
                }
                if let Err(e) = create_symlink(&path, &symlink_path) {
                    eprintln!("Warning: failed to create symlink for skill '{}': {}", id, e);
                    continue;
                }

                // 移除同名 skill（低优先级的先加载的）
                registry.entries.retain(|e| e.id != id);

                registry.entries.push(SkillEntry {
                    id,
                    scope: scope.to_string(),
                    source_type,
                    symlink_path,
                    target_path: path,
                    metadata,
                });
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse skill at {:?}: {}",
                    path, e
                );
            }
        }
    }
}

/// 创建软链（跨平台包装）
#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// 主入口：扫描所有来源并构建 Registry
pub fn scan_and_build_registry(workspace: &Path) -> SkillRegistry {
    let cache_dir = cache_skills_dir();
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        eprintln!("Warning: failed to create skills cache dir: {}", e);
    }

    let mut registry = SkillRegistry::new();

    let sources = scan_sources(workspace);
    for (source_dir, scope, source_type) in sources {
        scan_source_dir(&source_dir, &scope, source_type, &cache_dir, &mut registry);
    }

    // 清理失效条目
    cleanup_stale_entries(&mut registry);

    // 持久化
    if let Err(e) = save_registry(&registry) {
        eprintln!("Warning: failed to save registry: {}", e);
    }

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_skill(dir: &Path, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let mut file = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        write!(
            file,
            "---\nname: {}\ndescription: {}\n---\n\n{}",
            name, description, body
        )
        .unwrap();
    }

    #[test]
    fn test_scan_source_dir_valid_skill() {
        let tmp = std::env::temp_dir().join("shun-code-test-scan-1");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        create_test_skill(&tmp, "commit", "Create commits", "## Steps\n1. Stage");

        let cache = std::env::temp_dir().join("shun-code-test-cache-1");
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();

        let mut registry = SkillRegistry::new();
        scan_source_dir(&tmp, "test", SkillSourceType::Global, &cache, &mut registry);

        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].id, "test-commit");
        assert_eq!(registry.entries[0].metadata.name, "commit");

        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn test_scan_source_dir_skips_invalid() {
        let tmp = std::env::temp_dir().join("shun-code-test-scan-2");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // 有效 skill
        create_test_skill(&tmp, "commit", "Create commits", "## Steps");
        // 无效目录（无 SKILL.md）
        std::fs::create_dir_all(tmp.join("invalid")).unwrap();
        std::fs::write(tmp.join("invalid/README.md"), "no front matter").unwrap();

        let cache = std::env::temp_dir().join("shun-code-test-cache-2");
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();

        let mut registry = SkillRegistry::new();
        scan_source_dir(&tmp, "test", SkillSourceType::Global, &cache, &mut registry);

        assert_eq!(registry.entries.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn test_scan_source_dir_overrides_same_name() {
        let tmp1 = std::env::temp_dir().join("shun-code-test-scan-3a");
        let tmp2 = std::env::temp_dir().join("shun-code-test-scan-3b");
        let _ = std::fs::remove_dir_all(&tmp1);
        let _ = std::fs::remove_dir_all(&tmp2);
        std::fs::create_dir_all(&tmp1).unwrap();
        std::fs::create_dir_all(&tmp2).unwrap();

        create_test_skill(&tmp1, "commit", "First desc", "## A");
        create_test_skill(&tmp2, "commit", "Second desc", "## B");

        let cache = std::env::temp_dir().join("shun-code-test-cache-3");
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();

        let mut registry = SkillRegistry::new();
        // 先扫描 tmp1
        scan_source_dir(&tmp1, "first", SkillSourceType::Global, &cache, &mut registry);
        assert_eq!(registry.entries[0].metadata.description, "First desc");

        // 再扫描 tmp2（同名，应该覆盖）
        scan_source_dir(&tmp2, "second", SkillSourceType::Global, &cache, &mut registry);
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].scope, "second");
        assert_eq!(registry.entries[0].metadata.description, "Second desc");

        let _ = std::fs::remove_dir_all(&tmp1);
        let _ = std::fs::remove_dir_all(&tmp2);
        let _ = std::fs::remove_dir_all(&cache);
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test skills::scanner::tests
```

Expected: 3 个测试全部 PASS。

- [ ] **Step 3: Commit**

```bash
git add src/skills/scanner.rs
git commit -m "feat(skills): add skill scanner with source prioritization and symlink management"
```

---

## Task 6: 创建 Skills 模块聚合

**Files:**
- Create: `src/skills/mod.rs`

- [ ] **Step 1: 编写 mod.rs**

```rust
pub mod loader;
pub mod registry;
pub mod scanner;
pub mod skill_type;

pub use skill_type::{SkillEntry, SkillMetadata, SkillRegistry, SkillSourceType};

use std::sync::LazyLock;

// =============================================================================
// 全局 SkillRegistry：LazyLock 实现懒加载
// =============================================================================
// 首次访问时触发扫描，后续直接返回已构建的 Registry。
// 由于扫描需要 workspace_dir()（已由 main.rs 的 set_workspace 设置），
// 因此必须在 set_workspace 之后才能访问 SKILL_REGISTRY。

static SKILL_REGISTRY: LazyLock<SkillRegistry> = LazyLock::new(|| {
    let workspace = crate::utils::workspace::workspace_dir();
    scanner::scan_and_build_registry(&workspace)
});

/// 显式触发 SkillRegistry 初始化（在 main.rs 启动时调用）
pub fn init_skills() {
    let _ = std::sync::LazyLock::force(&SKILL_REGISTRY);
}

/// 获取全局 SkillRegistry 的引用
pub fn get_registry() -> &'static SkillRegistry {
    &SKILL_REGISTRY
}

/// 通过 name 或 id 查找 skill 并返回完整内容（供 use_skill 工具使用）
pub fn load_skill_content(name_or_id: &str) -> Result<String, String> {
    let registry = get_registry();
    let entry = registry
        .find(name_or_id)
        .ok_or_else(|| format!("Skill '{}' not found in registry.", name_or_id))?;

    // 读取软链指向的目录内容
    let target = std::fs::read_link(&entry.symlink_path)
        .map_err(|e| format!("Failed to resolve skill symlink: {}", e))?;

    if !target.exists() {
        return Err(format!(
            "Skill '{}' target directory no longer exists.",
            name_or_id
        ));
    }

    let content = loader::load_skill_full_content(&target)
        .map_err(|e| format!("Failed to read skill content: {}", e))?;

    Ok(format!(
        "<skill name=\"{}\" id=\"{}\">\n{}\n</skill>",
        entry.metadata.name, entry.id, content
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_skill_content_not_found() {
        // 不初始化真实的 registry，直接测试返回值
        // 由于全局 LazyLock 已经绑定，这里没法 mock，
        // 所以只做简单的 error 路径验证：假设 registry 中不存在 "nonexistent"
        let result = load_skill_content("nonexistent-skill-xyz-999");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/skills/mod.rs
git commit -m "feat(skills): add skills module with global LazyLock registry"
```

---

## Task 7: 修改 PromptBuilder 注入 Available Skills

**Files:**
- Modify: `src/agent/prompt.rs`

- [ ] **Step 1: 修改 PromptBuilder::build 方法签名和实现**

在 `src/agent/prompt.rs` 中，将 `build` 方法修改为接收 `&SkillRegistry`：

```rust
use crate::skills::SkillRegistry;

// ... 现有 PROMPT_TEMPLATE 不变 ...

impl PromptBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, tools_schema: &serde_json::Value, registry: &SkillRegistry) -> String {
        let tools_str = serde_json::to_string_pretty(tools_schema).unwrap_or_default();
        let mut prompt = PROMPT_TEMPLATE.replace("{tools_schema}", &tools_str);

        if !registry.entries.is_empty() {
            prompt.push_str("\n\n## Available Skills\n");
            prompt.push_str("You can load any of the following skills on-demand by calling the `use_skill` tool:\n\n");
            for entry in &registry.entries {
                prompt.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    entry.metadata.name, entry.scope, entry.metadata.description
                ));
            }
        }

        prompt
    }
}
```

- [ ] **Step 2: 修改测试以适应新签名**

更新 `src/agent/prompt.rs` 中所有调用 `builder.build(...)` 的测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder_includes_schema() {
        let builder = PromptBuilder::new();
        let schema = serde_json::json!([{"name": "bash", "description": "Run shell commands"}]);
        let registry = SkillRegistry::new();
        let prompt = builder.build(&schema, &registry);
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("\"name\": \"bash\""));
        assert!(prompt.contains("Run shell commands"));
        assert!(prompt.contains("Rules:"));
    }

    #[test]
    fn test_prompt_builder_empty_schema() {
        let builder = PromptBuilder::default();
        let registry = SkillRegistry::new();
        let prompt = builder.build(&serde_json::json!([]), &registry);
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("[]"));
    }

    #[test]
    fn test_prompt_builder_with_skills() {
        use crate::skills::skill_type::{SkillEntry, SkillMetadata, SkillSourceType};

        let builder = PromptBuilder::new();
        let schema = serde_json::json!([]);
        let mut registry = SkillRegistry::new();
        registry.entries.push(SkillEntry {
            id: "claude-commit".to_string(),
            scope: "claude".to_string(),
            source_type: SkillSourceType::Claude,
            symlink_path: std::path::PathBuf::from("/tmp/test"),
            target_path: std::path::PathBuf::from("/tmp/test2"),
            metadata: SkillMetadata {
                name: "commit".to_string(),
                description: "Create conventional commits".to_string(),
                tags: vec![],
            },
        });

        let prompt = builder.build(&schema, &registry);
        assert!(prompt.contains("## Available Skills"));
        assert!(prompt.contains("`commit` (claude): Create conventional commits"));
    }

    #[test]
    fn test_prompt_builder_without_skills() {
        let builder = PromptBuilder::new();
        let schema = serde_json::json!([]);
        let registry = SkillRegistry::new();
        let prompt = builder.build(&schema, &registry);
        assert!(!prompt.contains("## Available Skills"));
    }

    #[test]
    fn test_prompt_builder_with_real_tools() {
        use crate::tools::tool_schema;

        let builder = PromptBuilder::new();
        let schema = tool_schema();
        let registry = SkillRegistry::new();
        let prompt = builder.build(&schema, &registry);

        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("\"name\": \"bash\""));
        assert!(prompt.contains("\"name\": \"read\""));
        assert!(prompt.contains("Run a shell command"));
    }
}
```

- [ ] **Step 3: 运行测试**

```bash
cargo test agent::prompt::tests
```

Expected: 5 个测试全部 PASS。

- [ ] **Step 4: Commit**

```bash
git add src/agent/prompt.rs
git commit -m "feat(prompt): inject Available Skills summary into system prompt"
```

---

## Task 8: 修改 agent_loop 传递 SkillRegistry

**Files:**
- Modify: `src/agent/agent.rs`
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: 修改 agent.rs**

```rust
use crate::skills::SkillRegistry;
use crate::skills::get_registry;
// ... 保留现有 imports ...

pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    let registry = get_registry();
    let system_prompt = PromptBuilder::new().build(&tool_schema(), registry);
    // ... 其余逻辑不变 ...
}

pub async fn agent_loop<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<()> {
    while run_one_turn(client, state).await? {}
    Ok(())
}
```

- [ ] **Step 2: 确认编译通过**

```bash
cargo check
```

Expected: 编译成功。

- [ ] **Step 3: Commit**

```bash
git add src/agent/agent.rs src/agent/mod.rs
git commit -m "feat(agent): pass SkillRegistry to prompt builder in run_one_turn"
```

---

## Task 9: 注册 use_skill 工具

**Files:**
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: 在 mod.rs 中新增 UseSkillHandler**

在现有 handler 定义之后、REGISTRY 之前，添加：

```rust
// =============================================================================
// UseSkillHandler：按需加载 skill 内容
// =============================================================================

#[derive(Debug)]
struct UseSkillHandler;

impl ToolHandler for UseSkillHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        let name = match &params[..] {
            [ToolParameter::Json(v)] => {
                v.get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            [ToolParameter::String(n)] => n.clone(),
            _ => "".to_string(),
        };

        if name.is_empty() {
            return Err("Missing name parameter".to_string());
        }

        crate::skills::load_skill_content(&name)
    }
}
```

- [ ] **Step 2: 在 REGISTRY 初始化中注册 use_skill**

在 `static REGISTRY: LazyLock<ToolsRegistry> = LazyLock::new(|| { ... })` 中，在 grep 注册之后添加：

```rust
    registry
        .register(
            "use_skill",
            "Load a skill by name or ID to inject its instructions into the conversation.",
            r#"{"type":"object","properties":{"name":{"type":"string","description":"Skill name or full ID (e.g., 'commit' or 'claude-commit')"}},"required":["name"]}"#,
            Box::new(UseSkillHandler),
        )
        .expect("register use_skill tool failed");
```

- [ ] **Step 3: 更新测试**

在 `tests` 模块中：

```rust
    #[test]
    fn test_list_tools_includes_use_skill() {
        let list = REGISTRY.list_tools().unwrap();
        assert!(list.contains("use_skill"), "registry should contain use_skill tool");
    }

    #[test]
    fn test_tool_call_use_skill_not_found() {
        let mut input = std::collections::HashMap::new();
        input.insert("name".to_string(), serde_json::json!("nonexistent_skill_xyz"));
        let result = tool_call("use_skill", &input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
```

- [ ] **Step 4: 运行测试**

```bash
cargo test tools::tests
```

Expected: 所有测试 PASS（原有测试 + 新增 2 个）。

- [ ] **Step 5: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat(tools): add use_skill tool for on-demand skill loading"
```

---

## Task 10: 修改 main.rs 启动时初始化 Skills

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: 在 main.rs 中添加 skills 模块声明和初始化**

在现有 `mod` 声明列表末尾添加：

```rust
mod skills;
```

在 `set_workspace(workspace.clone());` 之后、任何可能用到 skills 的操作之前，添加：

```rust
    // 初始化 Skill Registry（必须在 set_workspace 之后）
    skills::init_skills();
    log_info!("skills initialized | count={}", skills::get_registry().entries.len());
```

- [ ] **Step 2: 确认编译通过**

```bash
cargo check
```

Expected: 编译成功。

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): initialize SkillRegistry at startup after workspace setup"
```

---

## Task 11: 运行完整测试套件

- [ ] **Step 1: 运行全部测试**

```bash
cargo test
```

Expected: 所有测试 PASS。

- [ ] **Step 2: 运行 cargo fmt**

```bash
cargo fmt
```

- [ ] **Step 3: 运行 cargo clippy**

```bash
cargo clippy -- -D warnings
```

Expected: 无警告（若存在与本次修改无关的既有警告，需确认未引入新警告）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "style: cargo fmt and clippy pass"
```

---

## Task 12: 端到端验证（可选手动测试）

- [ ] **Step 1: 创建一个测试 skill**

```bash
mkdir -p ~/.config/shun-code/skills/commit
cat > ~/.config/shun-code/skills/commit/SKILL.md << 'EOF'
---
name: commit
description: Create conventional commits with proper formatting and emoji prefixes
---

## Context
Current git status: !`git status`

## Steps
1. Analyze staged changes
2. Choose type (feat/fix/docs)
3. Format: `emoji type(scope): description`
4. Execute commit
EOF
```

- [ ] **Step 2: 编译并运行 shun-code**

```bash
cargo build --release
./target/release/shun-code -i
```

Expected: 启动日志中显示 `skills initialized | count=1`。

- [ ] **Step 3: 在交互模式中验证**

输入查询让 LLM 调用 `use_skill` 工具（需要配置有效的 LLM API key）。

- [ ] **Step 4: 清理测试数据**

```bash
rm -rf ~/.config/shun-code/skills/commit
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - [x] Skill 扫描来源与优先级 → Task 5 (scanner.rs)
   - [x] Skill 识别与 YAML 解析 → Task 3 (loader.rs)
   - [x] Registry 持久化格式 → Task 4 (registry.rs)
   - [x] System Prompt 集成 → Task 7 (prompt.rs)
   - [x] `use_skill` 工具 → Task 9 (tools/mod.rs)
   - [x] Skill 内容注入 messages → Task 6 (mod.rs `load_skill_content`)
   - [x] 启动时序 → Task 10 (main.rs)
   - [x] 项目级 skill 隔离 → Task 5 (scan_sources 中 workspace 判断)

2. **Placeholder scan:**
   - [x] 无 TBD / TODO
   - [x] 所有测试包含具体断言
   - [x] 所有任务包含完整代码

3. **Type consistency:**
   - [x] `SkillRegistry` 定义在 skill_type.rs，在 prompt.rs、agent.rs、tools/mod.rs 中一致使用
   - [x] `load_skill_content` 在 mod.rs 中定义，在 tools/mod.rs 的 UseSkillHandler 中调用
   - [x] `get_registry()` 返回 `&'static SkillRegistry`，与全局 LazyLock 类型一致
