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

// skills 模块：Agent Skills 功能入口
// 负责子模块声明与公共类型导出，让外部可以通过 `crate::skills::SkillMetadata` 等方式访问

pub mod loader;
pub mod registry;
pub mod scanner;
pub mod skill_type;

// 重新导出常用类型，减少外部调用时的路径层级
pub use skill_type::{SkillEntry, SkillMetadata, SkillRegistry, SkillSourceType};

// =============================================================================
// 全局 RwLock 注册表
// =============================================================================
// 使用 `RwLock` 替代 `LazyLock`，支持热重载时替换 registry 内容。

use std::sync::RwLock;

static SKILL_REGISTRY: RwLock<SkillRegistry> = RwLock::new(SkillRegistry {
    version: String::new(),
    entries: Vec::new(),
});

/// 初始化 Skill 注册表，支持传入用户自定义目录。
///
/// 调用此方法会重新扫描所有来源（系统默认 + 用户自定义）并替换 registry。
pub fn init_skills(extra_dirs: Option<&[String]>) {
    let workspace = crate::utils::workspace::workspace_dir();
    let registry = scanner::scan_and_build_registry(&workspace, extra_dirs);
    let mut lock = SKILL_REGISTRY.write().unwrap();
    *lock = registry;
}

/// 重新扫描 Skill 注册表。
///
/// 与 `init_skills` 等价，用于热重载场景。
pub fn rescan_skills(extra_dirs: Option<&[String]>) {
    init_skills(extra_dirs);
}

/// 获取全局 Skill 注册表的快照（clone）。
///
/// 返回 clone 而非引用，避免长期持有读锁。
pub fn get_registry() -> SkillRegistry {
    SKILL_REGISTRY.read().unwrap().clone()
}

/// 加载指定 Skill 的完整内容。
///
/// 流程：
/// 1. 通过 `name_or_id` 在注册表中查找对应条目
/// 2. 读取条目的 `target_path`（符号链接指向的原始目录）
/// 3. 调用 `loader::load_skill_full_content` 加载完整内容
/// 4. 将结果包装在 `<skill name="..." id="...">...</skill>` XML 标签中返回
pub fn load_skill_content(name_or_id: &str) -> Result<String, String> {
    let registry = get_registry();
    let entry = registry
        .find(name_or_id)
        .ok_or_else(|| format!("Skill '{}' not found", name_or_id))?;

    let content = loader::load_skill_full_content(&entry.target_path)?;
    Ok(format!(
        "<skill name=\"{}\" id=\"{}\">{}</skill>",
        entry.metadata.name, entry.id, content
    ))
}

// =============================================================================
// 单元测试
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 测试加载不存在的 Skill 时返回包含 "not found" 的错误
    #[test]
    fn test_load_skill_content_not_found() {
        let result = load_skill_content("nonexistent-skill-xyz");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not found"),
            "error should contain 'not found', got: {}",
            err
        );
    }
}
