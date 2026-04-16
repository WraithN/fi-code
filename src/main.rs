#![allow(warnings)]

// =============================================================================
// Rust 基础概念：模块系统
// =============================================================================
// `mod` 关键字声明当前 crate 包含的模块，Rust 编译器会在对应目录查找

mod agent;
mod permission;
mod provider;
mod session;
mod tools;
mod utils;

// `anyhow` 是一个错误处理库，提供了简化错误传播的功能
use anyhow::{Context, Result};

// `colored` 库用于终端彩色输出
use colored::Colorize;

// `rustyline` 是一个命令行读取库（类似 GNU readline）
use rustyline::DefaultEditor;

use agent::{agent_loop, LoopState};
use clap::Parser;
use utils::cli::Args;
use utils::log::set_debug;
use utils::workspace::set_workspace;
use provider::Provider;
use session::message::{Message, Role};
use session::{SessionManager, SessionMeta, SessionStatus};
use std::path::PathBuf;

// =============================================================================
// 程序入口：main 函数
// =============================================================================

// `#[tokio::main]` 是属性宏，将 main 函数包装在 tokio 异步运行时中
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    set_debug(args.log_level.eq_ignore_ascii_case("debug"));

    // 设置工作目录：命令行参数 > 默认用户主目录
    let workspace = args.workspace.clone().unwrap_or_else(|| {
        dirs::home_dir().expect("无法获取用户主目录")
    });
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)
            .with_context(|| format!("无法创建工作目录: {:?}", workspace))?;
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("无法解析工作目录: {:?}", workspace))?;
    set_workspace(workspace);

    let config_dir = directories::ProjectDirs::from("", "", "shun-code")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config/shun-code"));
    let sessions_dir = config_dir.join("sessions");
    let session_manager = SessionManager::new(sessions_dir.clone());

    // -s 优先级最高
    if let Some(session_arg) = args.session {
        if let Some(selector) = session_arg {
            let session = session_manager.find_session(&selector)?;
            SessionManager::print_session(&session);
        } else {
            let sessions = session_manager.list_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                println!("Recent sessions:");
                for (i, s) in sessions.iter().enumerate() {
                    println!(
                        "  [{}] {} | {} | {} messages | {}",
                        i,
                        &s.id[..s.id.len().min(8)],
                        s.project_path,
                        s.message_count,
                        if s.status == SessionStatus::Active { "active" } else { "archived" }
                    );
                }
            }
        }
        return Ok(());
    }

    // 如果没有显式指定操作模式，提示用户
    if !args.interactive && args.session.is_none() && args.command.is_none() {
        println!("Please provide an option. Use -h or --help for more information.");
        return Ok(());
    }

    let provider = Provider::new()?;
    let client = provider.get_client()?;

    // -c 单命令模式
    if let Some(cmd) = args.command {
        let cmd = cmd.trim();
        if !cmd.is_empty() {
            let mut session = session_manager.create_session(provider.model_name()?)?;
            run_single_command(client.as_ref(), &session_manager, &sessions_dir, &mut session, cmd).await?;
        }
        return Ok(());
    }

    // -i 交互式模式
    run_interactive(client.as_ref(), &provider, &session_manager, &sessions_dir).await?;
    Ok(())
}

async fn run_single_command(
    client: &dyn crate::provider::base_client::AIClient,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut session::Session,
    query: &str,
) -> Result<()> {
    use crate::session::message::Part;

    let user_msg = Message::new(
        session.id.clone(),
        Role::User,
        vec![Part::Text { text: query.to_string() }],
    );
    session.messages.push(user_msg.clone());
    let _ = session_manager.append_message(&session.id, &user_msg);

    let mut state = LoopState::new(session.messages.clone());
    agent_loop(client, &mut state).await?;
    session.messages = state.messages;

    if let Err(e) = tokio::task::spawn_blocking({
        let sm = SessionManager::new(sessions_dir.clone());
        let s = session.clone();
        move || sm.save_session(&s)
    }).await?
    {
        eprintln!("Warning: failed to save session: {}", e);
    }

    if let Some(last_msg) = session.messages.last() {
        if last_msg.role == Role::Assistant {
            let text = provider::extract_text(&last_msg.parts);
            if !text.is_empty() {
                println!("{}", text);
            }
        }
    }
    Ok(())
}

async fn run_interactive(
    client: &dyn crate::provider::base_client::AIClient,
    provider: &Provider,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
) -> Result<()> {
    let mut session = choose_or_create_session(session_manager, provider.model_name()?).await?;
    let prompt_prefix = format!("{} >> ", &session.id[..session.id.len().min(8)]);
    let mut editor = DefaultEditor::new()?;

    loop {
        let readline = editor.readline(prompt_prefix.cyan().to_string().as_str());
        match readline {
            Ok(line) => {
                let query = line.trim();
                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }
                editor.add_history_entry(query)?;

                let user_msg = Message::new(
                    session.id.clone(),
                    Role::User,
                    vec![session::message::Part::Text { text: query.to_string() }],
                );
                session.messages.push(user_msg.clone());
                if let Err(e) = session_manager.append_message(&session.id, &user_msg) {
                    eprintln!("Warning: failed to persist user message: {}", e);
                }

                let mut state = LoopState::new(session.messages.clone());
                agent_loop(client, &mut state).await?;
                session.messages = state.messages;

                if let Err(e) = tokio::task::spawn_blocking({
                    let sm = SessionManager::new(sessions_dir.clone());
                    let s = session.clone();
                    move || sm.save_session(&s)
                }).await?
                {
                    eprintln!("Warning: failed to save session: {}", e);
                }

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
            | Err(rustyline::error::ReadlineError::Eof) => break,
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
