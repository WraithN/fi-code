use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "shun-code", version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    /// Enable debug logging (debug|trace|info|off, default: info)
    #[cfg(debug_assertions)]
    #[arg(
        short = 'l',
        long = "log",
        value_name = "LEVEL",
        default_value = "info"
    )]
    pub log_level: String,

    /// Enter interactive REPL mode
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Print session information and exit
    #[arg(short = 's', long = "session", value_name = "SESSION", num_args = 0..=1)]
    pub session: Option<Option<String>>,

    /// Execute a single command and exit
    #[arg(short = 'c', long = "command", value_name = "MESSAGE")]
    pub command: Option<String>,

    /// Workspace directory (default: home directory)
    #[arg(short = 'w', long = "workspace", value_name = "PATH")]
    pub workspace: Option<PathBuf>,
}
