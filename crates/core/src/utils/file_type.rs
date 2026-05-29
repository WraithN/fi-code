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

use std::path::Path;

/// 从文件路径推断语言标识符，用于语法高亮。
///
/// 示例：
/// - "src/main.rs" → Some("rust")
/// - "app.tsx" → Some("typescript")
/// - "Makefile" → Some("makefile")
/// - "无扩展名/未知" → None
pub fn file_type_from_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let ext = path.extension().and_then(|e| e.to_str())?;

    let lang = match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "md" | "markdown" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" => "bash",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "dockerfile" => "dockerfile",
        "xml" => "xml",
        "svg" => "svg",
        "scss" | "sass" => "scss",
        "less" => "less",
        "php" => "php",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "r" => "r",
        "lua" => "lua",
        "vim" => "vim",
        "makefile" | "mk" => "makefile",
        "cmake" => "cmake",
        "zig" => "zig",
        "nim" => "nim",
        "elixir" | "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "fs" | "fsx" => "fsharp",
        "cs" => "csharp",
        "vb" => "vb",
        "ps1" => "powershell",
        "dart" => "dart",
        "flutter" => "dart",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "prisma" => "prisma",
        _ => return None,
    };

    Some(lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_from_path_rust() {
        assert_eq!(file_type_from_path("src/main.rs"), Some("rust".to_string()));
    }

    #[test]
    fn test_file_type_from_path_typescript() {
        assert_eq!(
            file_type_from_path("app.tsx"),
            Some("typescript".to_string())
        );
        assert_eq!(
            file_type_from_path("utils.ts"),
            Some("typescript".to_string())
        );
    }

    #[test]
    fn test_file_type_from_path_javascript() {
        assert_eq!(
            file_type_from_path("index.js"),
            Some("javascript".to_string())
        );
    }

    #[test]
    fn test_file_type_from_path_python() {
        assert_eq!(file_type_from_path("script.py"), Some("python".to_string()));
    }

    #[test]
    fn test_file_type_from_path_dockerfile() {
        assert_eq!(
            file_type_from_path("app.dockerfile"),
            Some("dockerfile".to_string())
        );
    }

    #[test]
    fn test_file_type_from_path_no_extension() {
        assert_eq!(file_type_from_path("README"), None);
    }

    #[test]
    fn test_file_type_from_path_unrecognized() {
        assert_eq!(file_type_from_path("data.bin"), None);
    }
}
