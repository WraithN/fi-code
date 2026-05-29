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

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use colored::Colorize;

use crate::cli_args::{Args, Commands};
use fi_code_core::agent::AgentType;
use fi_code_core::agent::{agent_loop, LoopState};
use fi_code_core::commands::slash::{SlashCommand, SlashCommandHandler};
use fi_code_core::config::Config;
use fi_code_core::mcp::manager::McpManager;
use fi_code_core::provider::Provider;
use fi_code_core::session::message::{Message, Part, Role};
use fi_code_core::session::{SessionManager, SessionStatus};
use fi_code_core::tools::set_mcp_manager;
use fi_code_core::utils::workspace::set_workspace;
use fi_code_core::{log_debug, log_info};

/// 入口执行结果：正常结束或需要启动 TUI
#[derive(Debug)]
pub enum EntryOutcome {
    Completed,
    StartTui { port: Option<u16> },
}

async fn start_web_mode(port: u16) -> anyhow::Result<EntryOutcome> {
    let config = Arc::new(std::sync::RwLock::new(fi_code_core::config::Config::load()?));
    let _watcher = fi_code_core::config::config::spawn_watcher(Arc::clone(&config))?;
    {
        let cfg = config.read().map_err(|_| anyhow::anyhow!("配置锁中毒"))?;
        let extra = cfg.skills.as_ref().map(|s| s.directories.as_slice());
        fi_code_core::skills::init_skills(extra);
    }

    // 初始化 MCP Manager
    {
        let cfg = config.read().map_err(|_| anyhow::anyhow!("配置锁中毒"))?;
        if let Some(mcp_config) = &cfg.mcp {
            match fi_code_core::mcp::manager::McpManager::from_config(mcp_config).await {
                Ok(manager) => {
                    fi_code_core::tools::set_mcp_manager(std::sync::Arc::new(manager));
                }
                Err(e) => {
                    eprintln!("Warning: MCP initialization failed: {}", e);
                }
            }
        }
    }

    let provider = fi_code_core::provider::Provider::new(Arc::clone(&config))?;
    fi_code_core::tools::set_task_provider(Arc::new(std::sync::RwLock::new(provider.clone())));
    let provider = Arc::new(std::sync::RwLock::new(provider));

    // 启动 Server
    let server = fi_code_core::server::Server::new(provider, config, Some(port));

    // 打开浏览器
    let url = format!("http://localhost:{}", port);
    if let Err(e) = open::that(&url) {
        eprintln!("Warning: failed to open browser: {}", e);
        println!("Please open {} manually", url);
    } else {
        println!("Opening browser at {} ...", url);
    }

    server.run().await;
    Ok(EntryOutcome::Completed)
}

pub async fn run() -> Result<EntryOutcome> {
    let args = Args::parse();

    // 设置 CLI 危险模式标志（影响权限检查行为）
    fi_code_core::permission::set_cli_dangerous(args.dangerous);

    // -W / --web 模式优先级高于默认 TUI
    if let Some(port_opt) = args.web {
        let port = port_opt.unwrap_or(4040);
        return start_web_mode(port).await;
    }

    // 如果指定了子命令
    match args.command {
        Some(Commands::Server { port }) => {
            let config = Arc::new(RwLock::new(Config::load()?));
            {
                let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
                let extra = cfg.skills.as_ref().map(|s| s.directories.as_slice());
                fi_code_core::skills::init_skills(extra);
            }
            let provider = Arc::new(RwLock::new(Provider::new(Arc::clone(&config))?));
            fi_code_core::server::Server::new(provider, config, port)
                .run()
                .await;
            return Ok(EntryOutcome::Completed);
        }
        Some(Commands::Logs {
            limit,
            follow: _,
            session,
            tool: _,
            raw: _,
        }) => {
            use fi_code_core::observability::cli_view::{run_logs_cli, LogsOptions};
            let options = LogsOptions {
                file: None,
                session,
                limit: Some(limit),
            };
            run_logs_cli(options)?;
            return Ok(EntryOutcome::Completed);
        }
        None => {
            // 检查是否有其他向后兼容的 flag
            if args.interactive || args.cmd.is_some() || args.session.is_some() || args.models {
                // 继续原有 CLI 逻辑（什么都不做，继续往下执行）
            } else {
                // 默认启动 TUI 模式，由调用方（cli crate）负责启动
                return Ok(EntryOutcome::StartTui { port: None });
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        use fi_code_core::utils::log::{set_log_level, LogLevel};
        set_log_level(LogLevel::from_str(&args.log_level));
        log_info!("fi-code starting | log_level={}", args.log_level);
    }

    // 设置工作目录：命令行参数 > 默认当前工作目录
    let workspace = args
        .workspace
        .or_else(|| std::env::current_dir().ok())
        .context("无法获取当前工作目录")?;
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)
            .with_context(|| format!("无法创建工作目录: {:?}", workspace))?;
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("无法解析工作目录: {:?}", workspace))?;
    set_workspace(workspace.clone());

    // 先加载 config，再用 config 中的自定义目录初始化 skills
    let config = Arc::new(RwLock::new(Config::load()?));
    let _watcher = fi_code_core::config::config::spawn_watcher(Arc::clone(&config))?;
    {
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let extra = cfg.skills.as_ref().map(|s| s.directories.as_slice());
        fi_code_core::skills::init_skills(extra);
    }
    log_info!(
        "skills initialized | count={}",
        fi_code_core::skills::get_registry().entries.len()
    );

    log_info!(
        "fi-code started | mode={} | workspace={:?}",
        if args.interactive {
            "interactive"
        } else if args.cmd.is_some() {
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
        handle_session_arg(session_arg, &session_manager)?;
        return Ok(EntryOutcome::Completed);
    }

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
                if let Some(limit) = &model_cfg.limit {
                    println!("      context: {}, output: {}", limit.context, limit.output);
                }
            }
        }
        return Ok(EntryOutcome::Completed);
    }

    let provider = Arc::new(Provider::new(Arc::clone(&config))?);
    fi_code_core::tools::set_task_provider(Arc::new(RwLock::new((*provider).clone())));

    // 解析 Agent 类型
    let agent_type = match args.agent.as_str() {
        "plan" => AgentType::Plan,
        _ => AgentType::Build,
    };

    // -c 单命令模式
    if let Some(cmd) = args.cmd {
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
                agent_type,
            )
            .await?;
        }
        return Ok(EntryOutcome::Completed);
    }

    // -i 交互式模式
    run_interactive(
        Arc::clone(&provider),
        &session_manager,
        &sessions_dir,
        config,
        agent_type,
    )
    .await?;
    Ok(EntryOutcome::Completed)
}

fn print_sessions_list(session_manager: &SessionManager) -> Result<()> {
    let sessions = session_manager.list_sessions()?;
    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }
    println!("Recent sessions:");
    for (i, s) in sessions.iter().enumerate() {
        let status = if s.status == SessionStatus::Active {
            "active"
        } else {
            "archived"
        };
        println!(
            "  [{}] {} | {} | {} messages | {}",
            i + 1,
            &s.id[..s.id.len().min(8)],
            s.project_path,
            s.message_count,
            status
        );
    }
    Ok(())
}

fn handle_session_arg(session_arg: Option<String>, session_manager: &SessionManager) -> Result<()> {
    if let Some(selector) = session_arg {
        let session = session_manager.find_session(&selector)?;
        SessionManager::print_session(&session);
    } else {
        print_sessions_list(session_manager)?;
    }
    Ok(())
}

async fn run_single_command(
    provider: Arc<Provider>,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut fi_code_core::session::Session,
    query: &str,
    config: Arc<RwLock<Config>>,
    agent_type: AgentType,
) -> Result<()> {
    log_debug!("run_single_command | query_len={}", query.len());

    // 拦截 slash 指令
    let slash_cmd = fi_code_core::commands::slash::parse(query);
    if !matches!(slash_cmd, SlashCommand::Unknown(ref s) if s.is_empty()) {
        let provider_lock = Arc::new(std::sync::RwLock::new((*provider).clone()));
        let handler = SlashCommandHandler::new(provider_lock, config);
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
    agent_loop(
        client.as_ref(),
        &mut state,
        agent_type,
        &mut None,
        &mut None,
        None,
        None,
    )
    .await?;

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
    _provider: Arc<Provider>,
    _session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut fi_code_core::session::Session,
    state: LoopState,
    interactive: bool,
) -> Result<()> {
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
            let text = fi_code_core::provider::extract_text(&last_msg.parts);
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
    agent_type: AgentType,
) -> Result<()> {
    fi_code_core::tools::set_task_provider(Arc::new(RwLock::new((*provider).clone())));
    let mut session = choose_or_create_session(session_manager, provider.model_name()?).await?;
    let prompt_prefix = format!("{} >> ", &session.id[..session.id.len().min(8)]);
    let mut history = Vec::new();

    log_debug!("run_interactive | session_id={}", session.id);

    loop {
        print!("{}", prompt_prefix.cyan());
        std::io::stdout().flush()?;

        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                let query = input.trim();
                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }
                history.push(query.to_string());

                // 拦截 slash 指令
                if try_execute_slash(query, &provider, &config).await {
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
                agent_loop(
                    client.as_ref(),
                    &mut state,
                    agent_type,
                    &mut None,
                    &mut None,
                    None,
                    None,
                )
                .await?;

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
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => break,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn try_execute_slash(
    query: &str,
    provider: &Arc<Provider>,
    config: &Arc<RwLock<Config>>,
) -> bool {
    let slash_cmd = fi_code_core::commands::slash::parse(query);
    let is_unknown = matches!(slash_cmd, SlashCommand::Unknown(ref s) if s.is_empty());
    if is_unknown {
        return false;
    }
    let provider_lock = Arc::new(std::sync::RwLock::new((**provider).clone()));
    let handler = SlashCommandHandler::new(provider_lock, Arc::clone(config));
    if let Err(e) = handler.execute(slash_cmd).await {
        eprintln!("Error: {}", e);
    }
    true
}

async fn choose_or_create_session(
    manager: &SessionManager,
    model_name: &str,
) -> Result<fi_code_core::session::Session> {
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
        Ok(manager.load_session(&sessions[0].id)?)
    }
}
