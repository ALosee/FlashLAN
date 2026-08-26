use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::Duration,
};
#[cfg(target_os = "android")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStarted {
    pub task_id: String,
    pub file_name: String,
    pub total: u64,
    pub direction: String,
    pub peer: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub task_id: String,
    pub file_name: String,
    pub progress: f64,
    pub speed: f64,
    pub transferred: u64,
    pub total: u64,
    pub direction: String,
    pub peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub task_id: String,
    pub file_name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_path: Option<String>,
    pub success: bool,
    pub message: String,
    pub direction: String,
    pub peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub task_id: String,
    pub file_name: String,
    pub total: u64,
    pub peer: String,
}

#[derive(Clone, Default)]
pub struct TransferManager {
    inner: Arc<TransferManagerInner>,
}

struct TransferManagerInner {
    pending: Mutex<HashMap<String, PendingTransferRequest>>,
    auto_receive: AtomicBool,
    save_dir: RwLock<PathBuf>,
}

impl Default for TransferManagerInner {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            auto_receive: AtomicBool::new(false),
            save_dir: RwLock::new(PathBuf::new()),
        }
    }
}

struct PendingTransferRequest {
    request: TransferRequest,
    sender: tokio::sync::oneshot::Sender<bool>,
}

impl TransferManager {
    pub fn set_auto_receive(&self, enabled: bool) {
        self.inner.auto_receive.store(enabled, Ordering::Relaxed);
    }

    pub fn set_save_dir(&self, save_dir: PathBuf) {
        if let Ok(mut current) = self.inner.save_dir.write() {
            *current = save_dir;
        }
    }

    pub fn save_dir(&self) -> Result<PathBuf, String> {
        self.inner
            .save_dir
            .read()
            .map(|path| path.clone())
            .map_err(|_| "save path state is unavailable".to_string())
    }

    fn auto_receive(&self) -> bool {
        self.inner.auto_receive.load(Ordering::Relaxed)
    }

    fn register_request(
        &self,
        request: TransferRequest,
    ) -> Result<tokio::sync::oneshot::Receiver<bool>, String> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.inner
            .pending
            .lock()
            .map_err(|_| "transfer request state is unavailable".to_string())?
            .insert(
                request.task_id.clone(),
                PendingTransferRequest { request, sender },
            );
        Ok(receiver)
    }

    pub fn pending_requests(&self) -> Result<Vec<TransferRequest>, String> {
        Ok(self
            .inner
            .pending
            .lock()
            .map_err(|_| "transfer request state is unavailable".to_string())?
            .values()
            .map(|pending| pending.request.clone())
            .collect())
    }

    fn cancel_request(&self, task_id: &str) {
        if let Ok(mut pending) = self.inner.pending.lock() {
            pending.remove(task_id);
        }
    }

    pub fn respond(&self, task_id: &str, accepted: bool) -> Result<(), String> {
        let sender = self
            .inner
            .pending
            .lock()
            .map_err(|_| "transfer request state is unavailable".to_string())?
            .remove(task_id)
            .ok_or_else(|| "transfer request is no longer pending".to_string())?;
        sender
            .sender
            .send(accepted)
            .map_err(|_| "transfer request has already been closed".to_string())
    }
}

pub const TRANSFER_PORT: u16 = 17321;
const CHUNK_SIZE: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECTION_TEST_TIMEOUT: Duration = Duration::from_secs(3);

/// Header sent before file bytes: JSON + newline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHeader {
    pub file_name: String,
    pub file_size: u64,
    pub task_id: String,
}

pub async fn test_connection(target_ip: String, target_port: u16) -> Result<(), String> {
    if target_port == 0 {
        return Err("端口号必须在 1 到 65535 之间".into());
    }

    let ip = target_ip
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| "IP 地址格式无效".to_string())?;
    let addr = SocketAddr::new(ip, target_port);

    tokio::time::timeout(CONNECTION_TEST_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| format!("连接 {}:{} 超时", ip, target_port))?
        .map_err(|error| format!("无法连接 {}:{}：{}", ip, target_port, error))?;

    Ok(())
}

pub async fn start_file_server(app: AppHandle, manager: TransferManager) -> Result<(), String> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TRANSFER_PORT))
        .await
        .map_err(|e| format!("bind transfer port: {e}"))?;
    println!("FlashLAN file server listening on {}", TRANSFER_PORT);
    let app = Arc::new(app);
    tokio::spawn(async move {
        loop {
            let Ok((socket, addr)) = listener.accept().await else {
                continue;
            };
            let app_clone = app.clone();
            let manager_clone = manager.clone();
            tokio::spawn(async move {
                let save_dir = match manager_clone.save_dir() {
                    Ok(path) if !path.as_os_str().is_empty() => path,
                    Ok(_) => {
                        eprintln!("save path is not configured");
                        return;
                    }
                    Err(error) => {
                        eprintln!("read save path failed: {error}");
                        return;
                    }
                };
                if let Err(e) =
                    handle_incoming(socket, save_dir, app_clone, manager_clone, addr.to_string())
                        .await
                {
                    eprintln!("handle incoming from {addr}: {e}");
                }
            });
        }
    });
    Ok(())
}

pub fn open_file_location(app: &AppHandle, path: &str, file_name: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return open_android_file(app, path, file_name);
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, file_name);
        tauri_plugin_opener::reveal_item_in_dir(path)
            .map_err(|error| format!("打开文件目录失败：{error}"))
    }
}

async fn handle_incoming(
    mut socket: TcpStream,
    save_dir: PathBuf,
    app: Arc<AppHandle>,
    manager: TransferManager,
    peer: String,
) -> Result<(), String> {
    // Read the header before any file bytes are sent.
    let header_str = read_line(&mut socket, 8192).await?;
    println!("Received header: {}", header_str);
    let header: FileHeader = serde_json::from_slice(header_str.as_bytes()).map_err(|e| {
        let msg = format!("header json: {e} header={}", header_str);
        eprintln!("{}", msg);
        msg
    })?;
    let file_name = safe_file_name(&header.file_name);

    let accepted = if manager.auto_receive() {
        true
    } else {
        let request = TransferRequest {
            task_id: header.task_id.clone(),
            file_name: file_name.clone(),
            total: header.file_size,
            peer: peer.clone(),
        };
        let receiver = manager.register_request(request.clone())?;
        let _ = app.emit("transfer_request", request);
        match tokio::time::timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(accepted)) => accepted,
            Ok(Err(_)) => false,
            Err(_) => false,
        }
    };

    if !accepted {
        manager.cancel_request(&header.task_id);
        let message = "接收端拒绝了文件或等待确认超时".to_string();
        let _ = app.emit(
            "transfer_complete",
            TransferResult {
                task_id: header.task_id,
                file_name,
                path: String::new(),
                open_path: None,
                success: false,
                message,
                direction: "receive".into(),
                peer,
            },
        );
        let _ = socket.write_all(b"REJECT\n").await;
        return Ok(());
    }

    let mut target = match ReceiveTarget::create(&app, &save_dir, &file_name).await {
        Ok(target) => target,
        Err(error) => {
            let _ = app.emit(
                "transfer_complete",
                TransferResult {
                    task_id: header.task_id.clone(),
                    file_name: file_name.clone(),
                    path: String::new(),
                    open_path: None,
                    success: false,
                    message: error.clone(),
                    direction: "receive".into(),
                    peer: peer.clone(),
                },
            );
            let _ = socket.write_all(b"REJECT\n").await;
            return Err(error);
        }
    };
    if let Err(error) = socket.write_all(b"ACCEPT\n").await {
        target.discard().await;
        return Err(error.to_string());
    }
    let _ = app.emit(
        "transfer_started",
        TransferStarted {
            task_id: header.task_id.clone(),
            file_name: file_name.clone(),
            total: header.file_size,
            direction: "receive".into(),
            peer: peer.clone(),
            path: target.display_path.clone(),
        },
    );
    println!("Receiving {} into {}", file_name, target.display_path);
    println!("File created, start receiving");
    let mut received: u64 = 0;
    let total = header.file_size;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let start = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    loop {
        if total == 0 {
            break;
        }
        let n = match socket.read(&mut buf).await {
            Ok(n) => n,
            Err(error) => {
                target.discard().await;
                return Err(error.to_string());
            }
        };
        if n == 0 {
            if received < total {
                target.discard().await;
                let message = format!("connection closed after {received}/{total} bytes");
                let _ = app.emit(
                    "transfer_complete",
                    TransferResult {
                        task_id: header.task_id.clone(),
                        file_name: file_name.clone(),
                        path: String::new(),
                        open_path: None,
                        success: false,
                        message: message.clone(),
                        direction: "receive".into(),
                        peer: peer.clone(),
                    },
                );
                return Err(message);
            }
            break;
        }
        if let Err(error) = target.file.write_all(&buf[..n]).await {
            target.discard().await;
            return Err(error.to_string());
        }
        received += n as u64;
        let progress = if total > 0 {
            (received as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            received as f64 / elapsed
        } else {
            0.0
        };
        if last_emit.elapsed().as_millis() > 100 || received == total {
            let _ = app.emit(
                "transfer_progress",
                TransferProgress {
                    task_id: header.task_id.clone(),
                    file_name: file_name.clone(),
                    progress,
                    speed,
                    transferred: received,
                    total,
                    direction: "receive".into(),
                    peer: peer.clone(),
                },
            );
            last_emit = std::time::Instant::now();
        }
        if received >= total {
            break;
        }
    }
    if let Err(error) = target.file.flush().await {
        target.discard().await;
        return Err(error.to_string());
    }
    let (final_path, open_path) = match target.finish().await {
        Ok(paths) => paths,
        Err(error) => {
            let _ = app.emit(
                "transfer_complete",
                TransferResult {
                    task_id: header.task_id.clone(),
                    file_name: file_name.clone(),
                    path: String::new(),
                    open_path: None,
                    success: false,
                    message: error.clone(),
                    direction: "receive".into(),
                    peer: peer.clone(),
                },
            );
            return Err(error);
        }
    };
    let _ = app.emit(
        "transfer_complete",
        TransferResult {
            task_id: header.task_id.clone(),
            file_name,
            path: final_path,
            open_path,
            success: true,
            message: "received".into(),
            direction: "receive".into(),
            peer,
        },
    );
    // Ack
    let _ = socket.write_all(b"OK\n").await;
    Ok(())
}

async fn read_line(socket: &mut TcpStream, max_len: usize) -> Result<String, String> {
    let mut buffer = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = socket
            .read(&mut byte)
            .await
            .map_err(|error| error.to_string())?;
        if n == 0 {
            return Err("connection closed before response".into());
        }
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&buffer).to_string());
        }
        buffer.push(byte[0]);
        if buffer.len() > max_len {
            return Err("protocol line is too large".into());
        }
    }
}

fn safe_file_name(file_name: &str) -> String {
    let normalized = file_name.replace('\\', "/");
    Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .unwrap_or("file")
        .to_string()
}

fn unique_path(save_dir: &Path, file_name: &str) -> PathBuf {
    let original = save_dir.join(file_name);
    let mut candidate = original.clone();
    let mut counter = 1;
    while candidate.exists() {
        let stem = original
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = original
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default();
        candidate = save_dir.join(format!("{stem}_{counter}{ext}"));
        counter += 1;
    }
    candidate
}

struct ReceiveTarget {
    file: tokio::fs::File,
    display_path: String,
    #[cfg(target_os = "android")]
    media_uri: Option<String>,
    #[cfg(target_os = "android")]
    app: AppHandle,
}

impl ReceiveTarget {
    async fn create(app: &AppHandle, save_dir: &Path, file_name: &str) -> Result<Self, String> {
        #[cfg(not(target_os = "android"))]
        let _ = app;
        #[cfg(target_os = "android")]
        {
            match create_media_store_file(app, file_name) {
                Ok((file, uri)) => {
                    return Ok(Self {
                        file: tokio::fs::File::from_std(file),
                        display_path: format!("Download/FlashLAN/{file_name}"),
                        media_uri: Some(uri),
                        app: app.clone(),
                    });
                }
                Err(error) => {
                    eprintln!("MediaStore receive fallback: {error}");
                }
            }
        }

        let final_path = unique_path(save_dir, file_name);
        let file = tokio::fs::File::create(&final_path)
            .await
            .map_err(|error| format!("create file {final_path:?}: {error}"))?;
        Ok(Self {
            file,
            display_path: final_path.to_string_lossy().to_string(),
            #[cfg(target_os = "android")]
            media_uri: None,
            #[cfg(target_os = "android")]
            app: app.clone(),
        })
    }

    async fn finish(self) -> Result<(String, Option<String>), String> {
        let ReceiveTarget {
            mut file,
            display_path,
            #[cfg(target_os = "android")]
            media_uri,
            #[cfg(target_os = "android")]
            app,
        } = self;
        #[cfg(target_os = "android")]
        let open_path = media_uri.clone();
        #[cfg(not(target_os = "android"))]
        let open_path = None;
        file.flush().await.map_err(|error| error.to_string())?;
        file.sync_all().await.map_err(|error| error.to_string())?;
        drop(file);

        #[cfg(target_os = "android")]
        if let Some(uri) = media_uri {
            if let Err(error) = finalize_media_store_file(&app, uri.clone()) {
                let _ = delete_media_store_file(&app, uri);
                return Err(error);
            }
        }
        Ok((display_path, open_path))
    }

    async fn discard(self) {
        let ReceiveTarget {
            file,
            #[cfg(target_os = "android")]
            media_uri,
            #[cfg(target_os = "android")]
            app,
            ..
        } = self;
        drop(file);
        #[cfg(target_os = "android")]
        if let Some(uri) = media_uri {
            let _ = delete_media_store_file(&app, uri);
        }
    }
}

#[cfg(target_os = "android")]
fn run_android_jni<T, F>(app: &AppHandle, callback: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut jni::JNIEnv<'_>, &jni::objects::JObject<'_>) -> Result<T, String>
        + Send
        + 'static,
{
    use std::sync::mpsc::sync_channel;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Android webview is not ready".to_string())?;
    let (sender, receiver) = sync_channel(1);
    window
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _| {
                let _ = sender.send(callback(env, activity));
            });
        })
        .map_err(|error| format!("Android JNI dispatch failed: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "Android JNI callback was cancelled".to_string())?
}

#[cfg(target_os = "android")]
fn open_android_file(app: &AppHandle, path: &str, file_name: &str) -> Result<(), String> {
    use jni::objects::JValue;

    if !path.starts_with("content://") {
        return Err("该文件没有可交给 Android 文件管理器的 URI".to_string());
    }

    let uri_string = path.to_string();
    let file_name = file_name.to_string();
    run_android_jni(app, move |env, activity| {
        let uri_class = env
            .find_class("android/net/Uri")
            .map_err(|error| error.to_string())?;
        let uri_text = env
            .new_string(uri_string)
            .map_err(|error| error.to_string())?;
        let uri = env
            .call_static_method(
                uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[(&uri_text).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if uri.is_null() {
            return Err("无法解析 Android 文件 URI".to_string());
        }

        let intent_class = env
            .find_class("android/content/Intent")
            .map_err(|error| error.to_string())?;
        let action_view = env
            .get_static_field(&intent_class, "ACTION_VIEW", "Ljava/lang/String;")
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let intent = env
            .new_object(
                &intent_class,
                "(Ljava/lang/String;)V",
                &[(&action_view).into()],
            )
            .map_err(|error| error.to_string())?;
        let mime = env
            .new_string(mime_type(&file_name))
            .map_err(|error| error.to_string())?;
        env.call_method(
            &intent,
            "setDataAndType",
            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;",
            &[(&uri).into(), (&mime).into()],
        )
        .map_err(|error| error.to_string())?;
        let read_permission = env
            .get_static_field(&intent_class, "FLAG_GRANT_READ_URI_PERMISSION", "I")
            .map_err(|error| error.to_string())?
            .i()
            .map_err(|error| error.to_string())?;
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(read_permission)],
        )
        .map_err(|error| error.to_string())?;

        let package_manager = env
            .call_method(
                activity,
                "getPackageManager",
                "()Landroid/content/pm/PackageManager;",
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let resolved = env
            .call_method(
                &intent,
                "resolveActivity",
                "(Landroid/content/pm/PackageManager;)Landroid/content/ComponentName;",
                &[(&package_manager).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if resolved.is_null() {
            let wildcard_mime = env.new_string("*/*").map_err(|error| error.to_string())?;
            env.call_method(
                &intent,
                "setType",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[(&wildcard_mime).into()],
            )
            .map_err(|error| error.to_string())?;
            let wildcard_resolved = env
                .call_method(
                    &intent,
                    "resolveActivity",
                    "(Landroid/content/pm/PackageManager;)Landroid/content/ComponentName;",
                    &[(&package_manager).into()],
                )
                .map_err(|error| error.to_string())?
                .l()
                .map_err(|error| error.to_string())?;
            if wildcard_resolved.is_null() {
                return Err(format!("手机上没有可以打开 {} 的应用", file_name));
            }
        }
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[(&intent).into()],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
fn put_string(
    env: &mut jni::JNIEnv<'_>,
    values: &jni::objects::JObject<'_>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let key = env.new_string(key).map_err(|error| error.to_string())?;
    let value = env.new_string(value).map_err(|error| error.to_string())?;
    env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[(&key).into(), (&value).into()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "android")]
fn put_int(
    env: &mut jni::JNIEnv<'_>,
    values: &jni::objects::JObject<'_>,
    key: &str,
    value: i32,
) -> Result<(), String> {
    let key = env.new_string(key).map_err(|error| error.to_string())?;
    let value = env
        .new_object("java/lang/Integer", "(I)V", &[value.into()])
        .map_err(|error| error.to_string())?;
    env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/Integer;)V",
        &[(&key).into(), (&value).into()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "android")]
fn media_store_values<'a>(
    env: &mut jni::JNIEnv<'a>,
    file_name: &str,
    pending: bool,
) -> Result<jni::objects::JObject<'a>, String> {
    let values = env
        .new_object("android/content/ContentValues", "()V", &[])
        .map_err(|error| error.to_string())?;
    put_string(env, &values, "_display_name", file_name)?;
    put_string(env, &values, "mime_type", mime_type(file_name))?;
    put_string(env, &values, "relative_path", "Download/FlashLAN/")?;
    put_int(env, &values, "is_pending", if pending { 1 } else { 0 })?;
    Ok(values)
}

#[cfg(target_os = "android")]
fn create_media_store_file(
    app: &AppHandle,
    file_name: &str,
) -> Result<(std::fs::File, String), String> {
    use jni::objects::JString;
    use std::os::fd::FromRawFd;

    let file_name = file_name.to_string();
    run_android_jni(app, move |env, activity| {
        let version = env
            .find_class("android/os/Build$VERSION")
            .map_err(|error| error.to_string())?;
        let sdk = env
            .get_static_field(version, "SDK_INT", "I")
            .map_err(|error| error.to_string())?
            .i()
            .map_err(|error| error.to_string())?;
        if sdk < 29 {
            return Err("MediaStore.Downloads requires Android 10 or newer".into());
        }

        let downloads = env
            .find_class("android/provider/MediaStore$Downloads")
            .map_err(|error| error.to_string())?;
        let collection = env
            .get_static_field(downloads, "EXTERNAL_CONTENT_URI", "Landroid/net/Uri;")
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let values = media_store_values(env, &file_name, true)?;
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let uri = env
            .call_method(
                &resolver,
                "insert",
                "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
                &[(&collection).into(), (&values).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if uri.is_null() {
            return Err("MediaStore did not return a content URI".into());
        }

        let mode = env.new_string("w").map_err(|error| error.to_string())?;
        let descriptor = env
            .call_method(
                &resolver,
                "openFileDescriptor",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                &[(&uri).into(), (&mode).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if descriptor.is_null() {
            return Err("MediaStore could not open the destination".into());
        }
        let fd = env
            .call_method(&descriptor, "detachFd", "()I", &[])
            .map_err(|error| error.to_string())?
            .i()
            .map_err(|error| error.to_string())?;
        let uri_object = env
            .call_method(&uri, "toString", "()Ljava/lang/String;", &[])
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let uri_string: String = env
            .get_string(&JString::from(uri_object))
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        // SAFETY: detachFd transfers ownership of the descriptor to Rust.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        Ok((file, uri_string))
    })
}

#[cfg(target_os = "android")]
fn finalize_media_store_file(app: &AppHandle, uri_string: String) -> Result<(), String> {
    run_android_jni(app, move |env, activity| {
        let uri_class = env
            .find_class("android/net/Uri")
            .map_err(|error| error.to_string())?;
        let uri_text = env
            .new_string(uri_string)
            .map_err(|error| error.to_string())?;
        let uri = env
            .call_static_method(
                uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[(&uri_text).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let values = env
            .new_object("android/content/ContentValues", "()V", &[])
            .map_err(|error| error.to_string())?;
        put_int(env, &values, "is_pending", 0)?;
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        env.call_method(
            &resolver,
            "update",
            "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
            &[
                (&uri).into(),
                (&values).into(),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
fn delete_media_store_file(app: &AppHandle, uri_string: String) -> Result<(), String> {
    run_android_jni(app, move |env, activity| {
        let uri_class = env
            .find_class("android/net/Uri")
            .map_err(|error| error.to_string())?;
        let uri_text = env
            .new_string(uri_string)
            .map_err(|error| error.to_string())?;
        let uri = env
            .call_static_method(
                uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[(&uri_text).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        env.call_method(
            &resolver,
            "delete",
            "(Landroid/net/Uri;Ljava/lang/String;[Ljava/lang/String;)I",
            &[
                (&uri).into(),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
struct AndroidSourceFile {
    file: std::fs::File,
    file_name: String,
    file_size: u64,
}

#[cfg(target_os = "android")]
fn open_android_content_uri(
    app: &AppHandle,
    uri_string: &str,
) -> Result<AndroidSourceFile, String> {
    use jni::objects::{JObject, JString};
    use std::os::fd::FromRawFd;

    let uri_string = uri_string.to_string();
    run_android_jni(app, move |env, activity| {
        let uri_class = env
            .find_class("android/net/Uri")
            .map_err(|error| error.to_string())?;
        let uri_text = env
            .new_string(&uri_string)
            .map_err(|error| error.to_string())?;
        let uri = env
            .call_static_method(
                uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[(&uri_text).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if uri.is_null() {
            return Err(format!("无法解析文件 URI：{uri_string}"));
        }

        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;

        let cursor = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    (&uri).into(),
                    (&JObject::null()).into(),
                    (&JObject::null()).into(),
                    (&JObject::null()).into(),
                    (&JObject::null()).into(),
                ],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;

        let mut display_name = None;
        let mut queried_size = None;
        if !cursor.is_null() {
            let has_row = env
                .call_method(&cursor, "moveToFirst", "()Z", &[])
                .map_err(|error| error.to_string())?
                .z()
                .map_err(|error| error.to_string())?;
            if has_row {
                let display_name_key = env
                    .new_string("_display_name")
                    .map_err(|error| error.to_string())?;
                let name_column = env
                    .call_method(
                        &cursor,
                        "getColumnIndex",
                        "(Ljava/lang/String;)I",
                        &[(&display_name_key).into()],
                    )
                    .map_err(|error| error.to_string())?
                    .i()
                    .map_err(|error| error.to_string())?;
                if name_column >= 0 {
                    let value = env
                        .call_method(
                            &cursor,
                            "getString",
                            "(I)Ljava/lang/String;",
                            &[name_column.into()],
                        )
                        .map_err(|error| error.to_string())?
                        .l()
                        .map_err(|error| error.to_string())?;
                    if !value.is_null() {
                        display_name = Some(
                            env.get_string(&JString::from(value))
                                .map_err(|error| error.to_string())?
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                }

                let size_key = env.new_string("_size").map_err(|error| error.to_string())?;
                let size_column = env
                    .call_method(
                        &cursor,
                        "getColumnIndex",
                        "(Ljava/lang/String;)I",
                        &[(&size_key).into()],
                    )
                    .map_err(|error| error.to_string())?
                    .i()
                    .map_err(|error| error.to_string())?;
                if size_column >= 0 {
                    let value = env
                        .call_method(&cursor, "getLong", "(I)J", &[size_column.into()])
                        .map_err(|error| error.to_string())?
                        .j()
                        .map_err(|error| error.to_string())?;
                    if value >= 0 {
                        queried_size = Some(value as u64);
                    }
                }
            }
            env.call_method(&cursor, "close", "()V", &[])
                .map_err(|error| error.to_string())?;
        }

        let mode = env.new_string("r").map_err(|error| error.to_string())?;
        let descriptor = env
            .call_method(
                &resolver,
                "openFileDescriptor",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                &[(&uri).into(), (&mode).into()],
            )
            .map_err(|error| error.to_string())?
            .l()
            .map_err(|error| error.to_string())?;
        if descriptor.is_null() {
            return Err(format!("无法打开文件 URI：{uri_string}"));
        }
        let fd = env
            .call_method(&descriptor, "detachFd", "()I", &[])
            .map_err(|error| error.to_string())?
            .i()
            .map_err(|error| error.to_string())?;
        // SAFETY: detachFd transfers ownership of the descriptor to Rust.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let file_size = match queried_size {
            Some(size) => size,
            None => file
                .metadata()
                .map_err(|error| format!("无法读取文件大小：{error}"))?
                .len(),
        };
        let file_name = safe_file_name(display_name.as_deref().unwrap_or("file"));
        Ok(AndroidSourceFile {
            file,
            file_name,
            file_size,
        })
    })
}

#[cfg(target_os = "android")]
fn mime_type(file_name: &str) -> &'static str {
    match Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    }
}

pub async fn send_file(
    path: String,
    target_ip: String,
    target_port: u16,
    task_id_opt: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    use std::path::Path;
    println!(
        "send_file request: path={} target={}:{}",
        path, target_ip, target_port
    );
    let (mut file, file_name, file_size) = {
        #[cfg(target_os = "android")]
        if path.starts_with("content://") {
            let source = open_android_content_uri(&app, &path)?;
            (
                tokio::fs::File::from_std(source.file),
                source.file_name,
                source.file_size,
            )
        } else {
            let p = Path::new(&path);
            if !p.exists() {
                let msg = format!("file not found: {path}");
                eprintln!("{}", msg);
                return Err(msg);
            }
            let metadata = tokio::fs::metadata(p).await.map_err(|e| e.to_string())?;
            if !metadata.is_file() {
                return Err("only files supported in MVP, folders TODO".into());
            }
            let file_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            (
                tokio::fs::File::open(p).await.map_err(|e| e.to_string())?,
                file_name,
                metadata.len(),
            )
        }
        #[cfg(not(target_os = "android"))]
        {
            let p = Path::new(&path);
            if !p.exists() {
                let msg = format!("file not found: {path}");
                eprintln!("{}", msg);
                return Err(msg);
            }
            let metadata = tokio::fs::metadata(p).await.map_err(|e| e.to_string())?;
            if !metadata.is_file() {
                return Err("only files supported in MVP, folders TODO".into());
            }
            let file_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            (
                tokio::fs::File::open(p).await.map_err(|e| e.to_string())?,
                file_name,
                metadata.len(),
            )
        }
    };
    let task_id = task_id_opt.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let header = FileHeader {
        file_name: file_name.clone(),
        file_size,
        task_id: task_id.clone(),
    };
    let header_json = serde_json::to_string(&header).map_err(|e| e.to_string())? + "\n";
    let addr = format!("{}:{}", target_ip, target_port);
    println!("Connecting to {} with 5s timeout", addr);
    let mut socket = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("connect {addr}: timeout after 5s"))?
        .map_err(|e| {
            let msg = format!("connect {addr}: {e}");
            eprintln!("{}", msg);
            msg
        })?;
    println!(
        "Connected to {}, sending header {}",
        addr,
        header_json.trim()
    );
    socket
        .write_all(header_json.as_bytes())
        .await
        .map_err(|e| {
            let msg = format!("write header: {e}");
            eprintln!("{}", msg);
            msg
        })?;
    let response = tokio::time::timeout(REQUEST_TIMEOUT, read_line(&mut socket, 128))
        .await
        .map_err(|_| "等待接收端确认超时".to_string())??;
    if response.trim() != "ACCEPT" {
        return Err("接收端拒绝了文件".into());
    }
    println!("File opened, name {}, size {}", file_name, file_size);
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut sent: u64 = 0;
    let start = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    println!("Start sending file {} ({} bytes)", file_name, file_size);
    loop {
        let n = file.read(&mut buf).await.map_err(|e| {
            let msg = format!("read file: {e}");
            eprintln!("{}", msg);
            msg
        })?;
        if n == 0 {
            println!("File read complete, sent {} bytes", sent);
            break;
        }
        println!("Read {} bytes, writing to socket...", n);
        socket.write_all(&buf[..n]).await.map_err(|e| {
            let msg = format!("write socket: {e}");
            eprintln!("{}", msg);
            msg
        })?;
        sent += n as u64;
        println!(
            "Sent {}/{} ({:.1}%)",
            sent,
            file_size,
            if file_size > 0 {
                sent as f64 / file_size as f64 * 100.0
            } else {
                0.0
            }
        );
        let progress = if file_size > 0 {
            (sent as f64 / file_size as f64) * 100.0
        } else {
            0.0
        };
        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            sent as f64 / elapsed
        } else {
            0.0
        };
        if last_emit.elapsed().as_millis() > 100 || sent == file_size {
            println!(
                "Emit progress {}% {}/{} speed {:.1}",
                progress, sent, file_size, speed
            );
            let _ = app.emit(
                "transfer_progress",
                TransferProgress {
                    task_id: task_id.clone(),
                    file_name: file_name.clone(),
                    progress,
                    speed,
                    transferred: sent,
                    total: file_size,
                    direction: "send".into(),
                    peer: target_ip.clone(),
                },
            );
            last_emit = std::time::Instant::now();
        }
    }
    socket.flush().await.map_err(|e| e.to_string())?;
    // Wait for ack with timeout
    let mut ack = [0u8; 3];
    let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut ack)).await;
    let _ = app.emit(
        "transfer_complete",
        TransferResult {
            task_id: task_id.clone(),
            file_name,
            path: path.clone(),
            open_path: Some(path.clone()),
            success: true,
            message: "sent".into(),
            direction: "send".into(),
            peer: target_ip,
        },
    );
    Ok(task_id)
}
