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

use std::path::PathBuf;

/// Skill 元数据（从 SKILL.md 的 YAML front matter 解析）
// #[derive(...)] 是 Rust 的派生宏，编译器会自动为 struct 生成对应 trait 的实现：
// - Debug: 支持 {:?} 格式化输出，方便调试
// - Clone: 支持 .clone() 深拷贝
// - serde::Serialize / serde::Deserialize: 支持序列化与反序列化（如 JSON/YAML）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    // #[serde(default)] 表示如果反序列化时缺少该字段，使用其类型的默认值（Vec::new()）
    // 而不是报错，提升容错性
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Skill 来源类型
// Copy trait 表示该类型可以按位复制（赋值后原变量仍可用），
// 它依赖 Clone；PartialEq / Eq 支持 == 比较
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SkillSourceType {
    Project,
    Global,
    Agent,
    Claude,
}

/// Registry 中的单个 Skill 条目
// #[serde(rename = "type")] 将 Rust 字段名 source_type 映射为序列化/反序列化时的 "type"，
// 因为 type 是 Rust 关键字，不能直接用，但 JSON/YAML 中可以用
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
    // &self 表示不可变借用，返回 Option<&SkillEntry> 表示可能找不到
    // iter() 创建迭代器，find() 接收闭包，返回第一个满足条件的元素
    pub fn find_by_id(&self, id: &str) -> Option<&SkillEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// 按 name 查找，返回最后一个匹配（优先级最高的，因为后加载覆盖前者）
    // iter().rev() 将迭代器反转，从后向前遍历，这样最后一个匹配会被优先返回
    pub fn find_by_name(&self, name: &str) -> Option<&SkillEntry> {
        self.entries.iter().rev().find(|e| e.metadata.name == name)
    }

    /// 按 name 或 id 查找
    // or_else 在 Option 为 None 时执行备用查找，实现“先按 id，再按 name”的降级策略
    pub fn find(&self, name_or_id: &str) -> Option<&SkillEntry> {
        self.find_by_id(name_or_id)
            .or_else(|| self.find_by_name(name_or_id))
    }
}

// 为 SkillRegistry 实现 Default trait，这样可以用 SkillRegistry::default() 创建
// 通常与 new() 保持一致，方便在其他 derive(Default) 的场景中自动使用
impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
