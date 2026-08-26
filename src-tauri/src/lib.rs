mod discovery;
mod transfer;

use discovery::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    device_name: String,
    save_path: String,
}

#[derive(Clone)]
struct SettingsState {
    path: PathBuf,
    settings: Arc<Mutex<AppSettings>>,
}

impl SettingsState {
    fn current(&self) -> Result<AppSettings, String> {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .map_err(|_| "settings state is unavailable".to_string())
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_device_info(settings: State<'_, SettingsState>) -> Result<DeviceInfo, String> {
    let settings = settings.current()?;
    Ok(discovery::get_device_info(&settings.device_name))
}

#[tauri::command]
fn get_settings(settings: State<'_, SettingsState>) -> Result<AppSettings, String> {
    settings.current()
}

#[tauri::command]
fn set_device_name(name: String, settings: State<'_, SettingsState>) -> Result<(), String> {
    let name = validate_device_name(&name)?;
    let mut current = settings.current()?;
    current.device_name = name.clone();
    save_settings(&settings.path, &current)?;
    replace_settings(&settings, current)?;
    discovery::update_mdns(&name)
}

#[tauri::command]
fn set_save_path(
    path: String,
    settings: State<'_, SettingsState>,
    transfer_manager: State<'_, transfer::TransferManager>,
) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("保存路径不能为空".to_string());
    }
    let path = PathBuf::from(path);
    fs::create_dir_all(&path).map_err(|error| format!("无法创建保存目录：{error}"))?;
    if !path.is_dir() {
        return Err("保存路径不是文件夹".to_string());
    }

    let mut current = settings.current()?;
    current.save_path = path.to_string_lossy().to_string();
    save_settings(&settings.path, &current)?;
    replace_settings(&settings, current)?;
    transfer_manager.set_save_dir(path);
    Ok(())
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
            let config_path = app
                .path()
                .app_config_dir()
                .or_else(|_| app.path().app_data_dir())
                .unwrap_or_else(|_| std::env::temp_dir().join("FlashLAN"))
                .join("settings.json");
            let mut settings = load_settings(&config_path, app.handle());
            let mut save_dir = PathBuf::from(&settings.save_path);
            if let Err(error) = fs::create_dir_all(&save_dir).and_then(|_| {
                if save_dir.is_dir() {
                    Ok(())
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "configured save path is not a directory",
                    ))
                }
            }) {
                eprintln!(
                    "Failed to use configured save_dir {:?}: {}",
                    save_dir, error
                );
                save_dir = default_save_dir(app.handle());
                let _ = fs::create_dir_all(&save_dir);
                settings.save_path = save_dir.to_string_lossy().to_string();
                if let Err(error) = save_settings(&config_path, &settings) {
                    eprintln!("Failed to persist fallback settings: {error}");
                }
            }
            app.manage(SettingsState {
                path: config_path,
                settings: Arc::new(Mutex::new(settings.clone())),
            });
            let transfer_manager = app.state::<transfer::TransferManager>().inner().clone();
            transfer_manager.set_save_dir(save_dir);

            // Register mDNS service
            match discovery::register_mdns(&settings.device_name) {
                Ok(daemon) => {
                    let _ = discovery::set_mdns_daemon(daemon);
                    println!("mDNS registered");
                }
                Err(e) => eprintln!("mDNS register failed: {e}"),
            }
            // Start file server. Android receives through MediaStore into the
            // public Download/FlashLAN directory; this private path is only a
            // fallback for older devices or a failed MediaStore operation.
            let handle = app.handle().clone();
            println!("FlashLAN save_dir: {:?}", transfer_manager.save_dir());
            tauri::async_runtime::spawn(async move {
                if let Err(e) = transfer::start_file_server(handle, transfer_manager).await {
                    eprintln!("file server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_device_info,
            get_settings,
            set_device_name,
            set_save_path,
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

fn default_save_dir(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(target_os = "android")]
    {
        return app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join("Download")
            .join("FlashLAN");
    }

    dirs::download_dir()
        .or_else(|| app.path().download_dir().ok())
        .or_else(|| app.path().app_data_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("FlashLAN")
}

fn load_settings(path: &Path, app: &tauri::AppHandle) -> AppSettings {
    let default = AppSettings {
        device_name: discovery::default_device_name(),
        save_path: default_save_dir(app).to_string_lossy().to_string(),
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return default;
    };
    let Ok(mut settings) = serde_json::from_str::<AppSettings>(&contents) else {
        return default;
    };
    if validate_device_name(&settings.device_name).is_err() {
        settings.device_name = default.device_name;
    }
    if settings.save_path.trim().is_empty() {
        settings.save_path = default.save_path;
    }
    settings
}

fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建配置目录：{error}"))?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| format!("无法保存设置：{error}"))
}

fn replace_settings(state: &SettingsState, settings: AppSettings) -> Result<(), String> {
    *state
        .settings
        .lock()
        .map_err(|_| "settings state is unavailable".to_string())? = settings;
    Ok(())
}

fn validate_device_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("设备名称不能为空".to_string());
    }
    if name.chars().count() > 64 {
        return Err("设备名称不能超过 64 个字符".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("设备名称不能包含控制字符".to_string());
    }
    Ok(name.to_string())
}
