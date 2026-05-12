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

use once_cell::sync::Lazy;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsCompatMode {
    Native,
    Wsl2,
    GitBash,
    Cygwin,
    None,
}

impl Default for WindowsCompatMode {
    fn default() -> Self {
        if cfg!(windows) {
            Self::None
        } else {
            Self::Native
        }
    }
}

static WINDOWS_COMPAT_MODE: Lazy<RwLock<WindowsCompatMode>> =
    Lazy::new(|| RwLock::new(detect_windows_compat_mode()));

pub fn get_compat_mode() -> WindowsCompatMode {
    *WINDOWS_COMPAT_MODE.read().unwrap()
}

fn detect_windows_compat_mode() -> WindowsCompatMode {
    if !cfg!(windows) {
        return WindowsCompatMode::Native;
    }

    if check_wsl2() {
        return WindowsCompatMode::Wsl2;
    }

    if check_git_bash() {
        return WindowsCompatMode::GitBash;
    }

    if check_cygwin() {
        return WindowsCompatMode::Cygwin;
    }

    WindowsCompatMode::None
}

fn check_wsl2() -> bool {
    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        return true;
    }

    if let Ok(output) = std::process::Command::new("wsl.exe")
        .arg("--version")
        .output()
    {
        return output.status.success();
    }

    false
}

fn check_git_bash() -> bool {
    let possible_paths = vec![
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];

    for path in possible_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

fn check_cygwin() -> bool {
    let possible_paths = vec![r"C:\cygwin64\bin\bash.exe", r"C:\cygwin\bin\bash.exe"];

    for path in possible_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

pub fn get_bash_path() -> Option<String> {
    match get_compat_mode() {
        WindowsCompatMode::Native => None,
        WindowsCompatMode::Wsl2 => Some("wsl.exe".to_string()),
        WindowsCompatMode::GitBash => {
            let possible_paths = vec![
                r"C:\Program Files\Git\bin\bash.exe",
                r"C:\Program Files (x86)\Git\bin\bash.exe",
            ];
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
                }
            }
            None
        }
        WindowsCompatMode::Cygwin => {
            let possible_paths = vec![r"C:\cygwin64\bin\bash.exe", r"C:\cygwin\bin\bash.exe"];
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
                }
            }
            None
        }
        WindowsCompatMode::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        // 默认应该是 Native 或 None（根据平台）
        let mode = WindowsCompatMode::default();
        assert!(matches!(
            mode,
            WindowsCompatMode::Native | WindowsCompatMode::None
        ));
    }

    #[test]
    fn test_get_compat_mode() {
        let mode = get_compat_mode();
        assert!(matches!(
            mode,
            WindowsCompatMode::Native | WindowsCompatMode::None
        ));
    }
}
