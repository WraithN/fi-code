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

use crate::utils::log_store::LogBroadcaster;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Off = 0,
    Info = 1,
    Debug = 2,
    Trace = 3,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => LogLevel::Off,
            "debug" => LogLevel::Debug,
            "trace" => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }

    pub fn enabled(self, required: LogLevel) -> bool {
        self >= required
    }
}

static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn current_log_level() -> LogLevel {
    match LOG_LEVEL.load(Ordering::Relaxed) {
        0 => LogLevel::Off,
        2 => LogLevel::Debug,
        3 => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

pub fn log_prefix(level: &str, module: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("{} [{:<5}] [{:<30.30}]", now, level, module)
}

static GLOBAL_LOG_BROADCASTER: OnceLock<Arc<LogBroadcaster>> = OnceLock::new();

pub fn set_global_log_broadcaster(b: Arc<LogBroadcaster>) {
    let _ = GLOBAL_LOG_BROADCASTER.set(b);
}

pub fn send_log(level: &str, module: &str, message: String) {
    if let Some(broadcaster) = GLOBAL_LOG_BROADCASTER.get() {
        broadcaster.send(level, module, message);
    } else {
        let prefix = log_prefix(level, module);
        eprintln!("{} {}", prefix, message);
    }
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Info) {
                let msg = format!($($arg)*);
                $crate::utils::log::send_log("INFO", module_path!(), msg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Debug) {
                let msg = format!($($arg)*);
                $crate::utils::log::send_log("DEBUG", module_path!(), msg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Trace) {
                let msg = format!($($arg)*);
                $crate::utils::log::send_log("TRACE", module_path!(), msg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Info) {
                let msg = format!($($arg)*);
                $crate::utils::log::send_log("ERROR", module_path!(), msg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Info) {
                let msg = format!($($arg)*);
                $crate::utils::log::send_log("WARN", module_path!(), msg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_block {
    ($level:expr, $title:expr, $content:expr) => {
        #[cfg(debug_assertions)]
        {
            let __level = $level;
            let enabled = $crate::utils::log::current_log_level().enabled(__level);
            if enabled {
                let prefix = $crate::utils::log::log_prefix(
                    match __level {
                        $crate::utils::log::LogLevel::Debug => "DEBUG",
                        $crate::utils::log::LogLevel::Trace => "TRACE",
                        _ => "INFO",
                    },
                    module_path!(),
                );
                let sep_width = 50;
                eprintln!("{} {:=^sep_width$}", prefix, format!(" {} ", $title));
                for line in $content.lines() {
                    eprintln!("{} {}", prefix, line);
                }
                eprintln!("{} {:=^sep_width$}", prefix, "");
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("off"), LogLevel::Off);
        assert_eq!(LogLevel::from_str("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("trace"), LogLevel::Trace);
    }

    #[test]
    fn test_log_level_enabled() {
        assert!(LogLevel::Debug.enabled(LogLevel::Debug));
        assert!(LogLevel::Trace.enabled(LogLevel::Debug));
        assert!(!LogLevel::Info.enabled(LogLevel::Debug));
    }

    #[test]
    fn test_atomic_set_and_get() {
        set_log_level(LogLevel::Trace);
        assert_eq!(current_log_level(), LogLevel::Trace);
        set_log_level(LogLevel::Info);
        assert_eq!(current_log_level(), LogLevel::Info);
    }
}
