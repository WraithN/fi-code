use std::fs;
use std::path::PathBuf;

// =============================================================================
// Skill Registry 持久化与清理
// =============================================================================
// 本模块负责 Registry 的磁盘读写、路径管理以及过期条目清理。
// Registry 以 JSON 格式存储在用户配置目录中，方便跨会话保持 Skill 注册状态。

use crate::skills::{SkillEntry, SkillRegistry};

/// 获取 Registry 文件的存储路径。
///
/// 使用 `directories::ProjectDirs` 解析平台相关的配置目录：
/// - Linux: `~/.config/shun-code/registry-skills.json`
/// - macOS: `~/Library/Application Support/shun-code/registry-skills.json`
/// - Windows: `%APPDATA%\shun-code\registry-skills.json`
///
/// `directories::ProjectDirs::from(qualifier, organization, application)` 中，
/// 前两个参数为空字符串，第三个是应用名 "shun-code"。
pub fn registry_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "shun-code")
        .map(|dirs| dirs.config_dir().join("registry-skills.json"))
        .unwrap_or_else(|| PathBuf::from(".config/shun-code/registry-skills.json"))
}

/// 获取 Skill 缓存目录的路径。
///
/// 使用 `directories::ProjectDirs` 解析平台相关的缓存目录：
/// - Linux: `~/.cache/shun-code/skills`
/// - macOS: `~/Library/Caches/shun-code/skills`
/// - Windows: `%LOCALAPPDATA%\shun-code\cache\skills`
pub fn cache_skills_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "shun-code")
        .map(|dirs| dirs.cache_dir().join("skills"))
        .unwrap_or_else(|| PathBuf::from(".cache/shun-code/skills"))
}

/// 从磁盘加载 Registry。
///
/// 行为：
/// 1. 如果文件不存在，返回空的 `SkillRegistry`（静默）
/// 2. 如果文件存在但解析失败，打印警告并返回空的 `SkillRegistry`
/// 3. 成功解析则返回对应的 `SkillRegistry`
///
/// `serde_json::from_str` 将 JSON 字符串反序列化为结构体。
pub fn load_registry() -> SkillRegistry {
    let path = registry_path();
    if !path.exists() {
        return SkillRegistry::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<SkillRegistry>(&content) {
            Ok(registry) => registry,
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse registry at {:?}: {}. Starting with empty registry.",
                    path, e
                );
                SkillRegistry::new()
            }
        },
        Err(e) => {
            eprintln!(
                "Warning: failed to read registry at {:?}: {}. Starting with empty registry.",
                path, e
            );
            SkillRegistry::new()
        }
    }
}

/// 将 Registry 保存到磁盘。
///
/// 流程：
/// 1. 使用 `serde_json::to_string_pretty` 生成格式化的 JSON
/// 2. 使用 `create_dir_all` 确保父目录存在
/// 3. 将 JSON 写入 `registry_path()`
///
/// `map_err` 将 `serde_json` 或 `std::io` 的错误转换为 `String`。
pub fn save_registry(registry: &SkillRegistry) -> Result<(), String> {
    let json = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize registry: {}", e))?;

    let path = registry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create registry directory: {}", e))?;
    }

    fs::write(&path, json).map_err(|e| format!("Failed to write registry: {}", e))?;
    Ok(())
}

/// 清理 Registry 中过期的条目。
///
/// 过期判定：条目的 `symlink_path` 指向的目标不再存在（`exists()` 返回 false）。
/// 清理动作：
/// 1. 从 `registry.entries` 中移除过期条目
/// 2. 尝试删除过期的 symlink 文件
/// 3. 打印移除的条目数量
///
/// `retain` 是 `Vec` 的方法，保留满足条件的元素，同时通过闭包参数获取被移除的元素。
/// 这里我们为了获取被移除的条目以便删除 symlink，使用 `drain_filter` 的替代方案：
/// 先收集要移除的条目，再从原 Vec 中过滤掉它们。
pub fn cleanup_stale_entries(registry: &mut SkillRegistry) {
    let mut removed_count = 0usize;
    let mut to_remove_indices = Vec::new();

    // 第一遍：找出所有过期条目的索引
    for (i, entry) in registry.entries.iter().enumerate() {
        if !entry.symlink_path.exists() {
            to_remove_indices.push(i);
        }
    }

    // 如果没有过期条目，直接返回
    if to_remove_indices.is_empty() {
        println!("cleanup_stale_entries: 0 entries removed");
        return;
    }

    // 从后向前删除，避免索引偏移问题
    for &i in to_remove_indices.iter().rev() {
        let entry = registry.entries.remove(i);
        removed_count += 1;

        // 尝试删除过期的 symlink 文件（如果它还存在的话）
        if entry.symlink_path.exists() {
            if let Err(e) = fs::remove_file(&entry.symlink_path) {
                eprintln!(
                    "Warning: failed to remove stale symlink {:?}: {}",
                    entry.symlink_path, e
                );
            }
        }
    }

    println!("cleanup_stale_entries: {} entries removed", removed_count);
}

// =============================================================================
// 单元测试
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillMetadata, SkillSourceType};
    use std::path::PathBuf;

    /// 测试 Registry 的序列化与反序列化往返（roundtrip）
    /// 先构造一个包含示例条目的 SkillRegistry，序列化为 JSON，再反序列化回来，
    /// 验证字段值保持一致。
    #[test]
    fn test_registry_serialization_roundtrip() {
        let mut registry = SkillRegistry::new();
        registry.entries.push(SkillEntry {
            id: "skill-001".to_string(),
            scope: "global".to_string(),
            source_type: SkillSourceType::Global,
            symlink_path: PathBuf::from("/tmp/skills/test-skill"),
            target_path: PathBuf::from("/home/user/skills/test-skill"),
            metadata: SkillMetadata {
                name: "test-skill".to_string(),
                description: "A test skill for roundtrip".to_string(),
                tags: vec!["test".to_string(), "demo".to_string()],
            },
        });

        let json = serde_json::to_string_pretty(&registry).unwrap();
        let loaded: SkillRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.version, "1.0");
        assert_eq!(loaded.entries.len(), 1);
        let entry = &loaded.entries[0];
        assert_eq!(entry.id, "skill-001");
        assert_eq!(entry.scope, "global");
        assert_eq!(entry.source_type, SkillSourceType::Global);
        assert_eq!(entry.symlink_path, PathBuf::from("/tmp/skills/test-skill"));
        assert_eq!(
            entry.target_path,
            PathBuf::from("/home/user/skills/test-skill")
        );
        assert_eq!(entry.metadata.name, "test-skill");
        assert_eq!(entry.metadata.description, "A test skill for roundtrip");
        assert_eq!(entry.metadata.tags, vec!["test", "demo"]);
    }
}
