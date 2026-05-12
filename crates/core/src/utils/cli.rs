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

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "fi-code", version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

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
    pub cmd: Option<String>,

    /// Show configured providers and models
    #[arg(short = 'm', long = "models")]
    pub models: bool,

    /// Workspace directory (default: home directory)
    #[arg(short = 'w', long = "workspace", value_name = "PATH")]
    pub workspace: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the web server
    Server {
        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,
    },
}
