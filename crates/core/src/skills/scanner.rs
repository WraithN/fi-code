// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::log_info;
use crate::skills::{
    loader::load_skill_metadata_and_body,
    registry::{cache_skills_dir, cleanup_stale_entries, save_registry},
    SkillEntry, SkillRegistry, SkillSourceType,
};

// =============================================================================
// Skill Scanner：遍历来源目录并构建 Registry
// =============================================================================
// 本模块负责发现磁盘上所有 Skill 来源目录，解析其中的 SKILL.md，
// 并在缓存目录创建符号链接，最终组装成 SkillRegistry。

/// 扫描所有 Skill 来源目录，返回存在的目录列表。
///
/// 来源目录按优先级排序（后加载的覆盖先加载的）：
/// 1. `<workspace>/.skills/` — scope = workspace 目录名, type = Project
/// 2. `~/.config/fi-code/skills/` — scope = "fi-code", type = Global
/// 3. `~/.config/agent/skills/` — scope = "agent", type = Agent
/// 4. `~/.claude/skills/` — scope = "claude", type = Claude
///
/// `directories::ProjectDirs::from("", "", app)` 用于解析平台相关的配置目录。
/// `dirs::home_dir()` 用于解析用户主目录。
pub fn scan_sources(workspace: &Path) -> Vec<(PathBuf, String, SkillSourceType)> {
    let mut sources = Vec::new();

    // 1. 工作区本地 Skill 目录
    let workspace_skills = workspace.join(".skills");
    if workspace_skills.is_dir() {
        let scope = workspace
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();
        sources.push((workspace_skills, scope, SkillSourceType::Project));
    }

    // 2. fi-code 全局配置目录
    if let Some(project_dirs) = directories::ProjectDirs::from("", "", "fi-code") {
        let global_skills = project_dirs.config_dir().join("skills");
        if global_skills.is_dir() {
            sources.push((
                global_skills,
                "fi-code".to_string(),
                SkillSourceType::Global,
            ));
        }
    }

    // 3. agent 全局配置目录
    if let Some(project_dirs) = directories::ProjectDirs::from("", "", "agent") {
        let agent_skills = project_dirs.config_dir().join("skills");
        if agent_skills.is_dir() {
            sources.push((agent_skills, "agent".to_string(), SkillSourceType::Agent));
        }
    }

    // 4. Claude 家目录
    if let Some(home) = dirs::home_dir() {
        let claude_skills = home.join(".claude").join("skills");
        if claude_skills.is_dir() {
            sources.push((claude_skills, "claude".to_string(), SkillSourceType::Claude));
        }
    }

    sources
}

/// 扫描单个来源目录，将其下包含 SKILL.md 的直接子目录加载为 Skill。
///
/// 对每个有效的 Skill 目录：
/// - 调用 `load_skill_metadata_and_body` 解析元数据
/// - 生成 id = "{scope}-{metadata.name}"
/// - 在 `cache_dir` 下创建指向原目录的 symlink
/// - 如果 registry 中已有相同 id 的条目，则移除旧条目（override）
/// - 将新 `SkillEntry` 追加到 registry
///
/// 遇到错误时打印警告并继续处理其他目录。
pub fn scan_source_dir(
    source_dir: &Path,
    scope: &str,
    source_type: SkillSourceType,
    cache_dir: &Path,
    registry: &mut SkillRegistry,
) {
    let dir_entries = match fs::read_dir(source_dir) {
        Ok(entries) => entries,
        Err(e) => {
            log_info!(
                "Warning: failed to read source directory {:?}: {}",
                source_dir,
                e
            );
            return;
        }
    };

    for entry in dir_entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // 只处理包含 SKILL.md 的目录
        let skill_md_path = path.join("SKILL.md");
        if !skill_md_path.exists() {
            continue;
        }

        match load_skill_metadata_and_body(&path) {
            Ok((metadata, _body)) => {
                let id = format!("{}-{}", scope, metadata.name);

                // 移除 registry 中已有的同名条目（override 行为）
                registry.entries.retain(|e| e.id != id);

                // 创建或替换 symlink
                let symlink_path = cache_dir.join(&id);
                if symlink_path.exists() {
                    if let Err(e) = fs::remove_file(&symlink_path) {
                        log_info!(
                            "Warning: failed to remove existing symlink {:?}: {}",
                            symlink_path,
                            e
                        );
                        continue;
                    }
                }

                if let Err(e) = create_symlink(&path, &symlink_path) {
                    log_info!(
                        "Warning: failed to create symlink {:?} -> {:?}: {}",
                        symlink_path,
                        path,
                        e
                    );
                    continue;
                }

                registry.entries.push(SkillEntry {
                    id,
                    scope: scope.to_string(),
                    source_type,
                    symlink_path,
                    target_path: path.clone(),
                    metadata,
                });
            }
            Err(e) => {
                log_info!("Warning: failed to load skill from {:?}: {}", path, e);
            }
        }
    }
}

/// 跨平台创建符号链接的包装函数。
///
/// Unix 使用 `std::os::unix::fs::symlink`；
/// Windows 根据目标是文件还是目录，分别使用 `symlink_file` 或 `symlink_dir`。
#[cfg(unix)]
pub fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
pub fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// 扫描所有来源目录并构建完整的 `SkillRegistry`。
///
/// 流程：
/// 1. 获取缓存目录并确保其存在
/// 2. 扫描所有来源目录
/// 3. 对每个来源调用 `scan_source_dir`
/// 4. 清理过期的 registry 条目
/// 5. 保存 registry 到磁盘
pub fn scan_and_build_registry(workspace: &Path) -> SkillRegistry {
    let cache_dir = cache_skills_dir();
    if let Err(e) = fs::create_dir_all(&cache_dir) {
        eprintln!(
            "Warning: failed to create cache directory {:?}: {}",
            cache_dir, e
        );
    }

    let mut registry = SkillRegistry::new();
    let sources = scan_sources(workspace);

    for (source_dir, scope, source_type) in sources {
        scan_source_dir(&source_dir, &scope, source_type, &cache_dir, &mut registry);
    }

    cleanup_stale_entries(&mut registry);

    if let Err(e) = save_registry(&registry) {
        eprintln!("Warning: failed to save registry: {}", e);
    }

    registry
}

// =============================================================================
// 单元测试
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillMetadata;
    use std::io::Write;
    use tempfile::TempDir;

    /// 辅助函数：在指定目录下创建有效的 SKILL.md
    fn write_skill_md(dir: &Path, name: &str, description: &str) {
        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n# Skill Body\n",
            name, description
        );
        let mut file = fs::File::create(dir.join("SKILL.md")).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    /// 测试 scan_source_dir 能正确发现包含 SKILL.md 的有效 Skill 目录
    #[test]
    fn test_scan_source_dir_valid_skill() {
        let source_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let skill_dir = source_dir.path().join("my-skill");
        fs::create_dir(&skill_dir).unwrap();
        write_skill_md(&skill_dir, "my-skill", "A test skill");

        let mut registry = SkillRegistry::new();
        scan_source_dir(
            source_dir.path(),
            "test-scope",
            SkillSourceType::Project,
            cache_dir.path(),
            &mut registry,
        );

        assert_eq!(registry.entries.len(), 1);
        let entry = &registry.entries[0];
        assert_eq!(entry.id, "test-scope-my-skill");
        assert_eq!(entry.scope, "test-scope");
        assert_eq!(entry.source_type, SkillSourceType::Project);
        assert_eq!(entry.metadata.name, "my-skill");
        assert_eq!(entry.metadata.description, "A test skill");
        assert!(entry.symlink_path.exists());
    }

    /// 测试 scan_source_dir 跳过不包含 SKILL.md 的目录，只保留有效 Skill
    #[test]
    fn test_scan_source_dir_skips_invalid() {
        let source_dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();

        // 有效 Skill 目录
        let valid_dir = source_dir.path().join("valid-skill");
        fs::create_dir(&valid_dir).unwrap();
        write_skill_md(&valid_dir, "valid-skill", "A valid skill");

        // 无效目录（缺少 SKILL.md）
        let invalid_dir = source_dir.path().join("invalid-dir");
        fs::create_dir(&invalid_dir).unwrap();

        let mut registry = SkillRegistry::new();
        scan_source_dir(
            source_dir.path(),
            "test-scope",
            SkillSourceType::Global,
            cache_dir.path(),
            &mut registry,
        );

        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].id, "test-scope-valid-skill");
    }

    /// 测试相同 id 的 Skill 会被后加载的来源覆盖
    #[test]
    fn test_scan_source_dir_overrides_same_name() {
        let source_dir1 = TempDir::new().unwrap();
        let source_dir2 = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();

        let skill_dir1 = source_dir1.path().join("my-skill");
        fs::create_dir(&skill_dir1).unwrap();
        write_skill_md(&skill_dir1, "my-skill", "First version");

        let skill_dir2 = source_dir2.path().join("my-skill");
        fs::create_dir(&skill_dir2).unwrap();
        write_skill_md(&skill_dir2, "my-skill", "Second version");

        let mut registry = SkillRegistry::new();

        // 第一次扫描
        scan_source_dir(
            source_dir1.path(),
            "same-scope",
            SkillSourceType::Global,
            cache_dir.path(),
            &mut registry,
        );
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].metadata.description, "First version");

        // 第二次扫描，相同 scope 和 name，应覆盖
        scan_source_dir(
            source_dir2.path(),
            "same-scope",
            SkillSourceType::Agent,
            cache_dir.path(),
            &mut registry,
        );
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(registry.entries[0].metadata.description, "Second version");
        assert_eq!(registry.entries[0].source_type, SkillSourceType::Agent);
        // symlink 应指向第二个目录
        assert_eq!(registry.entries[0].target_path, skill_dir2);
    }
}
