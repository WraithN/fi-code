use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::net::TcpStream;
use tauri::AppHandle;
use std::thread;

pub struct SidecarManager {
    process: Option<Child>,
    pub port: u16,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: None,
            port: 4040,
        }
    }

    pub fn start(&mut self, _app_handle: &AppHandle) -> Result<(), String> {
        let binary_path = self.locate_binary()?;

        let mut cmd = Command::new(&binary_path);
        cmd.arg("--server")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn sidecar: {}", e))?;
        self.process = Some(child);
        Ok(())
    }

    fn locate_binary(&self) -> Result<std::path::PathBuf, String> {
        let dev_path = std::path::PathBuf::from("../target/debug/fi-code");
        if dev_path.exists() {
            return Ok(dev_path);
        }
        Err("Sidecar binary not found. Please build fi-code first: cargo build".to_string())
    }

    pub fn wait_ready(&self, timeout_secs: u64) -> Result<(), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let start = std::time::Instant::now();

        while start.elapsed().as_secs() < timeout_secs {
            if TcpStream::connect_timeout(
                &addr.parse().unwrap(),
                Duration::from_secs(1),
            ).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(500));
        }

        Err(format!("Sidecar not ready after {}s", timeout_secs))
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
    }

    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
