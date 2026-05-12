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
use std::path::Path;

// =============================================================================
// Skill 内容加载器
// =============================================================================
// 本模块负责解析 SKILL.md 的 YAML front matter 格式，并组装完整的 Skill 内容。
// YAML front matter 是一种常见的文档元数据格式：以 `---` 开头和结尾，中间是 YAML，
// 后面跟着 Markdown 正文。

use crate::skills::SkillMetadata;

/// 解析 SKILL.md 格式的字符串，提取 YAML front matter 和 Markdown 正文。
///
/// 格式要求：
/// ```text
/// ---
/// name: example
/// description: An example skill
/// ---
/// # Markdown Body
/// ```
///
/// 返回 `(yaml_string, markdown_body)`，其中 yaml_string 不包含首尾的 `---`。
/// `trim_start_matches` 和 `trim_end_matches` 用于去除首尾空白，
/// `splitn(3, "---")` 将字符串按 `---` 最多切分成 3 段，确保我们能正确提取中间部分。
pub fn parse_skill_md(content: &str) -> Result<(String, String), String> {
    // 1. 检查字符串是否以 `---` 开头
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err("Missing opening `---` delimiter".to_string());
    }

    // 2. 去掉开头的 `---`，然后查找第二个 `---`
    let after_open = &trimmed[3..];

    // 3. 查找关闭的 `---`
    let Some(closing_pos) = after_open.find("---") else {
        return Err("Missing closing `---` delimiter".to_string());
    };

    // 4. 提取 YAML 部分（去掉首尾空白）
    let yaml_part = after_open[..closing_pos].trim().to_string();

    // 5. 提取 Markdown 正文部分（从关闭 delimiter 之后开始）
    let body_part = after_open[closing_pos + 3..].trim_start().to_string();

    Ok((yaml_part, body_part))
}

/// 从指定目录加载 Skill 的元数据和正文。
///
/// 流程：
/// 1. 读取 `{dir}/SKILL.md` 文件内容
/// 2. 调用 `parse_skill_md` 分离 YAML 和 Markdown
/// 3. 使用 `serde_yaml::from_str` 将 YAML 反序列化为 `SkillMetadata`
/// 4. 校验 `name` 字段非空
///
/// `?` 是 Rust 的错误传播运算符：如果表达式返回 `Err`，则提前返回。
/// `map_err` 将一种错误类型转换为另一种，这里把 `serde_yaml` 的错误转为 `String`。
pub fn load_skill_metadata_and_body(dir: &Path) -> Result<(SkillMetadata, String), String> {
    let skill_md_path = dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    let (yaml_str, body) = parse_skill_md(&content)?;

    let metadata: SkillMetadata =
        serde_yaml::from_str(&yaml_str).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    if metadata.name.trim().is_empty() {
        return Err("Skill metadata `name` cannot be empty".to_string());
    }

    Ok((metadata, body))
}

/// 加载 Skill 的完整内容，包括正文、参考文档和示例。
///
/// 组装顺序：
/// 1. SKILL.md 的 Markdown 正文
/// 2. 如果存在 `{dir}/REFERENCE.md`，追加 `\n\n--- Reference ---\n{content}`
/// 3. 如果存在 `{dir}/examples/*.md`，按文件名排序后追加，每个文件格式为：
///    `\n\n--- Examples ---\n\n### {filename}\n{content}`
///
/// `read_dir` 返回目录下的条目迭代器，`filter_map` 过滤掉读取失败的条目，
/// 并只保留以 `.md` 结尾的文件。
pub fn load_skill_full_content(dir: &Path) -> Result<String, String> {
    let (_metadata, mut body) = load_skill_metadata_and_body(dir)?;

    // 追加 REFERENCE.md
    let reference_path = dir.join("REFERENCE.md");
    if reference_path.exists() {
        let reference_content = fs::read_to_string(&reference_path)
            .map_err(|e| format!("Failed to read REFERENCE.md: {}", e))?;
        body.push_str("\n\n--- Reference ---\n");
        body.push_str(&reference_content);
    }

    // 追加 examples/*.md
    let examples_dir = dir.join("examples");
    if examples_dir.exists() && examples_dir.is_dir() {
        let mut example_files: Vec<_> = fs::read_dir(&examples_dir)
            .map_err(|e| format!("Failed to read examples dir: {}", e))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() && path.extension().map(|ext| ext == "md").unwrap_or(false) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        // 按文件名排序，保证输出顺序稳定
        example_files.sort_by(|a, b| {
            let a_name = a.file_name().unwrap_or_default().to_string_lossy();
            let b_name = b.file_name().unwrap_or_default().to_string_lossy();
            a_name.cmp(&b_name)
        });

        if !example_files.is_empty() {
            body.push_str("\n\n--- Examples ---\n");
            for path in example_files {
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read example file {}: {}", filename, e))?;
                body.push_str(&format!("\n### {}\n{}", filename, content));
            }
        }
    }

    Ok(body)
}

// =============================================================================
// 单元测试
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 测试正常解析包含完整 front matter 的 SKILL.md 内容
    #[test]
    fn test_parse_skill_md_valid() {
        let content =
            "---\nname: test-skill\ndescription: A test skill\n---\n# Hello\nThis is the body.\n";
        let result = parse_skill_md(content).unwrap();
        assert_eq!(result.0, "name: test-skill\ndescription: A test skill");
        assert_eq!(result.1, "# Hello\nThis is the body.\n");
    }

    /// 测试缺少开头 `---` delimiter 时应返回错误
    #[test]
    fn test_parse_skill_md_missing_delimiter() {
        let content = "name: test\ndescription: no delimiters\n# Body\n";
        let result = parse_skill_md(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing opening"));
    }

    /// 测试有开头 `---` 但缺少结尾 `---` 时应返回错误
    #[test]
    fn test_parse_skill_md_no_closing() {
        let content = "---\nname: test\ndescription: no closing\n# Body\n";
        let result = parse_skill_md(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing closing"));
    }
}
