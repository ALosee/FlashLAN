use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex, RwLock,
    },
    time::Duration,
};

// Alias kept short; std Mutex guards are used briefly.
use crate::security::encode_b64;
use base64::Engine as _;
use std::sync::Mutex as StdMutex;
#[cfg(target_os = "android")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Either a plain TCP stream or an upgraded TLS stream. After the handshake
/// every read/write goes through this enum so the transfer code is agnostic.
pub enum Wire {
    /// Server-side TLS stream.
    TlsServer(Box<tokio_rustls::server::TlsStream<TcpStream>>),
    /// Client-side TLS stream.
    TlsClient(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl AsyncRead for Wire {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Wire::TlsServer(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            Wire::TlsClient(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Wire {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        data: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            Wire::TlsServer(stream) => std::pin::Pin::new(stream).poll_write(cx, data),
            Wire::TlsClient(stream) => std::pin::Pin::new(stream).poll_write(cx, data),
        }
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Wire::TlsServer(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            Wire::TlsClient(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Wire::TlsServer(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            Wire::TlsClient(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}

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

/// Emitted when a peer shares clipboard text over the encrypted channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessage {
    pub id: String,
    pub text: String,
    pub peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub task_id: String,
    pub file_name: String,
    pub total: u64,
    pub peer: String,
    /// Number of concrete files in this session (batch shows real count).
    #[serde(default = "one")]
    pub file_count: u32,
}

fn one() -> u32 {
    1
}

/// Cancellation registry: task ids queued for cancellation from the UI.
static CANCELLED_TASKS: LazyLock<StdMutex<HashSet<String>>> =
    LazyLock::new(|| StdMutex::new(HashSet::new()));

pub fn request_cancel(task_id: &str) {
    if let Ok(mut set) = CANCELLED_TASKS.lock() {
        set.insert(task_id.to_string());
    }
}

fn is_cancelled(task_id: &str) -> bool {
    CANCELLED_TASKS
        .lock()
        .map(|set| set.contains(task_id))
        .unwrap_or(false)
}

fn clear_cancel(task_id: &str) {
    if let Ok(mut set) = CANCELLED_TASKS.lock() {
        set.remove(task_id);
    }
}

const CANCELLED_MESSAGE: &str = "传输已取消";

/// Tracks transfer speed over a short rolling window instead of the whole
/// session average, so UI speed reacts to actual throughput changes.
struct SpeedMeter {
    last_tick: std::time::Instant,
    last_bytes: u64,
    speed: f64,
}

impl SpeedMeter {
    fn new() -> Self {
        Self {
            last_tick: std::time::Instant::now(),
            last_bytes: 0,
            speed: 0.0,
        }
    }

    fn tick(&mut self, bytes: u64, force: bool) -> f64 {
        let elapsed = self.last_tick.elapsed().as_secs_f64();
        if force || elapsed >= 0.5 {
            self.speed = (bytes - self.last_bytes) as f64 / elapsed.max(0.001);
            self.last_bytes = bytes;
            self.last_tick = std::time::Instant::now();
        }
        self.speed
    }
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
/// How long the sender waits for the receiver's final OK/REJECT ack.
const ACK_TIMEOUT: Duration = Duration::from_secs(10);
/// Budget for TLS + identity banner exchange.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);

/// Header sent before file bytes: JSON + newline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHeader {
    pub file_name: String,
    pub file_size: u64,
    pub task_id: String,
    /// Present when the sender streams a folder: each entry keeps its path
    /// relative to the transfer root plus the exact byte size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<BatchFileEntry>>,
    /// Display name of the transferred folder root in batch mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFileEntry {
    pub path: String,
    pub size: u64,
}

/// A single concrete file inside a send session.
struct SendEntry {
    absolute_path: PathBuf,
    relative_path: String,
    display_name: String,
    size: u64,
    /// Already-opened source (Android content URIs cannot be re-opened by
    /// path), otherwise the file is opened lazily from `absolute_path`.
    preopened: Option<std::fs::File>,
}

fn chrono_like_timestamp_short() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

async fn collect_send_entries(path: &Path) -> Result<(String, Vec<SendEntry>, u64), String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("无法读取文件信息：{e}"))?;
    let root_display = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("folder")
        .to_string();

    if metadata.is_file() {
        let name = root_display.clone();
        return Ok((
            name,
            vec![SendEntry {
                absolute_path: path.to_path_buf(),
                relative_path: root_display.clone(),
                display_name: root_display,
                size: metadata.len(),
                preopened: None,
            }],
            metadata.len(),
        ));
    }

    // Folder: walk recursively and keep files only (empty dirs are skipped).
    let mut entries: Vec<SendEntry> = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| format!("无法读取文件夹 {dir:?}：{e}"))?;
        while let Some(item) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("遍历文件夹失败：{e}"))?
        {
            let child_path = item.path();
            let child_meta = item
                .metadata()
                .await
                .map_err(|e| format!("无法读取文件信息：{e}"))?;
            if child_meta.is_dir() {
                stack.push(child_path);
                continue;
            }
            if !child_meta.is_file() {
                continue;
            }
            let relative = child_path
                .strip_prefix(path)
                .unwrap_or(&child_path)
                .to_string_lossy()
                .replace('\\', "/");
            let display = format!("{root_display}/{relative}");
            entries.push(SendEntry {
                absolute_path: child_path,
                relative_path: relative.clone(),
                display_name: display,
                size: child_meta.len(),
                preopened: None,
            });
        }
    }

    if entries.is_empty() {
        return Err("文件夹为空，没有可发送的文件".into());
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let total = entries.iter().map(|entry| entry.size).sum();
    Ok((root_display, entries, total))
}

fn single_file_display_name(entries: &[SendEntry]) -> String {
    entries
        .first()
        .map(|entry| entry.display_name.clone())
        .unwrap_or_else(|| "file".to_string())
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

/// Establish a TLS session and verify the identity advertised by the peer.
/// The barcode contains this fingerprint, so pairing must not trust the QR
/// payload until the device on the other end of the socket presents the same
/// identity over the encrypted channel.
pub async fn verify_peer_fingerprint(
    target_ip: String,
    target_port: u16,
    expected_fingerprint: String,
    connector: TlsConnector,
    own_fingerprint: String,
) -> Result<(), String> {
    let expected = expected_fingerprint.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("设备指纹格式无效".into());
    }

    let ip = target_ip
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| "IP 地址格式无效".to_string())?;
    let addr = SocketAddr::new(ip, target_port);
    let tcp = tokio::time::timeout(CONNECTION_TEST_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| format!("连接 {}:{} 超时", ip, target_port))?
        .map_err(|error| format!("无法连接 {}:{}：{}", ip, target_port, error))?;
    let tls = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        connector.connect(
            rustls::pki_types::ServerName::try_from("flashlan.local".to_string()).unwrap(),
            tcp,
        ),
    )
    .await
    .map_err(|_| "TLS 握手超时".to_string())?
    .map_err(|error| format!("TLS 握手失败：{error}"))?;
    let mut wire = Wire::TlsClient(Box::new(tls));
    send_hello(&mut wire, &own_fingerprint).await?;
    let actual = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_hello(&mut wire))
        .await
        .map_err(|_| "等待设备身份信息超时".to_string())??
        .to_ascii_lowercase();

    if actual != expected {
        return Err("二维码中的设备指纹与实际设备不一致，请重新扫码".into());
    }
    Ok(())
}

pub async fn start_file_server(
    app: AppHandle,
    manager: TransferManager,
    tls: TlsAcceptor,
    identity_fingerprint: String,
    trust: Arc<crate::security::TrustStore>,
) -> Result<(), String> {
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
            let tls_clone = tls.clone();
            let fingerprint_clone = identity_fingerprint.clone();
            let trust_clone = trust.clone();
            tokio::spawn(async move {
                // TLS happens first; transfer args are resolved after the
                // handshake so a bad client cannot occupy save-dir locks.
                let peer_addr = addr.to_string();
                let (mut wire, peer_id) = match tokio::time::timeout(
                    HANDSHAKE_TIMEOUT,
                    accept_session(socket, &tls_clone, &fingerprint_clone),
                )
                .await
                {
                    Ok(Ok(session)) => session,
                    Ok(Err(error)) => {
                        eprintln!("tls handshake from {peer_addr}: {error}");
                        return;
                    }
                    Err(_) => {
                        eprintln!("tls handshake from {peer_addr}: timeout");
                        return;
                    }
                };
                let Some(save_dir) = manager_clone.save_dir_ok() else {
                    eprintln!("save path is not configured");
                    return;
                };
                let mut request = IncomingRequest {
                    trusted: trust_clone.is_trusted(&peer_id.fingerprint),
                };
                // Loop per connection? One connection transfers one logical task.
                if let Err(e) = handle_incoming(
                    &mut wire,
                    save_dir,
                    app_clone,
                    manager_clone,
                    peer_addr,
                    &mut request,
                )
                .await
                {
                    eprintln!("handle incoming: {e}");
                }
            });
        }
    });
    Ok(())
}

/// Result of the TLS handshake for one connection.
pub struct PeerIdentity {
    pub fingerprint: String,
}

impl TransferManager {
    /// save_dir that exists and is non-empty, None otherwise.
    fn save_dir_ok(&self) -> Option<PathBuf> {
        self.save_dir()
            .ok()
            .filter(|path| !path.as_os_str().is_empty())
    }
}

/// Perform the TLS handshake and exchange device identity banners:
/// each side sends `HELLO <base64(json)>\n` with its fingerprint/name.
async fn accept_session(
    socket: TcpStream,
    tls: &TlsAcceptor,
    own_fingerprint: &str,
) -> Result<(Wire, PeerIdentity), String> {
    let stream = tls
        .accept(socket)
        .await
        .map_err(|error| format!("tls accept: {error}"))?;
    let mut wire = Wire::TlsServer(Box::new(stream));
    send_hello(&mut wire, own_fingerprint).await?;
    let peer = read_hello(&mut wire).await?;
    Ok((wire, PeerIdentity { fingerprint: peer }))
}

async fn send_hello(wire: &mut Wire, fingerprint: &str) -> Result<(), String> {
    let line = format!("{} {}\n", HELLO_MAGIC, encode_b64(fingerprint.as_bytes()));
    wire.write_all(line.as_bytes())
        .await
        .map_err(|error| format!("write hello: {error}"))
}

async fn read_hello(wire: &mut Wire) -> Result<String, String> {
    let line = read_line(wire, 512).await?;
    let mut parts = line.split_whitespace();
    let magic = parts.next().unwrap_or_default();
    if magic != HELLO_MAGIC {
        return Err("peer did not speak the FlashLAN handshake".into());
    }
    let payload = parts.next().unwrap_or_default();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .map_err(|_| "invalid hello payload")?;
    Ok(String::from_utf8(decoded).map_err(|_| "invalid hello encoding")?)
}

const HELLO_MAGIC: &str = "HELLO1";

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

pub struct IncomingRequest {
    /// Whether the sender's fingerprint is in our trust store.
    pub trusted: bool,
}

async fn handle_incoming(
    socket: &mut Wire,
    save_dir: PathBuf,
    app: Arc<AppHandle>,
    manager: TransferManager,
    peer: String,
    request: &mut IncomingRequest,
) -> Result<(), String> {
    // A connection starts either with a file header (JSON) or a quick
    // "TEXT <base64>" clipboard share; TLS banner already identified the peer.
    let header_str = read_line(socket, 8192).await?;
    if header_str.starts_with("TEXT ") {
        return handle_text_message(socket, &app, &peer, request.trusted, &header_str).await;
    }
    println!("Received header: {}", header_str);
    let header: FileHeader = serde_json::from_slice(header_str.as_bytes()).map_err(|e| {
        let msg = format!("header json: {e} header={}", header_str);
        eprintln!("{}", msg);
        msg
    })?;
    let file_name = safe_file_name(&header.file_name);

    let (accepted, declined_reason) = if manager.auto_receive() && request.trusted {
        (true, None)
    } else {
        let request = TransferRequest {
            task_id: header.task_id.clone(),
            file_name: file_name.clone(),
            total: header.file_size,
            peer: peer.clone(),
            file_count: header
                .files
                .as_ref()
                .map(|list| list.len().max(1) as u32)
                .unwrap_or(1),
        };
        let receiver = manager.register_request(request.clone())?;
        let _ = app.emit("transfer_request", request);
        match tokio::time::timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(accepted)) => {
                if accepted {
                    (true, None)
                } else {
                    (false, Some("对方拒绝了本次接收".to_string()))
                }
            }
            Ok(Err(_)) => (false, Some("确认通道已关闭，自动拒绝".to_string())),
            Err(_) => (false, Some("等待确认超时（60 秒），已自动拒绝".to_string())),
        }
    };

    if !accepted {
        manager.cancel_request(&header.task_id);
        let message = declined_reason.unwrap_or_else(|| "对方拒绝了本次接收".to_string());
        let _ = app.emit(
            "transfer_complete",
            TransferResult {
                task_id: header.task_id,
                file_name,
                path: String::new(),
                open_path: None,
                success: false,
                message: message.clone(),
                direction: "receive".into(),
                peer,
            },
        );
        let _ = socket.write_all(b"REJECT\n").await;
        return Ok(());
    }

    clear_cancel(&header.task_id);

    // Batch mode restores the original folder structure; on Android every
    // file goes into the public Download/FlashLAN/<root>/ collection.
    if let Some(batch_files) = header.files.clone() {
        // Empty root => flat placement directly in the default location.
        let root_name = match header.root.as_deref() {
            Some("") | None => String::new(),
            Some(other) => safe_file_name(other),
        };
        return receive_batch(
            socket,
            app,
            peer,
            header.task_id,
            root_name,
            batch_files,
            header.file_size,
            file_name,
            save_dir,
        )
        .await;
    }

    // Resume support: reply includes how many bytes we already have on disk
    // (".part" suffix marks an unfinished file). Sender skips that prefix.
    let mut target =
        match ReceiveTarget::create(&app, &save_dir, &file_name, header.file_size).await {
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
    let accept_reply = if target.resumed_bytes > 0 && header.file_size > target.resumed_bytes {
        format!("ACCEPT {}\n", target.resumed_bytes)
    } else {
        "ACCEPT\n".to_string()
    };
    if let Err(error) = socket.write_all(accept_reply.as_bytes()).await {
        target.discard().await;
        return Err(error.to_string());
    }
    // If we somehow hold more bytes than the file is long, restart from 0.
    let skip_bytes = target.resumed_bytes.min(header.file_size);
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
    let mut received: u64 = 0;
    let total = header.file_size;
    // Resume: skip (discard) the prefix the receiver already holds.
    let mut skipped: u64 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut last_emit = std::time::Instant::now();
    let mut speed_meter = SpeedMeter::new();
    loop {
        if is_cancelled(&header.task_id) {
            target.discard().await;
            let _ = app.emit(
                "transfer_complete",
                TransferResult {
                    task_id: header.task_id.clone(),
                    file_name: file_name.clone(),
                    path: String::new(),
                    open_path: None,
                    success: false,
                    message: CANCELLED_MESSAGE.into(),
                    direction: "receive".into(),
                    peer: peer.clone(),
                },
            );
            return Ok(());
        }
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
        if n == 0 && received + skipped < total {
            target.discard().await;
            let message = format!(
                "connection closed after {}/{} bytes",
                received + skipped,
                total
            );
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
        if n == 0 {
            break;
        }
        // Resume: drop the prefix instead of writing it a second time.
        if skipped < skip_bytes {
            let remaining_skip = (skip_bytes - skipped) as usize;
            if n <= remaining_skip {
                skipped += n as u64;
                continue;
            }
            let usable = &buf[remaining_skip..];
            skipped = skip_bytes;
            if let Err(error) = target.file.write_all(usable).await {
                target.discard().await;
                return Err(error.to_string());
            }
            received += usable.len() as u64;
            continue;
        }
        if let Err(error) = target.file.write_all(&buf[..n]).await {
            target.discard().await;
            return Err(error.to_string());
        }
        received += n as u64;
        let progress = if total > 0 {
            ((received + skipped) as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let speed = speed_meter.tick(received, received == total);
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
        if received + skipped >= total {
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

async fn read_line<R: AsyncRead + Unpin>(socket: &mut R, max_len: usize) -> Result<String, String> {
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

/// Like [`safe_file_name`] but keeps inner path structure, refusing any
/// traversal components so remote entries cannot escape the target dir.
fn safe_relative_path(relative: &str) -> Option<PathBuf> {
    let normalized = relative.replace('\\', "/");
    let mut result = PathBuf::new();
    for component in normalized.split('/') {
        match component {
            "" | "." => continue,
            ".." => return None,
            part => {
                let safe = safe_file_name(part);
                if safe == "file" && part != "file" {
                    // Component fully sanitized away; reject to be safe.
                    if part.trim().is_empty() || part.contains(['/', '\\']) {
                        return None;
                    }
                }
                result.push(safe);
            }
        }
    }
    if result.as_os_str().is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Pick a non-existing folder name by appending `_1`, `_2`, ... when needed.
fn unique_dir(parent: &Path, name: &str) -> PathBuf {
    let original = parent.join(name);
    if !original.exists() {
        return original;
    }
    let mut counter = 1;
    loop {
        let candidate = parent.join(format!("{name}_{counter}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Handle a "TEXT <base64>" line: emit to the UI and acknowledge.
async fn handle_text_message(
    socket: &mut Wire,
    app: &AppHandle,
    peer: &str,
    _trusted: bool,
    line: &str,
) -> Result<(), String> {
    use base64::Engine as _;
    let payload = line.trim_start_matches("TEXT ").trim();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .map_err(|_| "invalid text payload")?;
    let text = String::from_utf8(decoded).map_err(|_| "invalid text encoding")?;
    if text.len() > 512 * 1024 {
        return Err("text message too large".into());
    }
    let message = TextMessage {
        id: uuid::Uuid::new_v4().to_string(),
        text,
        peer: peer.to_string(),
    };
    let _ = app.emit("text_message", message);
    socket
        .write_all(b"OK\n")
        .await
        .map_err(|error| format!("write text ack: {error}"))?;
    Ok(())
}

/// Destination for one incoming file of a batch transfer: the public
/// Downloads collection via MediaStore (preferred, Android 10+) or the
/// visible folder tree under the configured save dir as a fallback.
struct BatchSink {
    file: tokio::fs::File,
    fs_path: Option<PathBuf>,
    #[cfg(target_os = "android")]
    media_uri: Option<String>,
}

impl BatchSink {
    async fn complete(mut self, app: &AppHandle) -> Result<(), String> {
        #[cfg(not(target_os = "android"))]
        let _ = app;
        self.file.flush().await.map_err(|e| e.to_string())?;
        self.file.sync_all().await.map_err(|e| e.to_string())?;
        drop(self.file);
        self.fs_path = None;
        #[cfg(target_os = "android")]
        if let Some(uri) = self.media_uri.take() {
            // Make the finished file visible outside the app immediately.
            if let Err(error) = finalize_media_store_file(app, uri.clone()) {
                let _ = delete_media_store_file(app, uri);
                return Err(error);
            }
        }
        Ok(())
    }

    async fn abort(mut self, app: &AppHandle) {
        #[cfg(not(target_os = "android"))]
        let _ = app;
        drop(self.file);
        #[cfg(target_os = "android")]
        if let Some(uri) = self.media_uri.take() {
            let _ = delete_media_store_file(app, uri);
        }
        if let Some(path) = self.fs_path.take() {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }

    async fn from_fallback(
        root_dir: &Path,
        rel_path: &Path,
        file_name: &str,
    ) -> Result<Self, String> {
        let target_dir = match rel_path.parent() {
            Some(parent) => {
                let dir = root_dir.join(parent);
                tokio::fs::create_dir_all(&dir)
                    .await
                    .map_err(|e| format!("无法创建子目录 {dir:?}：{e}"))?;
                dir
            }
            None => root_dir.to_path_buf(),
        };
        let final_path = unique_path(&target_dir, file_name);
        let file = tokio::fs::File::create(&final_path)
            .await
            .map_err(|e| format!("创建文件失败 {final_path:?}：{e}"))?;
        Ok(Self {
            file,
            fs_path: Some(final_path),
            #[cfg(target_os = "android")]
            media_uri: None,
        })
    }
}

#[allow(clippy::too_many_arguments)]
async fn receive_batch(
    socket: &mut Wire,
    app: Arc<AppHandle>,
    peer: String,
    task_id: String,
    root_name: String,
    batch_files: Vec<BatchFileEntry>,
    total: u64,
    display_name: String,
    save_dir: PathBuf,
) -> Result<(), String> {
    clear_cancel(&task_id);

    // Where the user will find the folder afterwards.
    #[cfg(target_os = "android")]
    let location = if root_name.is_empty() {
        "Download/FlashLAN/".to_string()
    } else {
        format!("Download/FlashLAN/{root_name}/")
    };
    #[cfg(not(target_os = "android"))]
    let base_root = if root_name.is_empty() {
        save_dir.clone()
    } else {
        unique_dir(&save_dir, &root_name)
    };
    #[cfg(not(target_os = "android"))]
    let location = base_root.to_string_lossy().to_string();

    let _ = socket.write_all(b"ACCEPT\n").await;
    let _ = app.emit(
        "transfer_started",
        TransferStarted {
            task_id: task_id.clone(),
            file_name: display_name.clone(),
            total,
            direction: "receive".into(),
            peer: peer.clone(),
            path: location.clone(),
        },
    );

    let mut received_total: u64 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut last_emit = std::time::Instant::now();
    let mut speed_meter = SpeedMeter::new();

    for entry in &batch_files {
        if is_cancelled(&task_id) {
            emit_receive_failed(
                &app,
                &task_id,
                &display_name,
                &peer,
                CANCELLED_MESSAGE.into(),
            );
            return Ok(());
        }
        let Some(rel_path) = safe_relative_path(&entry.path) else {
            let message = format!("对方发送了非法路径：{}", entry.path);
            emit_receive_failed(&app, &task_id, &display_name, &peer, message);
            return Ok(());
        };
        let name_part = rel_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        #[cfg(target_os = "android")]
        let rel_dir = rel_path
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        // Public Downloads (MediaStore) first; fall back to app storage only
        // when that fails so very old devices still receive the files.
        #[cfg(target_os = "android")]
        let sink = match create_media_store_file_at(
            &app,
            &name_part,
            &if rel_dir.is_empty() {
                location.clone()
            } else {
                format!("{location}{rel_dir}/")
            },
        ) {
            Ok((file, uri)) => BatchSink {
                file: tokio::fs::File::from_std(file),
                fs_path: None,
                media_uri: Some(uri),
            },
            Err(error) => {
                eprintln!("MediaStore batch fallback: {error}");
                let fallback_root = unique_dir(&save_dir, &root_name);
                match BatchSink::from_fallback(&fallback_root, &rel_path, &name_part).await {
                    Ok(sink) => sink,
                    // from_fallback owns partial-file cleanup via BatchSink.
                    Err(create_error) => return Err(create_error),
                }
            }
        };

        #[cfg(not(target_os = "android"))]
        let sink = BatchSink::from_fallback(&base_root, &rel_path, &name_part).await?;

        let mut out = sink;
        let mut received_entry: u64 = 0;
        while received_entry < entry.size {
            if is_cancelled(&task_id) {
                out.abort(&app).await;
                emit_receive_failed(
                    &app,
                    &task_id,
                    &display_name,
                    &peer,
                    CANCELLED_MESSAGE.into(),
                );
                return Ok(());
            }
            let want = (entry.size - received_entry).min(CHUNK_SIZE as u64) as usize;
            let n = match socket.read(&mut buf[..want]).await {
                Ok(n) => n,
                Err(error) => {
                    out.abort(&app).await;
                    return Err(error.to_string());
                }
            };
            if n == 0 {
                out.abort(&app).await;
                let message = format!("connection closed after {received_total}/{total} bytes");
                emit_receive_failed(&app, &task_id, &display_name, &peer, message.clone());
                return Err(message);
            }
            if let Err(error) = out.file.write_all(&buf[..n]).await {
                out.abort(&app).await;
                return Err(error.to_string());
            }
            received_entry += n as u64;
            received_total += n as u64;

            let progress = if total > 0 {
                (received_total as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let speed = speed_meter.tick(received_total, received_total == total);
            if last_emit.elapsed().as_millis() > 100 || received_total == total {
                let _ = app.emit(
                    "transfer_progress",
                    TransferProgress {
                        task_id: task_id.clone(),
                        file_name: name_part.clone(),
                        progress,
                        speed,
                        transferred: received_total,
                        total,
                        direction: "receive".into(),
                        peer: peer.clone(),
                    },
                );
                last_emit = std::time::Instant::now();
            }
        }
        if let Err(error) = out.complete(&app).await {
            let message = format!("保存文件失败：{error}");
            emit_receive_failed(&app, &task_id, &display_name, &peer, message.clone());
            return Err(message);
        }
    }

    let ack_task_id = task_id;
    clear_cancel(&ack_task_id);
    let ack_display = display_name;
    let ack_peer = peer;
    let _ = app.emit(
        "transfer_complete",
        TransferResult {
            task_id: ack_task_id,
            file_name: ack_display,
            path: location,
            open_path: None,
            success: true,
            message: "received".into(),
            direction: "receive".into(),
            peer: ack_peer,
        },
    );
    let _ = socket.write_all(b"OK\n").await;
    Ok(())
}

fn emit_receive_failed(
    app: &AppHandle,
    task_id: &str,
    file_name: &str,
    peer: &str,
    message: String,
) {
    let _ = app.emit(
        "transfer_complete",
        TransferResult {
            task_id: task_id.to_string(),
            file_name: file_name.to_string(),
            path: String::new(),
            open_path: None,
            success: false,
            message,
            direction: "receive".into(),
            peer: peer.to_string(),
        },
    );
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
    /// Bytes already on disk before this session (resume support).
    resumed_bytes: u64,
    #[cfg(target_os = "android")]
    media_uri: Option<String>,
    #[cfg(target_os = "android")]
    app: AppHandle,
}

impl ReceiveTarget {
    async fn create(
        app: &AppHandle,
        save_dir: &Path,
        file_name: &str,
        expected_size: u64,
    ) -> Result<Self, String> {
        #[cfg(not(target_os = "android"))]
        let _ = app;
        #[cfg(target_os = "android")]
        {
            match create_media_store_file(app, file_name) {
                Ok((file, uri)) => {
                    return Ok(Self {
                        file: tokio::fs::File::from_std(file),
                        display_path: format!("Download/FlashLAN/{file_name}"),
                        resumed_bytes: 0,
                        media_uri: Some(uri),
                        app: app.clone(),
                    });
                }
                Err(error) => {
                    eprintln!("MediaStore receive fallback: {error}");
                }
            }
        }

        // Resume support: a leftover "<name>.part" holds previous bytes.
        let part_path = save_dir.join(format!("{file_name}.part"));
        let (final_candidate, mut resumed_bytes) = if part_path.exists() {
            let size = tokio::fs::metadata(&part_path)
                .await
                .map(|meta| meta.len())
                .unwrap_or(0);
            (part_path, size)
        } else {
            let final_path = unique_path(save_dir, file_name);
            (
                save_dir.join(format!(
                    "{}.part",
                    final_path.file_name().unwrap_or_default().to_string_lossy()
                )),
                0,
            )
        };
        // A stale partial file can be longer than a newly selected source
        // with the same name. In that case restarting is safer than keeping
        // an oversized/corrupted result.
        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).write(true);
        if resumed_bytes > expected_size {
            resumed_bytes = 0;
            options.truncate(true);
        } else {
            options.append(true);
        }
        let file = options
            .open(&final_candidate)
            .await
            .map_err(|error| format!("create file {final_candidate:?}: {error}"))?;
        Ok(Self {
            file,
            display_path: final_candidate.to_string_lossy().to_string(),
            resumed_bytes,
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
            resumed_bytes: _,
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

        // Resume support: ".part" files get their final name on success.
        #[cfg(not(target_os = "android"))]
        let display_path = {
            let path = PathBuf::from(&display_path);
            if path.extension().and_then(|e| e.to_str()) == Some("part") {
                let final_name = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("file");
                let final_path = unique_path(path.parent().unwrap_or(Path::new(".")), final_name);
                match std::fs::rename(&path, &final_path) {
                    Ok(()) => final_path.to_string_lossy().to_string(),
                    Err(_) => display_path,
                }
            } else {
                display_path
            }
        };
        // On Android the MediaStore entry is visible already (is_pending is
        // only used there), so no rename is needed.
        let _ = &display_path;
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
    relative_dir: &str,
) -> Result<jni::objects::JObject<'a>, String> {
    let values = env
        .new_object("android/content/ContentValues", "()V", &[])
        .map_err(|error| error.to_string())?;
    put_string(env, &values, "_display_name", file_name)?;
    put_string(env, &values, "mime_type", mime_type(file_name))?;
    put_string(env, &values, "relative_path", relative_dir)?;
    put_int(env, &values, "is_pending", if pending { 1 } else { 0 })?;
    Ok(values)
}

#[cfg(target_os = "android")]
fn create_media_store_file_at(
    app: &AppHandle,
    file_name: &str,
    relative_dir: &str,
) -> Result<(std::fs::File, String), String> {
    use jni::objects::JString;
    use std::os::fd::FromRawFd;

    let file_name = file_name.to_string();
    let relative_dir = relative_dir.to_string();
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
        let values = media_store_values(env, &file_name, true, &relative_dir)?;
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
fn create_media_store_file(
    app: &AppHandle,
    file_name: &str,
) -> Result<(std::fs::File, String), String> {
    create_media_store_file_at(app, file_name, "Download/FlashLAN/")
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

pub struct SendSecurity {
    pub connector: TlsConnector,
    /// Sender's own fingerprint, sent in the HELLO banner.
    pub fingerprint: String,
}

/// Entry point for sending one file OR a whole folder. `paths` with a single
/// folder collapses into that folder; otherwise each path becomes an entry
/// of one combined session (single confirmation on the receiver).
pub async fn send_file(
    paths: Vec<String>,
    target_ip: String,
    target_port: u16,
    task_id_opt: Option<String>,
    app: AppHandle,
    security: SendSecurity,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("没有可发送的文件".into());
    }
    // Single folder arg -> folder mode handled by existing logic via len()==1.
    send_session(paths, target_ip, target_port, task_id_opt, app, security).await
}

async fn send_session(
    paths: Vec<String>,
    target_ip: String,
    target_port: u16,
    task_id_opt: Option<String>,
    app: AppHandle,
    security: SendSecurity,
) -> Result<String, String> {
    println!("send_session request: paths={paths:?} target={target_ip}:{target_port}");

    // Collect every concrete SendEntry across all requested paths first.
    // A single path keeps its own name as root (file or folder). Multiple
    // paths are grouped under "FlashLAN-<timestamp>" as the batch root.
    let multi = paths.len() > 1;
    // Loose files are placed directly into the receiver's default location;
    // only selections containing at least one folder get a grouping wrapper
    // ("FlashLAN-<timestamp>") so the folder subtree stays recognizable.
    let mut any_dir = false;

    #[cfg(target_os = "android")]
    let mut entries: Vec<SendEntry> = Vec::new();
    #[cfg(target_os = "android")]
    for path in paths.iter() {
        if path.starts_with("content://") {
            let source = open_android_content_uri(&app, path)?;
            entries.push(SendEntry {
                absolute_path: PathBuf::new(),
                relative_path: source.file_name.clone(),
                display_name: source.file_name.clone(),
                size: source.file_size,
                preopened: Some(source.file),
            });
            continue;
        }
        let source_path = PathBuf::from(path);
        if !source_path.exists() {
            return Err(format!("file not found: {path}"));
        }
        let is_dir = tokio::fs::metadata(&source_path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);
        any_dir |= is_dir;
        match collect_send_entries(&source_path).await {
            Ok((_root, mut list, _total)) => {
                if multi && is_dir {
                    let name = source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("folder")
                        .to_string();
                    for entry in &mut list {
                        entry.relative_path = format!("{name}/{}", entry.relative_path);
                    }
                }
                entries.extend(list);
            }
            Err(error) => return Err(error),
        }
    }

    #[cfg(not(target_os = "android"))]
    let mut entries: Vec<SendEntry> = Vec::new();
    #[cfg(not(target_os = "android"))]
    for path in &paths {
        let source_path = PathBuf::from(path);
        if !source_path.exists() {
            return Err(format!("file not found: {path}"));
        }
        let is_dir = tokio::fs::metadata(&source_path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);
        any_dir |= is_dir;
        match collect_send_entries(&source_path).await {
            Ok((_root, mut list, _total)) => {
                if multi && is_dir {
                    let name = source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("folder")
                        .to_string();
                    for entry in &mut list {
                        entry.relative_path = format!("{name}/{}", entry.relative_path);
                    }
                }
                entries.extend(list);
            }
            Err(error) => return Err(error),
        }
    }

    let total_size: u64 = entries.iter().map(|entry| entry.size).sum();
    // Batch whenever there is more than one concrete file or a single folder.
    let single_is_dir = tokio::fs::metadata(&paths[0])
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    let batch_mode = entries.len() > 1 || single_is_dir;
    let fs_root = if multi && any_dir {
        format!("FlashLAN-{}", chrono_like_timestamp_short())
    } else if single_is_dir {
        // Preserve the selected folder as the root when transferring one
        // folder, instead of flattening its contents into the save directory.
        Path::new(&paths[0])
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("folder")
            .to_string()
    } else {
        String::new()
    };
    let root_display = if multi && !any_dir {
        format!("{} 个文件", entries.len())
    } else if !multi && !single_is_dir {
        // A single file is not a batch. Its real name must be sent in the
        // header so the receiver can retain the extension (e.g. .jpg).
        single_file_display_name(&entries)
    } else {
        fs_root.clone()
    };
    let root_display_path = paths[0].clone();

    let task_id = task_id_opt.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    clear_cancel(&task_id);

    let mut header = FileHeader {
        file_name: root_display.clone(),
        file_size: total_size,
        task_id: task_id.clone(),
        files: None,
        root: None,
    };
    if batch_mode {
        header.files = Some(
            entries
                .iter()
                .map(|entry| BatchFileEntry {
                    path: entry.relative_path.clone(),
                    size: entry.size,
                })
                .collect(),
        );
        // "" means "no wrapper folder": entries land directly in the
        // receiver's default download location.
        header.root = Some(fs_root);
    }

    let header_json = serde_json::to_string(&header).map_err(|e| e.to_string())? + "\n";
    let addr = format!("{}:{}", target_ip, target_port);
    println!("Connecting to {} with 5s timeout", addr);
    let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("connect {addr}: timeout after 5s"))?
        .map_err(|e| {
            let msg = format!("connect {addr}: {e}");
            eprintln!("{}", msg);
            msg
        })?;
    let mut socket = match tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        security.connector.connect(
            rustls::pki_types::ServerName::try_from("flashlan.local".to_string()).unwrap(),
            tcp,
        ),
    )
    .await
    {
        Ok(Ok(tls)) => Wire::TlsClient(Box::new(tls)),
        Ok(Err(error)) => return Err(format!("TLS 握手失败：{error}")),
        Err(_) => return Err("TLS 握手超时".into()),
    };
    // Mutual identity banners over the encrypted channel.
    send_hello(&mut socket, &security.fingerprint).await?;
    let peer_fingerprint =
        match tokio::time::timeout(HANDSHAKE_TIMEOUT, read_hello(&mut socket)).await {
            Ok(Ok(fingerprint)) => fingerprint,
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err("等待对方身份信息超时".into()),
        };
    let _ = peer_fingerprint;
    socket
        .write_all(header_json.as_bytes())
        .await
        .map_err(|e| {
            let msg = format!("write header: {e}");
            eprintln!("{}", msg);
            msg
        })?;

    // The receiver blocks on a user confirmation here; use the same generous
    // timeout as its confirmation window so neither side gives up early.
    let response = match tokio::time::timeout(REQUEST_TIMEOUT, read_line(&mut socket, 128)).await {
        Err(_) => {
            let message = "等待对方确认超时".to_string();
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            return Err(message);
        }
        Ok(Err(error)) => {
            let message = format!("连接中断：{error}");
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            return Err(message);
        }
        Ok(Ok(line)) => line.trim().to_string(),
    };
    // "ACCEPT" or "ACCEPT <resume-offset>" (single-file resume support).
    let mut response_parts = response.split_whitespace();
    let start_offset: u64 = match response_parts.next() {
        Some("ACCEPT") => response_parts
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0),
        Some("REJECT") | Some("CANCEL") => {
            let message = "对方拒绝了本次传输".to_string();
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            return Err(message);
        }
        _ => {
            let message = format!("对方响应异常：{response}");
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            return Err(message);
        }
    };
    if start_offset > total_size {
        let message = "对方的续传偏移异常".to_string();
        emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
        return Err(message);
    }

    println!(
        "Start sending {} ({} bytes, batch={batch_mode})",
        root_display, total_size
    );
    let _ = app.emit(
        "transfer_started",
        TransferStarted {
            task_id: task_id.clone(),
            file_name: root_display.clone(),
            total: total_size,
            direction: "send".into(),
            peer: target_ip.clone(),
            path: paths.join(", "),
        },
    );

    let send_result = stream_entries(
        &mut socket,
        &app,
        &task_id,
        entries,
        total_size,
        batch_mode,
        &target_ip,
        start_offset,
    )
    .await;

    // Validate the final ack instead of assuming success: only "OK" means the
    // receiver actually finished writing everything to disk.
    let ack = if send_result.is_ok() {
        socket.flush().await.ok();
        match tokio::time::timeout(ACK_TIMEOUT, read_line(&mut socket, 16)).await {
            Ok(Ok(line)) => Some(line.trim().to_string()),
            Ok(Err(_)) => None,
            Err(_) => None,
        }
    } else {
        None
    };

    match (send_result, ack.as_deref()) {
        (Ok(()), Some("OK")) => {
            let _ = app.emit(
                "transfer_complete",
                TransferResult {
                    task_id: task_id.clone(),
                    file_name: root_display,
                    path: root_display_path.clone(),
                    open_path: Some(root_display_path.clone()),
                    success: true,
                    message: "sent".into(),
                    direction: "send".into(),
                    peer: target_ip,
                },
            );
            clear_cancel(&task_id);
            Ok(task_id)
        }
        (Ok(()), _) => {
            let message = "对方未能完整保存文件".to_string();
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            Err(message)
        }
        (Err(stream_error), _) => {
            let message = if is_cancelled(&task_id) {
                CANCELLED_MESSAGE.to_string()
            } else {
                stream_error
            };
            emit_send_failed(&app, &task_id, &root_display, &target_ip, message.clone());
            Err(message)
        }
    }
}

/// Stream every entry over the accepted connection, emitting cumulative
/// progress. Returns once all bytes have been written to the socket.
#[allow(clippy::too_many_arguments)]
/// Share clipboard text over an encrypted one-shot connection.
pub async fn send_text(
    text: String,
    target_ip: String,
    target_port: u16,
    app: AppHandle,
    security: SendSecurity,
) -> Result<(), String> {
    use base64::Engine as _;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("文本内容为空".into());
    }
    if text.len() > 512 * 1024 {
        return Err("文本内容过大".into());
    }
    let payload = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let addr = format!("{}:{}", target_ip, target_port);
    let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("connect {addr}: timeout"))?
        .map_err(|e| format!("connect {addr}: {e}"))?;
    let socket = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        security.connector.connect(
            rustls::pki_types::ServerName::try_from("flashlan.local".to_string()).unwrap(),
            tcp,
        ),
    )
    .await
    .map_err(|_| "TLS 握手超时".to_string())?
    .map_err(|error| format!("TLS 握手失败：{error}"))?;
    let mut wire = Wire::TlsClient(Box::new(socket));
    send_hello(&mut wire, &security.fingerprint).await?;
    let _peer = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_hello(&mut wire))
        .await
        .map_err(|_| "等待对方身份信息超时".to_string())??;
    wire.write_all(format!("TEXT {payload}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    wire.flush().await.map_err(|e| e.to_string())?;
    let ack = tokio::time::timeout(ACK_TIMEOUT, read_line(&mut wire, 16))
        .await
        .map_err(|_| "等待文字消息确认超时".to_string())??;
    if ack.trim() != "OK" {
        return Err("对方未能接收文字消息".into());
    }
    wire.shutdown().await.map_err(|e| e.to_string())?;
    let _ = app;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn stream_entries(
    socket: &mut Wire,
    app: &AppHandle,
    task_id: &str,
    entries: Vec<SendEntry>,
    total_size: u64,
    batch_mode: bool,
    target_ip: &str,
    skip_bytes: u64,
) -> Result<(), String> {
    let mut buf = vec![0u8; CHUNK_SIZE];
    // Resume fast-forward: bytes already held by the receiver are counted as
    // sent for progress but never written to the wire.
    let mut sent_total: u64 = 0;
    let mut remaining_skip = skip_bytes;
    let mut last_emit = std::time::Instant::now();
    let mut speed_meter = SpeedMeter::new();

    for entry in entries {
        let mut reader: tokio::fs::File = match entry.preopened {
            Some(file) => tokio::fs::File::from_std(file),
            None => tokio::fs::File::open(&entry.absolute_path)
                .await
                .map_err(|e| format!("无法打开文件 {}：{e}", entry.display_name))?,
        };
        let mut sent_entry: u64 = 0;
        while sent_entry < entry.size {
            if is_cancelled(task_id) {
                return Err(CANCELLED_MESSAGE.into());
            }
            let want = (entry.size - sent_entry).min(CHUNK_SIZE as u64) as usize;
            let n = reader.read(&mut buf[..want]).await.map_err(|e| {
                let msg = format!("read file: {e}");
                eprintln!("{}", msg);
                msg
            })?;
            if n == 0 {
                return Err(format!("源文件在读取中途变短：{}", entry.display_name));
            }
            // Resume fast-forward inside a chunk boundary.
            if remaining_skip > 0 {
                let skippable = (n as u64).min(remaining_skip);
                sent_entry += skippable;
                sent_total += skippable;
                remaining_skip -= skippable;
                if skippable == n as u64 {
                    continue;
                }
                let take = &buf[skippable as usize..];
                socket.write_all(take).await.map_err(|e| {
                    let msg = format!("write socket: {e}");
                    eprintln!("{}", msg);
                    msg
                })?;
                sent_entry += take.len() as u64;
                sent_total += take.len() as u64;
                continue;
            }
            socket.write_all(&buf[..n]).await.map_err(|e| {
                let msg = format!("write socket: {e}");
                eprintln!("{}", msg);
                msg
            })?;
            sent_entry += n as u64;
            sent_total += n as u64;

            let progress = if total_size > 0 {
                (sent_total as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };
            let speed = speed_meter.tick(sent_total, sent_total == total_size);
            if last_emit.elapsed().as_millis() > 100 || sent_total == total_size {
                let _ = app.emit(
                    "transfer_progress",
                    TransferProgress {
                        task_id: task_id.to_string(),
                        file_name: entry.display_name.clone(),
                        progress,
                        speed,
                        transferred: sent_total,
                        total: total_size,
                        direction: "send".into(),
                        peer: target_ip.to_string(),
                    },
                );
                last_emit = std::time::Instant::now();
            }
        }
        // In folder mode show per-file completion through the progress event.
        let _ = batch_mode;
    }
    Ok(())
}

fn emit_send_failed(app: &AppHandle, task_id: &str, file_name: &str, peer: &str, message: String) {
    let _ = app.emit(
        "transfer_complete",
        TransferResult {
            task_id: task_id.to_string(),
            file_name: file_name.to_string(),
            path: String::new(),
            open_path: None,
            success: false,
            message,
            direction: "send".into(),
            peer: peer.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collect_send_entries_lists_files_recursively() {
        let base = std::env::temp_dir().join(format!("flashlan-test-{}", uuid::Uuid::new_v4()));
        let nested = base.join("a/b");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::write(base.join("top.txt"), b"hello")
            .await
            .unwrap();
        tokio::fs::write(nested.join("deep.bin"), [0u8; 3])
            .await
            .unwrap();

        let (root, entries, total) = collect_send_entries(&base).await.unwrap();
        assert_eq!(root, base.file_name().unwrap().to_str().unwrap());
        assert_eq!(entries.len(), 2);
        assert_eq!(total, 8);
        assert!(entries.iter().any(|e| e.relative_path == "top.txt"));
        assert!(entries
            .iter()
            .any(|e| e.relative_path.replace('\\', "/") == "a/b/deep.bin"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn collect_send_entries_rejects_empty_folder() {
        let base = std::env::temp_dir().join(format!("flashlan-empty-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&base).await.unwrap();
        assert!(collect_send_entries(&base).await.is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn single_file_display_name_keeps_extension() {
        let base = std::env::temp_dir().join(format!("flashlan-image-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&base).await.unwrap();
        let image = base.join("photo.jpg");
        tokio::fs::write(&image, [0xff, 0xd8, 0xff]).await.unwrap();

        let (_, entries, _) = collect_send_entries(&image).await.unwrap();
        assert_eq!(single_file_display_name(&entries), "photo.jpg");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn safe_relative_path_blocks_traversal() {
        assert!(safe_relative_path("docs/../..//etc/passwd").is_none());
        assert!(safe_relative_path("../secret").is_none());
        assert_eq!(
            safe_relative_path("a/b/c.txt").unwrap(),
            PathBuf::from("a/b/c.txt")
        );
        assert_eq!(safe_relative_path("").is_none(), true);
    }

    #[test]
    fn unique_dir_avoids_existing() {
        let base = std::env::temp_dir().join(format!("flashlan-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir(base.join("folder")).unwrap();
        let chosen = unique_dir(&base, "folder");
        assert_eq!(chosen.file_name().unwrap(), "folder_1");
        let _ = std::fs::remove_dir_all(&base);
    }
}
