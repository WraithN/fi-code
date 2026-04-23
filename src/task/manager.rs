// =============================================================================
// manager 模块：TaskManager 编排器
// =============================================================================
// 负责任务计划的执行编排：串行执行每个子任务，收集结果并更新状态。

use anyhow::Result;
use std::sync::Arc;

use crate::agent::{AgentRunner, AgentRunResult};
use crate::provider::base_client::AIClient;
use crate::session::message::{Message, Part, Role};
use crate::task::{Task, TaskPlan, TaskStatus};

// =============================================================================
// 任务执行摘要
// =============================================================================

pub struct TaskExecutionSummary {
    pub task_id: String,
    pub result: String,
    pub status: TaskStatus,
}

// =============================================================================
// TaskManager
// =============================================================================

pub struct TaskManager {
    client_factory: Arc<dyn Fn() -> Box<dyn AIClient> + Send + Sync>,
    subagent_prompt: String,
    subagent_tools_schema: serde_json::Value,
    max_turns_per_task: usize,
}

impl TaskManager {
    pub fn new(
        client_factory: Arc<dyn Fn() -> Box<dyn AIClient> + Send + Sync>,
        subagent_prompt: String,
        subagent_tools_schema: serde_json::Value,
    ) -> Self {
        Self {
            client_factory,
            subagent_prompt,
            subagent_tools_schema,
            max_turns_per_task: 25,
        }
    }

    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns_per_task = max;
        self
    }

    pub async fn execute_plan(
        &self,
        plan: &mut TaskPlan,
        on_progress: &mut dyn FnMut(&TaskPlan),
    ) -> Result<Vec<TaskExecutionSummary>> {
        let mut summaries = Vec::new();

        for i in 0..plan.tasks.len() {
            // 更新状态为 InProgress
            plan.tasks[i].status = TaskStatus::InProgress;
            plan.tasks[i].started_at = Some(chrono::Utc::now());
            on_progress(plan);

            // 取出任务信息，避免借用冲突
            let task_id = plan.tasks[i].id.clone();
            let task_name = plan.tasks[i].name.clone();
            let task_desc = plan.tasks[i].description.clone();

            let result = self.execute_single_task(&task_id, &task_name, &task_desc).await;

            // 更新任务结果
            match result {
                Ok(summary) => {
                    plan.tasks[i].status = TaskStatus::Completed;
                    plan.tasks[i].result = Some(summary.clone());
                    plan.tasks[i].completed_at = Some(chrono::Utc::now());
                    summaries.push(TaskExecutionSummary {
                        task_id: plan.tasks[i].id.clone(),
                        result: summary,
                        status: TaskStatus::Completed,
                    });
                }
                Err(e) => {
                    plan.tasks[i].status = TaskStatus::Failed;
                    plan.tasks[i].result = Some(format!("Error: {}", e));
                    plan.tasks[i].completed_at = Some(chrono::Utc::now());
                    summaries.push(TaskExecutionSummary {
                        task_id: plan.tasks[i].id.clone(),
                        result: format!("Error: {}", e),
                        status: TaskStatus::Failed,
                    });
                }
            }

            on_progress(plan);
        }

        Ok(summaries)
    }

    async fn execute_single_task(
        &self,
        task_id: &str,
        task_name: &str,
        task_desc: &str,
    ) -> Result<String> {
        let initial_msg = Message::new(
            format!("subagent-{}", task_id),
            Role::User,
            vec![Part::Text {
                text: format!(
                    "请完成以下任务。完成后请用一段话总结你做了什么以及结果。\n\n任务名称：{}\n任务描述：{}",
                    task_name, task_desc
                ),
            }],
        );

        let runner = AgentRunner::new(
            (self.client_factory)(),
            self.subagent_prompt.clone(),
            self.subagent_tools_schema.clone(),
        )
        .with_max_turns(self.max_turns_per_task);

        let result = runner.run(vec![initial_msg]).await?;
        let summary = extract_summary(&result.messages);
        Ok(summary)
    }
}

fn extract_summary(messages: &[Message]) -> String {
    for msg in messages.iter().rev() {
        if msg.role == Role::Assistant {
            return msg
                .parts
                .iter()
                .map(|p| match p {
                    Part::Text { text } => text.clone(),
                    Part::Reasoning { thinking, .. } => thinking.clone(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    "(no assistant response)".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::message::{Message, Part, Role};

    #[test]
    fn test_extract_summary_with_text() {
        let msg = Message::new(
            "test".to_string(),
            Role::Assistant,
            vec![Part::Text {
                text: "I did the work".to_string(),
            }],
        );
        let summary = extract_summary(&[msg]);
        assert_eq!(summary, "I did the work");
    }

    #[test]
    fn test_extract_summary_empty() {
        let summary = extract_summary(&[]);
        assert_eq!(summary, "(no assistant response)");
    }
}
