use std::sync::atomic::{AtomicU8, Ordering};

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
    format!("{} [{:<5}] [{:<30}]", now, level, module)
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::utils::log::current_log_level().enabled($crate::utils::log::LogLevel::Info) {
                eprintln!("{} {}", $crate::utils::log::log_prefix("INFO", module_path!()), format!($($arg)*));
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
                eprintln!("{} {}", $crate::utils::log::log_prefix("DEBUG", module_path!()), format!($($arg)*));
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
                eprintln!("{} {}", $crate::utils::log::log_prefix("TRACE", module_path!()), format!($($arg)*));
            }
        }
    };
}

#[macro_export]
macro_rules! log_block {
    ($level:expr, $title:expr, $content:expr) => {
        #[cfg(debug_assertions)]
        {
            let enabled = $crate::utils::log::current_log_level().enabled($level);
            if enabled {
                let prefix = $crate::utils::log::log_prefix(
                    match $level {
                        $crate::utils::log::LogLevel::Debug => "DEBUG",
                        $crate::utils::log::LogLevel::Trace => "TRACE",
                        _ => "INFO",
                    },
                    module_path!()
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
}
