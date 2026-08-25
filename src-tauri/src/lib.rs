use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub task_id: String,
    pub progress: f64,
    pub speed: f64,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_device_info() -> DeviceInfo {
    let hostname = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_else(|_| "Unknown".into());
    DeviceInfo {
        id: "local".into(),
        name: hostname,
        ip: local_ip_address::local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into()),
        platform: std::env::consts::OS.into(),
    }
}

#[tauri::command]
async fn discover_devices() -> Vec<DeviceInfo> {
    // TODO: mDNS discovery via mdns-sd
    vec![]
}

#[tauri::command]
async fn send_file(path: String, target_ip: String) -> Result<String, String> {
    // TODO: implement file transfer
    Ok(format!("Queued {} to {}", path, target_ip))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_device_info, discover_devices, send_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
