# Glob 工具设计文档

## 1. 概述

### 1.1 问题背景

当前 fi-code 缺少一个按文件名或扩展名搜索文件的功能。当需要新增 `glob` 工具，支持通过 glob 模式快速搜索匹配的文件。

### 1.2 目标

- 提供简单易用的文件搜索功能
- 支持标准 glob 模式语法
- 与现有工具风格保持一致
- 遵循安全限制（不超出工作目录）

---

## 2. 详细设计

### 2.1 新增依赖

在 `Cargo.toml` 中添加：
```toml
glob = "0.3"
```

### 2.2 工具参数

```json
{
  "name": "glob",
  "description": "使用 glob 模式搜索文件，支持 *、**、?、[] 等模式",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern": {
        "type": "string",
        "description": "Glob 模式，如 **/*.rs、src/**/*、*.md"
      },
      "dir": {
        "type": "string",
        "description": "可选，搜索根目录，默认为当前工作目录"
      }
    },
    "required": ["pattern"]
  }
}
```

### 2.3 实现结构

#### 2.3.1 BasicTool::run_glob
新增静态方法

```rust
pub fn run_glob(pattern: &str, dir: Option<&str>) -> Result<String, String>
```

功能：
- 使用 `glob::glob_with` 进行模式匹配
- 限制在工作目录内
- 限制最大返回 1000 个文件
- 限制输出 50000 字符

#### 2.3.2 GlobHandler
新增工具处理器

```rust
struct GlobHandler;
impl ToolHandler for GlobHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String>;
}
```

#### 2.3.3 工具注册
在 `mod.rs` 的 `REGISTRY` 初始化中添加：
```rust
registry.register(
    "glob",
    "使用 glob 模式搜索文件，支持 *、**、?、[] 等模式",
    r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Glob 模式，如 **/*.rs、src/**/*、*.md"},"dir":{"type":"string","description":"可选，搜索根目录，默认为当前工作目录"}},"required":["pattern"]}"#,
    Box::new(GlobHandler),
).expect("register glob tool failed");
```

### 2.4 安全设计

- 复用现有的 `BasicTool::safe_path` 检查
- 所有返回路径必须在工作目录内
- 最大返回 1000 个文件，避免内存问题
- 输出限制 50000 字符，避免撑爆上下文

### 2.5 输出格式

每行一个相对路径（相对于工作目录）：
```
src/main.rs
src/tools/mod.rs
src/config.rs
```

如果没有匹配的文件：
```
No files found matching pattern
```

---

## 3. 测试计划

### 3.1 单元测试
1. 测试基本的 `*.rs` 匹配
2. 测试递归 `**/*.md` 匹配
3. 测试指定目录搜索
4. 测试无匹配的情况
5. 测试路径安全检查

---

## 4. 集成

### 4.1 修改文件
- `Cargo.toml` - 添加依赖
- `src/tools/basic_tools.rs` - 添加 `run_glob` 方法
- `src/tools/mod.rs` - 添加 `GlobHandler` 和注册代码

---

## 5. 示例

### 示例 1：搜索所有 Rust 文件
```json
{
  "pattern": "**/*.rs"
}
```

输出：
```
src/main.rs
src/tools/mod.rs
src/config.rs
```

### 示例 2：在 src 目录搜索 Markdown 文件
```json
{
  "pattern": "**/*.md",
  "dir": "src"
}
```
