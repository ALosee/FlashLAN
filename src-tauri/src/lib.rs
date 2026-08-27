mod discovery;
mod security;
mod transfer;

use discovery::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};

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
async fn verify_peer_fingerprint(
    target_ip: String,
    target_port: Option<u16>,
    fingerprint: String,
    identity: tauri::State<'_, security::Identity>,
) -> Result<(), String> {
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    transfer::verify_peer_fingerprint(
        target_ip,
        port,
        fingerprint,
        identity.client_connector()?,
        identity.fingerprint.clone(),
    )
    .await
}

#[tauri::command]
async fn send_file(
    app: tauri::AppHandle,
    path: serde_json::Value,
    target_ip: String,
    target_port: Option<u16>,
    task_id: Option<String>,
    identity: tauri::State<'_, security::Identity>,
) -> Result<String, String> {
    // Accept both a single string (older callers) and an array of strings so
    // multi-selection flows through one session / one confirmation dialog.
    let paths: Vec<String> = match path {
        serde_json::Value::String(single) => vec![single],
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => return Err("path 参数格式不正确".into()),
    };
    if paths.is_empty() {
        return Err("没有可发送的文件".into());
    }
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    let security = transfer::SendSecurity {
        connector: identity.client_connector()?,
        fingerprint: identity.fingerprint.clone(),
    };
    transfer::send_file(paths, target_ip, port, task_id, app, security).await
}

// ---------- Pairing / trust commands ----------

/// Fingerprint of this device's certificate (SHA-256 hex).
#[tauri::command]
fn get_own_fingerprint(identity: tauri::State<'_, security::Identity>) -> String {
    identity.fingerprint.clone()
}

/// List of paired (trusted) devices.
#[tauri::command]
fn list_trusted_devices(
    trust: tauri::State<'_, Arc<security::TrustStore>>,
) -> Vec<(String, security::TrustedDevice)> {
    trust.list()
}

/// Remove a paired device.
#[tauri::command]
fn remove_trusted_device(
    fingerprint: String,
    trust: tauri::State<'_, Arc<security::TrustStore>>,
) -> Result<(), String> {
    trust.remove(&fingerprint)
}

/// Complete pairing with a peer that has shown its PIN: store its
/// fingerprint under the given display name.
#[tauri::command]
fn trust_device(
    fingerprint: String,
    name: String,
    trust: tauri::State<'_, Arc<security::TrustStore>>,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("设备名称不能为空".to_string());
    }
    trust.add(
        fingerprint.to_lowercase(),
        security::TrustedDevice {
            name: name.to_string(),
            paired_at: now_millis(),
        },
    )
}

/// Share text with a peer over the encrypted channel (fire-and-forget).
#[tauri::command]
async fn send_text(
    app: tauri::AppHandle,
    text: String,
    target_ip: String,
    target_port: Option<u16>,
    identity: tauri::State<'_, security::Identity>,
) -> Result<(), String> {
    let port = target_port.unwrap_or(discovery::SERVICE_PORT);
    let security = transfer::SendSecurity {
        connector: identity.client_connector()?,
        fingerprint: identity.fingerprint.clone(),
    };
    transfer::send_text(text, target_ip, port, app, security).await
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// QR payload: `flashlan://<ip>:<port>#<fingerprint>` rendered as an SVG.
/// The scanning side connects to ip:port and verifies the fingerprint the
/// peer presents in the TLS banner, preventing MITM without manual typing.
#[tauri::command]
async fn generate_connect_qr(
    identity: tauri::State<'_, security::Identity>,
    settings: tauri::State<'_, SettingsState>,
) -> Result<String, String> {
    let info = discovery::get_device_info(&settings.current()?.device_name);
    let url = format!(
        "flashlan://{}:{}#{}",
        info.ip, info.port, identity.fingerprint
    );
    use qrcode::{render::svg, QrCode};
    let code = QrCode::with_error_correction_level(url.as_bytes(), qrcode::EcLevel::M)
        .map_err(|error| format!("二维码生成失败：{error}"))?;
    let svg = code
        .render::<svg::Color>()
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build();
    Ok(svg)
}

#[tauri::command]
fn open_file_location(
    app: tauri::AppHandle,
    path: String,
    file_name: String,
) -> Result<(), String> {
    transfer::open_file_location(&app, &path, &file_name)
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

#[tauri::command]
fn cancel_transfer(task_id: String) {
    transfer::request_cancel(&task_id);
}

/// Whether the TCP file server bound its port successfully at startup.
#[tauri::command]
fn get_server_status(server_listening: tauri::State<'_, ServerListening>) -> bool {
    server_listening
        .0
        .load(std::sync::atomic::Ordering::Relaxed)
}

/// Persist pasted text as a temp .txt file so it can go through the normal
/// file send path (Ctrl+V "paste text as file").
#[tauri::command]
fn create_text_clipboard_file(text: String) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("剪贴板内容为空".to_string());
    }
    if trimmed.len() > 1024 * 1024 {
        return Err("粘贴内容过大（超过 1MB）".to_string());
    }
    let target = std::env::temp_dir().join(format!("FlashLAN-{}.txt", chrono_like_timestamp()));
    fs::write(&target, trimmed).map_err(|error| format!("无法写入临时文件：{error}"))?;
    Ok(target.to_string_lossy().to_string())
}

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_millis())
}

#[derive(Default)]
pub struct ServerListening(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app: &tauri::AppHandle, _args: Vec<String>, _cwd: String| {
                // Second launch: focus the existing window instead.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            },
        ));
    }

    builder = builder
        .manage(transfer::TransferManager::default())
        .manage(ServerListening::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());

    builder
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
            let listening_flag = app.state::<ServerListening>().0.clone();

            // Security: per-device self-signed certificate + trust store.
            let identity = match security::Identity::load_or_create(
                &app.path()
                    .app_config_dir()
                    .unwrap_or_else(|_| std::env::temp_dir().join("FlashLAN"))
                    .join("keys"),
            ) {
                Ok(identity) => identity,
                Err(error) => {
                    eprintln!("identity init failed: {error}");
                    return Err(error.into());
                }
            };
            let trust = Arc::new(security::TrustStore::new());
            trust.bind_path(
                app.path()
                    .app_config_dir()
                    .unwrap_or_else(|_| std::env::temp_dir().join("FlashLAN"))
                    .join("trusted-devices.json"),
            );
            app.manage(trust.clone());
            // Managed as bare Identity so State<Identity> in commands resolves;
            // capture fingerprint/TLS acceptor before ownership moves in.
            let fingerprint = identity.fingerprint.clone();
            let tls_acceptor = identity.server_acceptor()?;
            app.manage(identity);

            tauri::async_runtime::spawn(async move {
                match transfer::start_file_server(
                    handle.clone(),
                    transfer_manager,
                    tls_acceptor,
                    fingerprint,
                    trust,
                )
                .await
                {
                    Ok(()) => {
                        listening_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = handle.emit("server_status", (true, String::new()));
                    }
                    Err(e) => {
                        eprintln!("file server failed: {e}");
                        let _ = handle.emit("server_status", (false, e));
                    }
                }
            });

            #[cfg(desktop)]
            {
                setup_tray(app.handle())?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Desktop: closing the window keeps FlashLAN receiving in the
            // tray; use the tray menu's quit to fully exit.
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
            #[cfg(not(desktop))]
            let _ = (window, event);
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_device_info,
            get_settings,
            set_device_name,
            set_save_path,
            discover_devices,
            test_connection,
            verify_peer_fingerprint,
            send_file,
            open_file_location,
            respond_transfer_request,
            get_pending_transfer_requests,
            set_auto_receive,
            cancel_transfer,
            get_server_status,
            create_text_clipboard_file,
            get_own_fingerprint,
            list_trusted_devices,
            remove_trusted_device,
            trust_device,
            generate_connect_qr,
            send_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Desktop tray: quick access to show/hide the window and quit.
#[cfg(desktop)]
fn setup_tray(app: &tauri::AppHandle) -> Result<(), tauri::Error> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{TrayIconBuilder, TrayIconEvent},
    };

    let open_item = MenuItem::with_id(app, "open", "打开 FlashLAN", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &quit_item])?;

    TrayIconBuilder::with_id("flashlan-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("FlashLAN - 局域网快传")
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?,
        )
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    button_state: tauri::tray::MouseButtonState::Up,
                    ..
                }
            ) {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
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

    #[cfg(not(target_os = "android"))]
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
