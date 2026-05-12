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

use std::collections::HashMap;
use std::io::{self, Write};

use crate::log_debug;

/// 风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskType {
    Critical,
    High,
    Low,
}

/// 处置措施
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    Ask,
    Allow,
    Deny,
}

impl PermissionAction {
    /// 根据工具名称和参数进行权限校验，返回 (处置措施, 风险等级, 原因说明)
    pub fn match_action(
        tool_name: &str,
        tool_params: &HashMap<String, serde_json::Value>,
    ) -> (Self, RiskType, String) {
        // 如果是 bash 工具，提取命令内容并转为小写以便检查
        let command = if tool_name == "bash" {
            tool_params
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase()
        } else {
            String::new()
        };

        // ========== DENY 场景 ==========
        let result = if tool_name == "bash" {
            // sudo 越权访问
            if command.contains("sudo") {
                (
                    Self::Deny,
                    RiskType::Critical,
                    "sudo privilege escalation detected".to_string(),
                )
            } else if command.contains("rm -rf") || command.contains("rm -fr") {
                (
                    Self::Deny,
                    RiskType::Critical,
                    "rm -rf dangerous operation detected".to_string(),
                )
            } else if is_bash_injection(&command) {
                (
                    Self::Deny,
                    RiskType::High,
                    "potential bash injection attack detected".to_string(),
                )
            } else {
                (
                    Self::Ask,
                    RiskType::High,
                    "tool bash requires user confirmation".to_string(),
                )
            }
        } else if tool_name == "read_file" || tool_name == "read" || tool_name == "grep" {
            (
                Self::Allow,
                RiskType::Low,
                format!("tool {} is in allowlist", tool_name),
            )
        } else {
            (
                Self::Ask,
                RiskType::Low,
                format!("tool {} requires user confirmation", tool_name),
            )
        };

        log_debug!(
            "permission check | tool={} | action={:?} | risk={:?} | reason={}",
            tool_name,
            result.0,
            result.1,
            result.2
        );
        result
    }
}

/// 检测 bash 注入攻击特征
fn is_bash_injection(command: &str) -> bool {
    let injection_patterns = [";", "|", "&&", "||", "`", "$(", ">", "<", "&"];
    injection_patterns.iter().any(|p| command.contains(p))
}

/// 权限检查器
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查工具调用权限
    /// - DENY:  直接返回 Permission denied
    /// - ASK:   提示用户输入，输入 Yes 则调用工具，否则返回 Permission denied
    /// - ALLOW: 直接运行工具
    pub async fn check(
        tool_name: &str,
        input: &HashMap<String, serde_json::Value>,
    ) -> Result<String, String> {
        let result = PermissionAction::match_action(tool_name, input);

        match result.0 {
            PermissionAction::Deny => {
                log_debug!(
                    "permission denied | tool={} | reason={}",
                    tool_name,
                    result.2
                );
                Err(format!("Permission denied: {}", result.2))
            }
            PermissionAction::Allow => {
                log_debug!("permission allowed | tool={}", tool_name);
                crate::tools::tool_call(tool_name, input).await
            }
            PermissionAction::Ask => {
                log_debug!(
                    "permission ask | tool={} | risk={:?} | reason={}",
                    tool_name,
                    result.1,
                    result.2
                );
                println!(
                    "[Permission] Tool '{}' requires confirmation (risk: {:?}, reason: {})",
                    tool_name, result.1, result.2
                );
                print!("Allow execution? (Yes/No): ");
                io::stdout().flush().map_err(|e| e.to_string())?;

                let mut buffer = String::new();
                io::stdin()
                    .read_line(&mut buffer)
                    .map_err(|e| e.to_string())?;

                let answer = buffer.trim();
                if answer.eq_ignore_ascii_case("yes") || answer.eq_ignore_ascii_case("y") {
                    crate::tools::tool_call(tool_name, input).await
                } else {
                    Err("Permission denied: user rejected".to_string())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_read_and_grep() {
        let mut params = HashMap::new();
        params.insert("path".to_string(), serde_json::json!("src/main.rs"));

        let (action, risk, _) = PermissionAction::match_action("read", &params);
        assert_eq!(action, PermissionAction::Allow);
        assert_eq!(risk, RiskType::Low);

        let (action, risk, _) = PermissionAction::match_action("grep", &params);
        assert_eq!(action, PermissionAction::Allow);
        assert_eq!(risk, RiskType::Low);

        let (action, _, _) = PermissionAction::match_action("read_file", &params);
        assert_eq!(action, PermissionAction::Allow);
    }

    #[test]
    fn test_bash_sudo_deny() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), serde_json::json!("sudo rm -rf /"));
        let (action, risk, reason) = PermissionAction::match_action("bash", &params);
        assert_eq!(action, PermissionAction::Deny);
        assert_eq!(risk, RiskType::Critical);
        assert!(reason.contains("sudo"));
    }

    #[test]
    fn test_bash_rm_rf_deny() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), serde_json::json!("rm -rf /tmp/test"));
        let (action, risk, reason) = PermissionAction::match_action("bash", &params);
        assert_eq!(action, PermissionAction::Deny);
        assert_eq!(risk, RiskType::Critical);
        assert!(reason.contains("rm -rf"));
    }

    #[test]
    fn test_bash_injection_deny() {
        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::json!("echo hello | cat /etc/passwd"),
        );
        let (action, risk, reason) = PermissionAction::match_action("bash", &params);
        assert_eq!(action, PermissionAction::Deny);
        assert_eq!(risk, RiskType::High);
        assert!(reason.contains("injection"));
    }

    #[test]
    fn test_bash_safe_ask() {
        let mut params = HashMap::new();
        params.insert("command".to_string(), serde_json::json!("echo hello"));
        let (action, risk, _) = PermissionAction::match_action("bash", &params);
        assert_eq!(action, PermissionAction::Ask);
        assert_eq!(risk, RiskType::High);
    }

    #[test]
    fn test_other_tool_ask() {
        let params = HashMap::new();
        let (action, risk, _) = PermissionAction::match_action("write", &params);
        assert_eq!(action, PermissionAction::Ask);
        assert_eq!(risk, RiskType::Low);
    }
}
