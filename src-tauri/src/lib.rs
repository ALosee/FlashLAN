mod discovery;
mod transfer;

use discovery::DeviceInfo;

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
async fn send_file(app: tauri::AppHandle, path: String, target_ip: String, target_port: Option<u16>) -> Result<String, String> {
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    transfer::send_file(path, target_ip, port, app).await
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
            let save_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("FlashLAN");
            let _ = std::fs::create_dir_all(&save_dir);
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
