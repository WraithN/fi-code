use std::sync::Mutex;

pub mod sidecar;
use sidecar::SidecarManager;

#[tauri::command]
async fn start_sidecar(state: tauri::State<'_, Mutex<SidecarManager>>, app: tauri::AppHandle) -> Result<String, String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    manager.start(&app)?;
    manager.wait_ready(10)?;
    Ok(format!("http://127.0.0.1:{}", manager.port))
}

#[tauri::command]
async fn stop_sidecar(state: tauri::State<'_, Mutex<SidecarManager>>) -> Result<(), String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    manager.stop();
    Ok(())
}

#[tauri::command]
fn get_sidecar_status(state: tauri::State<'_, Mutex<SidecarManager>>) -> Result<bool, String> {
    let manager = state.lock().map_err(|e| e.to_string())?;
    Ok(manager.is_running())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(SidecarManager::new()))
        .invoke_handler(tauri::generate_handler![
            start_sidecar,
            stop_sidecar,
            get_sidecar_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
