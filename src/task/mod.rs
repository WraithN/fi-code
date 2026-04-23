// =============================================================================
// task 模块：任务系统数据模型
// =============================================================================
// 本模块定义了 TaskManager 使用的核心数据结构：
// - `TaskStatus`：任务状态枚举
// - `Task`：单个任务的完整信息
// - `TaskPlan`：由主 Agent 生成的任务计划

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// 任务状态
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::InProgress => write!(f, "InProgress"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed => write!(f, "Failed"),
        }
    }
}

// =============================================================================
// 单个任务
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            status: TaskStatus::Pending,
            result: None,
            started_at: None,
            completed_at: None,
        }
    }
}

// =============================================================================
// 任务计划
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub tasks: Vec<Task>,
    pub original_query: String,
}

impl TaskPlan {
    pub fn new(original_query: impl Into<String>) -> Self {
        Self {
            tasks: Vec::new(),
            original_query: original_query.into(),
        }
    }
}

// =============================================================================
// 子模块
// =============================================================================

pub mod manager;
pub use manager::{TaskExecutionSummary, TaskManager};

// =============================================================================
// 单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Pending), "Pending");
        assert_eq!(format!("{}", TaskStatus::InProgress), "InProgress");
        assert_eq!(format!("{}", TaskStatus::Completed), "Completed");
        assert_eq!(format!("{}", TaskStatus::Failed), "Failed");
    }

    #[test]
    fn test_task_new() {
        let task = Task::new("1", "Read file", "Read src/main.rs");
        assert_eq!(task.id, "1");
        assert_eq!(task.name, "Read file");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.result.is_none());
    }

    #[test]
    fn test_task_plan_serde() {
        let mut plan = TaskPlan::new("Do something complex");
        plan.tasks.push(Task::new("1", "Step 1", "Description"));
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("Do something complex"));
        let decoded: TaskPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tasks.len(), 1);
    }
}
