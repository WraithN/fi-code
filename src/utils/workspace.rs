use std::path::PathBuf;
use std::sync::Mutex;

// =============================================================================
// 全局工作目录配置
// =============================================================================
// 通过 Mutex 包装，允许在程序启动时设置一次，也便于测试中动态调整。

static WORKSPACE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 设置全局工作目录。
pub fn set_workspace(path: PathBuf) {
    let mut guard = WORKSPACE.lock().unwrap();
    *guard = Some(path);
}

/// 获取当前配置的工作目录。
/// - 如果已调用 `set_workspace`，返回设置的目录
/// - 否则默认返回用户主目录
pub fn workspace_dir() -> PathBuf {
    WORKSPACE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| dirs::home_dir().expect("无法获取用户主目录"))
}
