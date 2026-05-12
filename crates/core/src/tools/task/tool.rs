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
use std::sync::Arc;

use crate::provider::base_client::AIClient;
use crate::provider::Provider;
use crate::server::transport::sse::{SseEvent, TaskProgressItem};
use crate::tools::subagent_tool_schema;
use crate::tools::task::{Task, TaskManager, TaskPlan};
use crate::tools::get_event_tx;
use crate::tui::event::AppEvent;

/// 执行 handle_task_plan 工具的异步逻辑
pub async fn execute_handle_task_plan(
    provider: Arc<std::sync::RwLock<Provider>>,
    input: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    let tasks_value = input
        .get("tasks")
        .ok_or("Missing or invalid 'tasks' array")?;
    let tasks_arr = tasks_value
        .as_array()
        .ok_or("Missing or invalid 'tasks' array")?;

    let mut plan = TaskPlan::new("");
    for (idx, task_val) in tasks_arr.iter().enumerate() {
        let name = task_val.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let description = task_val
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        plan.tasks
            .push(Task::new(format!("{}", idx + 1), name, description));
    }

    if plan.tasks.is_empty() {
        return Err("No valid tasks provided".to_string());
    }

    let task_names: Vec<String> = plan.tasks.iter().map(|t| t.name.clone()).collect();

    let provider_clone = Arc::clone(&provider);
    let client_factory: Arc<dyn Fn() -> Box<dyn AIClient> + Send + Sync> = Arc::new(move || {
        provider_clone
            .read()
            .unwrap()
            .get_client()
            .expect("Failed to create client")
    });

    let subagent_schema = subagent_tool_schema().await;
    let task_manager = TaskManager::new(
        client_factory,
        crate::tools::task::manager::DEFAULT_SUBAGENT_PROMPT.to_string(),
        subagent_schema,
    );

    // 生成稳定的 plan_id
    let plan_id = format!("plan-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());

    // 使用独立 OS 线程 + 新 Runtime 执行，避免编译器将 async fn 调用链视为递归
    let plan_id_clone = plan_id.clone();
    let handle = std::thread::spawn(move || {
        let event_tx = get_event_tx();
        let mut on_progress = move |plan: &TaskPlan| {
            if let Some(ref tx) = event_tx {
                let items: Vec<TaskProgressItem> = plan
                    .tasks
                    .iter()
                    .map(|t| TaskProgressItem {
                        id: t.id.clone(),
                        name: t.name.clone(),
                        status: t.status.clone(),
                    })
                    .collect();
                let _ = tx.try_send(AppEvent::SseEvent(SseEvent::TaskProgress {
                    plan_id: plan_id_clone.clone(),
                    tasks: items,
                }));
            }
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async move { task_manager.execute_plan(&mut plan, &mut on_progress).await })
    });

    let summaries = handle
        .join()
        .map_err(|e| format!("Task execution panicked: {:?}", e))?
        .map_err(|e| format!("Task execution failed: {}", e))?;

    let mut result = format!("任务计划已完成，共 {} 个子任务：\n\n", task_names.len());
    for (idx, summary) in summaries.iter().enumerate() {
        let task_name = &task_names[idx];
        let status_icon = match summary.status {
            crate::tools::task::TaskStatus::Completed => "✅",
            crate::tools::task::TaskStatus::Failed => "❌",
            _ => "⏳",
        };
        result.push_str(&format!(
            "{} [任务 {}: {}]\n{}\n\n",
            status_icon,
            idx + 1,
            task_name,
            summary.result
        ));
    }

    Ok(result)
}
