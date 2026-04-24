use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use colored::Colorize;
use rustyline::DefaultEditor;

use crate::agent::{agent_loop, LoopState};
use crate::config::Config;
use crate::mcp::manager::McpManager;
use crate::provider::{base_client::AIClient, Provider};
use crate::session::message::{Message, Part, Role};
use crate::session::{SessionManager, SessionStatus};
use crate::task::{TaskManager, TaskPlan};
use crate::tools::set_mcp_manager;
use crate::utils::cli::Args;
use crate::utils::workspace::set_workspace;
use crate::{log_debug, log_info};

const SUBAGENT_SYSTEM_PROMPT: &str = r#"你是一个专注于执行特定子任务的 AI 助手。
你的任务是完成用户交给你的具体任务，不要偏离主题。
完成后，请用一段话总结你做了什么、结果是什么。
"#;

fn print_task_plan(plan: &crate::task::TaskPlan) {
    println!("\n📋 Task Plan ({} tasks):", plan.tasks.len());
    for task in &plan.tasks {
        let icon = match task.status {
            crate::task::TaskStatus::Pending => "[ ]",
            crate::task::TaskStatus::InProgress => "🔄",
            crate::task::TaskStatus::Completed => "✅",
            crate::task::TaskStatus::Failed => "❌",
        };
        println!("  {} {}", icon, task.name);
    }
    println!();
}

fn extract_task_plan_result(messages: &[Message]) -> Option<String> {
    for msg in messages.iter().rev() {
        if msg.role == Role::User {
            for part in &msg.parts {
                if let Part::ToolResult { content, .. } = part {
                    if let Ok(plan) = serde_json::from_str::<TaskPlan>(content) {
                        if !plan.tasks.is_empty() {
                            return Some(content.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

pub async fn run() -> Result<()> {
    let args = Args::parse();

    #[cfg(debug_assertions)]
    {
        use crate::utils::log::{set_log_level, LogLevel};
        set_log_level(LogLevel::from_str(&args.log_level));
        log_info!("fi-code starting | log_level={}", args.log_level);
    }

    // 设置工作目录：命令行参数 > 默认用户主目录
    let workspace = args
        .workspace
        .or_else(dirs::home_dir)
        .context("无法获取用户主目录")?;
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)
            .with_context(|| format!("无法创建工作目录: {:?}", workspace))?;
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("无法解析工作目录: {:?}", workspace))?;
    set_workspace(workspace.clone());
    crate::skills::init_skills();
    log_info!(
        "skills initialized | count={}",
        crate::skills::get_registry().entries.len()
    );

    log_info!(
        "fi-code started | mode={} | workspace={:?}",
        if args.interactive {
            "interactive"
        } else if args.command.is_some() {
            "command"
        } else if args.session.is_some() {
            "session"
        } else {
            "none"
        },
        workspace
    );

    let config_dir = directories::ProjectDirs::from("", "", "fi-code")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config/fi-code"));
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
                        if s.status == SessionStatus::Active {
                            "active"
                        } else {
                            "archived"
                        }
                    );
                }
            }
        }
        return Ok(());
    }

    let config = Arc::new(RwLock::new(Config::load()?));
    let _watcher = crate::config::config::spawn_watcher(Arc::clone(&config))?;

    // 初始化 MCP Manager
    {
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        if let Some(mcp_config) = &cfg.mcp {
            match McpManager::from_config(mcp_config).await {
                Ok(manager) => {
                    let status = manager.all_status().await;
                    log_info!("MCP initialized | servers={}", status.len());
                    for (name, st) in &status {
                        log_info!("MCP server status | {}={:?}", name, st);
                    }
                    set_mcp_manager(Arc::new(manager));
                }
                Err(e) => {
                    eprintln!("Warning: MCP initialization failed: {}", e);
                }
            }
        }
    }

    if args.models {
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        println!("Providers and Models:");
        for (provider_key, provider_cfg) in &cfg.provider {
            println!("  {} ({})", provider_key, provider_cfg.name);
            for (model_key, model_cfg) in &provider_cfg.models {
                println!("    {} — {}", model_key, model_cfg.name);
                println!(
                    "      context: {}, output: {}",
                    model_cfg.limit.context, model_cfg.limit.output
                );
            }
        }
        return Ok(());
    }

    // 如果没有显式指定操作模式，提示用户
    if !args.interactive && args.session.is_none() && args.command.is_none() {
        println!("Please provide an option. Use -h or --help for more information.");
        return Ok(());
    }

    let provider = Arc::new(Provider::new(Arc::clone(&config))?);

    // -c 单命令模式
    if let Some(cmd) = args.command {
        let cmd = cmd.trim();
        if !cmd.is_empty() {
            let mut session = session_manager.create_session(provider.model_name()?)?;
            run_single_command(
                Arc::clone(&provider),
                &session_manager,
                &sessions_dir,
                &mut session,
                cmd,
                Arc::clone(&config),
            )
            .await?;
        }
        return Ok(());
    }

    // -i 交互式模式
    run_interactive(
        Arc::clone(&provider),
        &session_manager,
        &sessions_dir,
        config,
    )
    .await?;
    Ok(())
}

async fn run_single_command(
    provider: Arc<Provider>,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut crate::session::Session,
    query: &str,
    config: Arc<RwLock<Config>>,
) -> Result<()> {
    log_debug!("run_single_command | query_len={}", query.len());

    // 拦截 slash 指令
    let slash_cmd = crate::commands::slash::parse(query);
    if !matches!(slash_cmd, crate::commands::slash::SlashCommand::Unknown(ref s) if s.is_empty()) {
        let provider_lock = Arc::new(std::sync::RwLock::new((*provider).clone()));
        let handler = crate::commands::slash::SlashCommandHandler::new(provider_lock, config);
        handler.execute(slash_cmd).await?;
        return Ok(());
    }

    let user_msg = Message::new(
        session.id.clone(),
        Role::User,
        vec![Part::Text {
            text: query.to_string(),
        }],
    );
    session.messages.push(user_msg.clone());
    let _ = session_manager.append_message(&session.id, &user_msg);

    let mut state = LoopState::new(session.messages.clone());
    let client = provider.get_client()?;
    agent_loop(client.as_ref(), &mut state).await?;

    handle_task_plan_and_save(
        provider,
        session_manager,
        sessions_dir,
        session,
        state,
        false,
    )
    .await
}

async fn handle_task_plan_and_save(
    provider: Arc<Provider>,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut crate::session::Session,
    mut state: LoopState,
    interactive: bool,
) -> Result<()> {
    if let Some(plan_json) = extract_task_plan_result(&state.messages) {
        let mut plan: TaskPlan = serde_json::from_str(&plan_json)
            .context("Failed to parse task plan from tool result")?;

        println!("\n📋 检测到任务计划，共 {} 个子任务", plan.tasks.len());
        print_task_plan(&plan);

        let provider_clone = provider.clone();
        let client_factory: Arc<dyn Fn() -> Box<dyn AIClient> + Send + Sync> =
            Arc::new(move || {
                provider_clone
                    .get_client()
                    .expect("Failed to create subagent client")
            });

        let subagent_schema = crate::tools::subagent_tool_schema().await;
        let task_manager = TaskManager::new(
            client_factory,
            SUBAGENT_SYSTEM_PROMPT.to_string(),
            subagent_schema,
        );

        let mut on_progress = |plan: &TaskPlan| {
            print_task_plan(plan);
        };

        let summaries = task_manager
            .execute_plan(&mut plan, &mut on_progress)
            .await?;

        let mut summary_text = "所有子任务已完成，结果汇总如下：\n\n".to_string();
        for (idx, summary) in summaries.iter().enumerate() {
            let task_name = &plan.tasks[idx].name;
            summary_text.push_str(&format!(
                "[任务 {}: {}]\n{}\n\n",
                idx + 1,
                task_name,
                summary.result
            ));
        }

        state.messages.push(Message::new(
            session.id.clone(),
            Role::User,
            vec![Part::Text { text: summary_text }],
        ));

        let client = provider.get_client()?;
        agent_loop(client.as_ref(), &mut state).await?;
    }

    session.messages = state.messages;

    if let Err(e) = tokio::task::spawn_blocking({
        let sm = SessionManager::new(sessions_dir.clone());
        let s = session.clone();
        move || sm.save_session(&s)
    })
    .await?
    {
        eprintln!("Warning: failed to save session: {}", e);
    }

    if let Some(last_msg) = session.messages.last() {
        if last_msg.role == Role::Assistant {
            let text = crate::provider::extract_text(&last_msg.parts);
            if !text.is_empty() {
                println!("{}", text);
            }
        }
        if interactive {
            println!();
        }
    }
    Ok(())
}

async fn run_interactive(
    provider: Arc<Provider>,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    config: Arc<RwLock<Config>>,
) -> Result<()> {
    let mut session = choose_or_create_session(session_manager, provider.model_name()?).await?;
    let prompt_prefix = format!("{} >> ", &session.id[..session.id.len().min(8)]);
    let mut editor = DefaultEditor::new()?;

    log_debug!("run_interactive | session_id={}", session.id);

    loop {
        let readline = editor.readline(prompt_prefix.cyan().to_string().as_str());
        match readline {
            Ok(line) => {
                let query = line.trim();
                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }
                editor.add_history_entry(query)?;

                // 拦截 slash 指令
                let slash_cmd = crate::commands::slash::parse(query);
                if !matches!(slash_cmd, crate::commands::slash::SlashCommand::Unknown(ref s) if s.is_empty())
                {
                    let provider_lock = Arc::new(std::sync::RwLock::new((*provider).clone()));
                    let handler = crate::commands::slash::SlashCommandHandler::new(
                        provider_lock,
                        Arc::clone(&config),
                    );
                    if let Err(e) = handler.execute(slash_cmd).await {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }

                let user_msg = Message::new(
                    session.id.clone(),
                    Role::User,
                    vec![Part::Text {
                        text: query.to_string(),
                    }],
                );
                session.messages.push(user_msg.clone());
                if let Err(e) = session_manager.append_message(&session.id, &user_msg) {
                    eprintln!("Warning: failed to persist user message: {}", e);
                }

                let mut state = LoopState::new(session.messages.clone());
                let client = provider.get_client()?;
                agent_loop(client.as_ref(), &mut state).await?;

                handle_task_plan_and_save(
                    Arc::clone(&provider),
                    session_manager,
                    sessions_dir,
                    &mut session,
                    state,
                    true,
                )
                .await?;
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

async fn choose_or_create_session(
    manager: &SessionManager,
    model_name: &str,
) -> Result<crate::session::Session> {
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
