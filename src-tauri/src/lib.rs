mod discovery;
mod transfer;

use discovery::DeviceInfo;
use tauri::Manager;
use std::{path::PathBuf, sync::OnceLock};

static MDNS_DAEMON: OnceLock<mdns_sd::ServiceDaemon> = OnceLock::new();

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_device_info() -> DeviceInfo {
    discovery::get_device_info()
}

#[tauri::command]
async fn discover_devices() -> Vec<DeviceInfo> {
    discovery::discover_devices(2000).await
}

#[tauri::command]
async fn send_file(
    app: tauri::AppHandle,
    path: String,
    target_ip: String,
    target_port: Option<u16>,
    task_id: Option<String>,
) -> Result<String, String> {
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    transfer::send_file(path, target_ip, port, task_id, app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Register mDNS service
            match discovery::register_mdns() {
                Ok(daemon) => {
                    let _ = MDNS_DAEMON.set(daemon);
                    println!("mDNS registered");
                }
                Err(e) => eprintln!("mDNS register failed: {e}"),
            }
            // Start file server
            let handle = app.handle().clone();
            let mut save_dir = dirs::download_dir()
                .or_else(|| app.path().download_dir().ok())
                .or_else(|| app.path().app_data_dir().ok())
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("FlashLAN");
            if let Err(e) = std::fs::create_dir_all(&save_dir) {
                eprintln!("Failed to create save_dir {:?}: {}", save_dir, e);
                if let Ok(fallback) = app.path().app_data_dir() {
                    let fb = fallback.join("FlashLAN");
                    let _ = std::fs::create_dir_all(&fb);
                    println!("Fallback save_dir: {:?}", fb);
                    save_dir = fb;
                }
            } else {
                println!("FlashLAN save_dir: {:?}", save_dir);
            }
            tauri::async_runtime::spawn(async move {
                if let Err(e) = transfer::start_file_server(handle, save_dir).await {
                    eprintln!("file server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet, get_device_info, discover_devices, send_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
