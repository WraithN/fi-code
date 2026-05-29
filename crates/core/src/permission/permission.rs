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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::{oneshot, Mutex};

use crate::log_debug;
use rust_i18n::t;

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

/// CLI 危险模式标志：开启后 CLI 模式下 Ask 级别直接通过
static CLI_DANGEROUS: AtomicBool = AtomicBool::new(false);

pub fn set_cli_dangerous(v: bool) {
    CLI_DANGEROUS.store(v, Ordering::Relaxed);
}

pub fn is_cli_dangerous() -> bool {
    CLI_DANGEROUS.load(Ordering::Relaxed)
}

/// 全局权限响应通道：tool_call_id → oneshot sender
/// 用于 Web/TUI/Desktop 模式下异步等待用户确认
static PERMISSION_RESPONSES: LazyLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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
                    t!("error.sudoDetected").to_string(),
                )
            } else if command.contains("rm -rf") || command.contains("rm -fr") {
                (
                    Self::Deny,
                    RiskType::Critical,
                    t!("error.rmrfDetected").to_string(),
                )
            } else if is_bash_injection(&command) {
                (
                    Self::Deny,
                    RiskType::High,
                    t!("error.bashInjection").to_string(),
                )
            } else {
                (
                    Self::Ask,
                    RiskType::High,
                    t!("tool.bashHighRisk").to_string(),
                )
            }
        } else if tool_name == "read_file" || tool_name == "read" || tool_name == "grep" {
            (
                Self::Allow,
                RiskType::Low,
                t!("tool.autoApproved", name = tool_name).to_string(),
            )
        } else if tool_name == "ask_for_question" {
            (
                Self::Allow,
                RiskType::Low,
                t!("tool.autoApproved", name = tool_name).to_string(),
            )
        } else if tool_name == "write" || tool_name == "edit" {
            (
                Self::Ask,
                RiskType::High,
                t!("tool.writeHighRisk", name = tool_name).to_string(),
            )
        } else {
            (
                Self::Allow,
                RiskType::Low,
                t!("tool.autoApproved", name = tool_name).to_string(),
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

/// 发送权限确认请求，等待用户响应（30 秒超时）
/// 返回 true 表示用户确认，false 表示拒绝或超时
pub async fn wait_permission_response(
    tool_call_id: &str,
    tool_name: &str,
    risk: RiskType,
    reason: &str,
) -> Result<bool, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut map = PERMISSION_RESPONSES.lock().await;
        map.insert(tool_call_id.to_string(), tx);
    }

    log_debug!(
        "permission waiting | tool_call_id={} | tool={} | risk={:?}",
        tool_call_id,
        tool_name,
        risk
    );

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(approved)) => {
            log_debug!(
                "permission resolved | tool_call_id={} | approved={}",
                tool_call_id,
                approved
            );
            Ok(approved)
        }
        Ok(Err(_)) => {
            let mut map = PERMISSION_RESPONSES.lock().await;
            map.remove(tool_call_id);
            Err(t!("error.channelClosed").to_string())
        }
        Err(_) => {
            let mut map = PERMISSION_RESPONSES.lock().await;
            map.remove(tool_call_id);
            Err(t!("error.permissionTimeout").to_string())
        }
    }
}

/// 用户响应权限确认请求
pub async fn respond_permission(tool_call_id: &str, approved: bool) -> Result<(), String> {
    let mut map = PERMISSION_RESPONSES.lock().await;
    if let Some(tx) = map.remove(tool_call_id) {
        tx.send(approved)
            .map_err(|_| t!("error.responseSendFailed").to_string())
    } else {
        Err(format!(
            "Permission request {} not found or already timed out",
            tool_call_id
        ))
    }
}

/// 权限检查器
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查工具调用权限（Web/Server 模式）
    /// 返回 Ok 表示允许执行，Err 表示拒绝
    pub async fn check_web(
        tool_call_id: &str,
        tool_name: &str,
        input: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let (action, risk, reason) = PermissionAction::match_action(tool_name, input);

        match action {
            PermissionAction::Deny => {
                log_debug!("permission denied | tool={} | reason={}", tool_name, reason);
                Err(format!("Permission denied: {}", reason))
            }
            PermissionAction::Allow => {
                log_debug!("permission allowed | tool={}", tool_name);
                Ok(())
            }
            PermissionAction::Ask => {
                log_debug!(
                    "permission ask | tool={} | risk={:?} | reason={}",
                    tool_name,
                    risk,
                    reason
                );
                // 等待用户通过 SSE / API 确认
                if wait_permission_response(tool_call_id, tool_name, risk, &reason).await? {
                    Ok(())
                } else {
                    Err("Permission denied: user rejected".to_string())
                }
            }
        }
    }

    /// 检查工具调用权限（CLI 模式）
    /// - dangerous=true: Ask 级别直接通过
    /// - dangerous=false: Ask 级别拒绝，Deny 级别拒绝
    pub fn check_cli(
        tool_name: &str,
        input: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let (action, _risk, reason) = PermissionAction::match_action(tool_name, input);

        match action {
            PermissionAction::Deny => {
                log_debug!(
                    "permission denied (cli) | tool={} | reason={}",
                    tool_name,
                    reason
                );
                Err(format!("Permission denied: {}", reason))
            }
            PermissionAction::Allow => {
                log_debug!("permission allowed (cli) | tool={}", tool_name);
                Ok(())
            }
            PermissionAction::Ask => {
                if is_cli_dangerous() {
                    log_debug!(
                        "permission auto-approved (cli dangerous) | tool={}",
                        tool_name
                    );
                    Ok(())
                } else {
                    log_debug!(
                        "permission denied (cli) | tool={} | use --dangerous to allow",
                        tool_name
                    );
                    Err(format!(
                        "Permission denied: {}. Use --dangerous flag to allow this operation in CLI mode.",
                        reason
                    ))
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
    fn test_write_edit_high_risk_ask() {
        let params = HashMap::new();
        let (action, risk, reason) = PermissionAction::match_action("write", &params);
        assert_eq!(action, PermissionAction::Ask);
        assert_eq!(risk, RiskType::High);
        assert!(reason.contains("modify files"));

        let (action, risk, reason) = PermissionAction::match_action("edit", &params);
        assert_eq!(action, PermissionAction::Ask);
        assert_eq!(risk, RiskType::High);
        assert!(reason.contains("modify files"));
    }

    #[test]
    fn test_other_tools_auto_allow() {
        let params = HashMap::new();
        let (action, risk, reason) = PermissionAction::match_action("some_unknown_tool", &params);
        assert_eq!(action, PermissionAction::Allow);
        assert_eq!(risk, RiskType::Low);
        assert!(reason.contains("auto-approved"));
    }

    #[test]
    fn test_cli_dangerous() {
        let params = HashMap::new();

        // 默认非 dangerous 模式：Ask 被拒绝
        set_cli_dangerous(false);
        assert!(PermissionChecker::check_cli("write", &params).is_err());

        // dangerous 模式：Ask 通过
        set_cli_dangerous(true);
        assert!(PermissionChecker::check_cli("write", &params).is_ok());

        // dangerous 模式：Deny 仍然拒绝
        let mut bash_params = HashMap::new();
        bash_params.insert("command".to_string(), serde_json::json!("sudo ls"));
        assert!(PermissionChecker::check_cli("bash", &bash_params).is_err());

        // 恢复默认值
        set_cli_dangerous(false);
    }
}
