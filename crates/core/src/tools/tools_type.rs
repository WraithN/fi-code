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

use serde_json::Value;

// =============================================================================
// Rust 基础概念：枚举（Enum）与类型别名（Type Alias）
// =============================================================================
// `enum` 允许一个类型具有多个不同的变体（variant），每个变体可以携带不同的数据
// 这里我们用 `ToolParameter` 来抽象"工具参数"这一概念，它可能是字符串、数字、布尔值、JSON 对象或空值

/// 抽象任意类型的工具参数
/// 使用枚举的好处是：调用方无需关心底层具体类型，只需统一传递 `ToolParameter`
#[derive(Debug, Clone)]
pub enum ToolParameter {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Json(Value),
    Null,
}

// =============================================================================
// Rust 基础概念：From trait 与类型转换
// =============================================================================
// `From<T>` 是 Rust 标准库中的转换 trait，实现了它之后可以自动获得 `into()` 方法
// 这里我们将 `serde_json::Value` 自动转换为 `ToolParameter`，方便外部直接把 JSON 传进来

impl From<Value> for ToolParameter {
    fn from(value: Value) -> Self {
        match value {
            Value::String(s) => ToolParameter::String(s),
            Value::Number(n) => {
                // `as_i64()` 尝试把数字转为整数，如果失败则转为浮点数
                if let Some(i) = n.as_i64() {
                    ToolParameter::Integer(i)
                } else {
                    ToolParameter::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::Bool(b) => ToolParameter::Bool(b),
            Value::Null => ToolParameter::Null,
            // 数组或对象等复杂 JSON 类型统一包装为 Json 变体
            v => ToolParameter::Json(v),
        }
    }
}

// =============================================================================
// Rust 基础概念：类型别名
// =============================================================================
// `type` 关键字为已有类型起别名，增强代码可读性
// `ToolParams` 表示"一个工具的参数列表"，本质是 `Vec<ToolParameter>`

/// 可变数量的工具参数
pub type ToolParams = Vec<ToolParameter>;

// =============================================================================
// Rust 基础概念：Trait 与面向接口编程
// =============================================================================
// `trait` 类似于其他语言中的接口（interface），定义了一组行为契约
// `ToolHandler` 定义了所有工具都必须实现的 `call` 方法
//
// `Send + Sync` 是两个标记 trait（marker trait），表示该类型可以安全地跨线程传递和共享引用

/// 工具处理 trait
pub trait ToolHandler: Send + Sync {
    /// 调用工具，传入工具名称和可变参数
    /// `&self` 表示借用当前对象，允许 trait 对象（`Box<dyn ToolHandler>`）安全调用
    fn call(&self, name: &str, params: ToolParams) -> Result<String, String>;
}
