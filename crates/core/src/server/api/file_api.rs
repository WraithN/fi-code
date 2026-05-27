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

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::server::models::ApiResponse;
use crate::server::server::AppState;

#[derive(Debug, Deserialize)]
pub struct FileTreeQuery {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub git_status: bool,
}

#[derive(Debug, Deserialize)]
pub struct FileContentQuery {
    pub path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileEntry>>,
}

#[derive(Debug, serde::Serialize)]
pub struct FileTreeResponse {
    pub root: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, serde::Serialize)]
pub struct FileContentResponse {
    pub path: String,
    pub content: String,
    pub language: String,
    pub size: usize,
    pub lines: usize,
}

pub async fn file_tree(
    State(_state): State<AppState>,
    Query(query): Query<FileTreeQuery>,
) -> Json<ApiResponse<FileTreeResponse>> {
    let root = if query.path.is_empty() {
        ".".to_string()
    } else {
        query.path
    };

    let mut entries = Vec::new();

    if let Ok(dir) = std::fs::read_dir(&root) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            entries.push(FileEntry {
                path,
                name,
                is_dir,
                depth: 0,
                git_status: None,
                children: None,
            });
        }
    }

    // 排序规则：
    // 1. 文件夹排在文件前面
    // 2. 隐藏文件/文件夹（以 . 开头）排在前面
    // 3. 字母排序，小写在前，大写在后
    entries.sort_by(|a, b| {
        // 规则1：文件夹优先
        match b.is_dir.cmp(&a.is_dir) {
            std::cmp::Ordering::Equal => {
                // 规则2：隐藏文件优先
                match b.name.starts_with('.').cmp(&a.name.starts_with('.')) {
                    std::cmp::Ordering::Equal => {
                        // 规则3：字母排序，小写在前
                        compare_file_name(&a.name, &b.name)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    let response = FileTreeResponse { root, entries };
    Json(ApiResponse::success(response))
}

pub async fn file_content(
    State(_state): State<AppState>,
    Query(query): Query<FileContentQuery>,
) -> Json<ApiResponse<FileContentResponse>> {
    match std::fs::read_to_string(&query.path) {
        Ok(content) => {
            let size = content.len();
            let lines = content.lines().count();
            let language = guess_language(&query.path);

            let response = FileContentResponse {
                path: query.path,
                content,
                language,
                size,
                lines,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => Json(ApiResponse::error(
            format!("Failed to read file: {}", e),
            "FILE_READ_ERROR",
        )),
    }
}

/// 比较两个文件名，规则：
/// - 先按字母不区分大小写排序
/// - 同字母时小写排在大写前面
fn compare_file_name(a: &str, b: &str) -> std::cmp::Ordering {
    for (ca, cb) in a.chars().zip(b.chars()) {
        let ca_lower = ca.to_lowercase().next().unwrap_or(ca);
        let cb_lower = cb.to_lowercase().next().unwrap_or(cb);
        if ca_lower != cb_lower {
            return ca_lower.cmp(&cb_lower);
        }
        if ca != cb {
            if ca.is_lowercase() && cb.is_uppercase() {
                return std::cmp::Ordering::Less;
            }
            if ca.is_uppercase() && cb.is_lowercase() {
                return std::cmp::Ordering::Greater;
            }
        }
    }
    a.len().cmp(&b.len())
}

fn guess_language(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("rs") => "rust".to_string(),
        Some("py") => "python".to_string(),
        Some("js") => "javascript".to_string(),
        Some("ts") => "typescript".to_string(),
        Some("md") => "markdown".to_string(),
        Some("json") => "json".to_string(),
        Some("yaml") | Some("yml") => "yaml".to_string(),
        _ => "text".to_string(),
    }
}
