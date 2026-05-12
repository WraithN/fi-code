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
// 全局 LazyLock 注册表
// =============================================================================
// `LazyLock` 是 Rust std 库提供的线程安全懒加载原语，初始化闭包只会执行一次。
// 我们在程序启动时通过 `init_skills()` 显式触发初始化，避免首次调用时的延迟。

use std::sync::LazyLock;

static SKILL_REGISTRY: LazyLock<SkillRegistry> = LazyLock::new(|| {
    let workspace = crate::utils::workspace::workspace_dir();
    scanner::scan_and_build_registry(&workspace)
});

/// 显式触发 Skill 注册表的初始化。
///
/// 调用此方法可确保 `SKILL_REGISTRY` 在程序启动阶段完成扫描与加载，
/// 而不是延迟到首次访问时才初始化。
pub fn init_skills() {
    let _ = LazyLock::force(&SKILL_REGISTRY);
}

/// 获取全局 Skill 注册表的不可变引用。
///
/// 由于 `SkillRegistry` 本身是不可变的（通过 `LazyLock` 保护），
/// 返回 `&'static` 生命周期引用可以安全地在整个程序生命周期内使用。
pub fn get_registry() -> &'static SkillRegistry {
    &SKILL_REGISTRY
}

/// 加载指定 Skill 的完整内容。
///
/// 流程：
/// 1. 通过 `name_or_id` 在注册表中查找对应条目
/// 2. 读取条目的 `target_path`（符号链接指向的原始目录）
/// 3. 调用 `loader::load_skill_full_content` 加载完整内容（正文 + REFERENCE.md + examples/*.md）
/// 4. 将结果包装在 `<skill name="..." id="...">...</skill>` XML 标签中返回
///
/// 如果找不到对应 Skill，返回包含 "not found" 的错误信息。
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
