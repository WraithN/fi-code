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

#![allow(dead_code)]

// =============================================================================
// Rust 基础概念：标准库集合与智能指针
// =============================================================================
// `HashMap<K, V>` 是 Rust 标准库提供的哈希表，平均 O(1) 的插入和查找复杂度
// `Arc<T>`（Atomic Reference Counting）是线程安全的引用计数智能指针，
// 多个地方共享同一份只读数据时，它比 `String` 更轻量，且避免了生命周期问题

use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::tools_type::{ToolHandler, ToolParams};

// =============================================================================
// ToolDigest：工具的元数据（静态描述信息）
// =============================================================================
// 这里使用 `Arc<str>` 而不是 `String`，是因为：
// 1. `Arc<str>` 是只读的，一旦创建不可修改，符合元数据的不可变性
// 2. 多个 `ToolSlot` 可能共享同一个 name/description，Arc 可以零拷贝共享

#[derive(Debug)]
struct ToolDigest {
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub params_schema: Arc<str>,
}

// =============================================================================
// ToolSlot：注册表的内部存储单元
// =============================================================================
// `Box<dyn ToolHandler>` 是 Rust 中的"trait 对象"（trait object），
// 它允许我们在运行时存储不同类型的 handler（BashHandler、ReadHandler 等），
// 只要它们都实现了 `ToolHandler` trait。
// `Box` 负责在堆上分配内存，并管理对象的生命周期。

struct ToolSlot {
    pub digest: ToolDigest,
    pub handler: Box<dyn ToolHandler>,
}

// 手动实现 Debug，因为 `Box<dyn ToolHandler>` 本身不能自动 derive Debug
impl std::fmt::Debug for ToolSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSlot")
            .field("digest", &self.digest)
            .field("handler", &"<dyn ToolHandler>")
            .finish()
    }
}

// =============================================================================
// ToolsRegistry：工具注册表（核心数据结构）
// =============================================================================
// 注册表模式（Registry Pattern）是一种常见的设计模式：
// 所有工具在初始化时"注册"进来，运行时通过名称查找并调用。
// 这样新增工具时不需要修改调用方代码，符合"开闭原则"。

#[derive(Debug)]
pub struct ToolsRegistry {
    // `Arc<str>` 作为 key：轻量、不可变、可共享
    // `ToolSlot` 作为 value：包含元数据和实际执行逻辑
    tools_map: HashMap<Arc<str>, ToolSlot>,
}

impl ToolsRegistry {
    /// 构造函数，创建一个空的注册表
    pub fn new() -> Self {
        Self {
            tools_map: HashMap::new(),
        }
    }

    /// 注册一个新工具
    ///
    /// # 参数
    /// - `name`: 工具的唯一标识名
    /// - `description`: 工具的人类可读描述
    /// - `params_schema`: 参数的 JSON Schema 字符串，用于 LLM 了解如何传参
    /// - `handler`: 实现了 `ToolHandler` 的具体执行器，用 `Box` 包装后存入堆
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        params_schema: &str,
        handler: Box<dyn ToolHandler>,
    ) -> Result<String, String> {
        let name_arc: Arc<str> = Arc::from(name);
        let digest = ToolDigest {
            name: Arc::clone(&name_arc),
            description: Arc::from(description),
            params_schema: Arc::from(params_schema),
        };
        let slot = ToolSlot { digest, handler };
        self.tools_map.insert(Arc::clone(&name_arc), slot);
        Ok(format!("Tool '{}' registered", name))
    }

    /// 根据工具名调用对应工具
    ///
    /// `ok_or_else` 是 `Option` 的方法：如果 `get` 找不到，则生成一个错误信息
    /// `?` 操作符把 `Result` 的错误提前返回
    pub fn call(&self, name: &str, params: ToolParams) -> Result<String, String> {
        let slot = self
            .tools_map
            .get(name)
            .ok_or_else(|| format!("Tool '{}' not found", name))?;
        slot.handler.call(name, params)
    }

    /// 列出所有已注册工具的名称和描述
    pub fn list_tools(&self) -> Result<String, String> {
        let mut tools = Vec::new();
        for (name, slot) in &self.tools_map {
            tools.push(format!("{}: {}", name, slot.digest.description));
        }
        Ok(tools.join("\n"))
    }

    /// 动态生成所有工具的 JSON Schema 数组
    ///
    /// 这里把注册时保存的 `params_schema` 字符串解析回 `serde_json::Value`，
    /// 然后和标准字段 `name`、`description` 一起组装成 schema 对象。
    /// 这样 `tool_schema()` 永远是注册表内数据的"真实反映"，无需手动维护两份。
    pub fn tool_schema(&self) -> serde_json::Value {
        let mut schemas = Vec::new();
        for (_name, slot) in &self.tools_map {
            let input_schema = serde_json::from_str(&slot.digest.params_schema)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            schemas.push(serde_json::json!({
                "name": &*slot.digest.name,
                "description": &*slot.digest.description,
                "input_schema": input_schema,
            }));
        }
        serde_json::Value::Array(schemas)
    }
}
