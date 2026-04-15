#![allow(warnings)]

// =============================================================================
// Rust 基础概念：模块系统
// =============================================================================
// `mod` 关键字声明当前 crate 包含的模块，Rust 编译器会在对应目录查找

mod agent;
mod message;
mod permission;
mod provider;
mod session;
mod tools;

// `anyhow` 是一个错误处理库，提供了简化错误传播的功能
use anyhow::Result;

// `colored` 库用于终端彩色输出
use colored::Colorize;

// `rustyline` 是一个命令行读取库（类似 GNU readline）
use rustyline::DefaultEditor;

use agent::{agent_loop, LoopState};
use message::{Message, Role};
use provider::{Model, Provider};
use session::{SessionManager, SessionMeta, SessionStatus};
use std::path::PathBuf;

// =============================================================================
// 程序入口：main 函数
// =============================================================================

// `#[tokio::main]` 是属性宏，将 main 函数包装在 tokio 异步运行时中
#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化模型与 provider
    let model = Model::get_model()?;
    let mut provider = Provider::new();
    provider.set_model(model.clone());
    let client = provider.get_client()?;
    let mut editor = DefaultEditor::new()?;

    // 2. 初始化 SessionManager
    // 使用 `directories` crate 解析平台相关的配置目录：
    // - Linux:   ~/.config/shun-code/
    // - macOS:   ~/Library/Application Support/shun-code/
    // - Windows: %APPDATA%\shun-code\
    let config_dir = directories::ProjectDirs::from("", "", "shun-code")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config/shun-code"));
    let sessions_dir = config_dir.join("sessions");
    let session_manager = SessionManager::new(sessions_dir.clone());

    // 3. 让用户选择恢复历史会话或创建新会话
    let mut session = choose_or_create_session(&session_manager, &model.model_name).await?;
    // 提示符显示 session ID 的前 8 位，如 "01HQ8J3K >>"
    let prompt_prefix = format!("{} >> ", &session.id[..session.id.len().min(8)]);

    // 4. REPL 主循环
    loop {
        let readline = editor.readline(prompt_prefix.cyan().to_string().as_str());

        match readline {
            Ok(line) => {
                let query = line.trim();

                // 空输入或退出指令
                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }

                editor.add_history_entry(query)?;

                // 5. 构造用户消息并追加到当前会话
                let user_msg = Message::new(
                    session.id.clone(),
                    Role::User,
                    vec![message::Part::Text { text: query.to_string() }],
                );
                session.messages.push(user_msg.clone());

                // 6. 尝试将用户消息持久化到 JSONL（失败仅打印警告，不中断对话）
                if let Err(e) = session_manager.append_message(&session.id, &user_msg) {
                    eprintln!("Warning: failed to persist user message: {}", e);
                }

                // 7. 创建 LoopState 并运行 agent 循环
                let mut state = LoopState::new(session.messages.clone());
                agent_loop(client.as_ref(), &mut state).await?;
                session.messages = state.messages;

                // 8. 每轮对话结束后，全量保存当前会话
                // 使用 spawn_blocking 避免在 async main 中阻塞事件循环
                if let Err(e) = tokio::task::spawn_blocking({
                    let sm = SessionManager::new(sessions_dir.clone());
                    let s = session.clone();
                    move || sm.save_session(&s)
                }).await? {
                    eprintln!("Warning: failed to save session: {}", e);
                }

                // 9. 提取并打印 Assistant 的最终文本回复
                if let Some(last_msg) = session.messages.last() {
                    if last_msg.role == Role::Assistant {
                        let text = provider::extract_text(&last_msg.parts);
                        if !text.is_empty() {
                            println!("{}", text);
                        }
                    }
                    println!();
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted)
            | Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl-C 或 Ctrl-D 触发退出
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

// =============================================================================
// 会话选择辅助函数
// =============================================================================

/// 列出所有历史会话并让用户选择恢复或新建。
///
/// 流程：
/// 1. 若没有任何历史会话，直接创建新会话
/// 2. 否则打印会话列表（按 updated_at 倒序）
/// 3. 用户输入数字选择：0 表示新建，其他数字表示恢复对应会话
/// 4. 非法输入默认恢复最近的一个会话
async fn choose_or_create_session(
    manager: &SessionManager,
    model_name: &str,
) -> Result<session::Session> {
    let sessions = manager.list_sessions()?;
    if sessions.is_empty() {
        return Ok(manager.create_session(model_name)?);
    }

    println!("Recent sessions:");
    for (i, s) in sessions.iter().enumerate() {
        println!(
            "  [{}] {} | {} | {} messages | {}",
            i + 1,
            &s.id[..s.id.len().min(8)],
            s.project_path,
            s.message_count,
            if s.status == SessionStatus::Active {
                "active"
            } else {
                "archived"
            }
        );
    }
    println!("  [0] Create new session");
    println!();
    print!("Select session [1]: ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().parse::<usize>().unwrap_or(1);

    if choice == 0 {
        Ok(manager.create_session(model_name)?)
    } else if choice <= sessions.len() {
        Ok(manager.load_session(&sessions[choice - 1].id)?)
    } else {
        // 越界输入时回退到最近的一个会话
        Ok(manager.load_session(&sessions[0].id)?)
    }
}
