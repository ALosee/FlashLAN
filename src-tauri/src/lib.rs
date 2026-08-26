mod discovery;
mod transfer;

use discovery::DeviceInfo;
use std::{path::PathBuf, sync::OnceLock};
use tauri::Manager;

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
async fn test_connection(target_ip: String, target_port: Option<u16>) -> Result<(), String> {
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    transfer::test_connection(target_ip, port).await
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

#[tauri::command]
fn respond_transfer_request(
    task_id: String,
    accepted: bool,
    transfer_manager: tauri::State<'_, transfer::TransferManager>,
) -> Result<(), String> {
    transfer_manager.respond(&task_id, accepted)
}

#[tauri::command]
fn get_pending_transfer_requests(
    transfer_manager: tauri::State<'_, transfer::TransferManager>,
) -> Result<Vec<transfer::TransferRequest>, String> {
    transfer_manager.pending_requests()
}

#[tauri::command]
fn set_auto_receive(
    enabled: bool,
    transfer_manager: tauri::State<'_, transfer::TransferManager>,
) -> Result<(), String> {
    transfer_manager.set_auto_receive(enabled);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(transfer::TransferManager::default())
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
            // Start file server. Android receives through MediaStore into the
            // public Download/FlashLAN directory; this private path is only a
            // fallback for older devices or a failed MediaStore operation.
            let handle = app.handle().clone();
            #[cfg(target_os = "android")]
            let mut save_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join("Download")
                .join("FlashLAN");
            #[cfg(not(target_os = "android"))]
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
            let transfer_manager = app.state::<transfer::TransferManager>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    transfer::start_file_server(handle, save_dir, transfer_manager).await
                {
                    eprintln!("file server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_device_info,
            discover_devices,
            test_connection,
            send_file,
            respond_transfer_request,
            get_pending_transfer_requests,
            set_auto_receive
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
