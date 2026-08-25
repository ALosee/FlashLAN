use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub task_id: String,
    pub file_name: String,
    pub progress: f64,
    pub speed: f64,
    pub transferred: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub task_id: String,
    pub file_name: String,
    pub path: String,
    pub success: bool,
    pub message: String,
}

pub const TRANSFER_PORT: u16 = 17321;
const CHUNK_SIZE: usize = 64 * 1024;

/// Header sent before file bytes: JSON + newline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHeader {
    pub file_name: String,
    pub file_size: u64,
    pub task_id: String,
}

pub async fn start_file_server(app: AppHandle, save_dir: PathBuf) -> Result<(), String> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TRANSFER_PORT))
        .await
        .map_err(|e| format!("bind transfer port: {e}"))?;
    println!("FlashLAN file server listening on {}", TRANSFER_PORT);
    let app = Arc::new(app);
    tokio::spawn(async move {
        loop {
            let Ok((socket, addr)) = listener.accept().await else { continue };
            let app_clone = app.clone();
            let save_dir_clone = save_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_incoming(socket, save_dir_clone, app_clone).await {
                    eprintln!("handle incoming from {addr}: {e}");
                }
            });
        }
    });
    Ok(())
}

async fn handle_incoming(mut socket: TcpStream, save_dir: PathBuf, app: Arc<AppHandle>) -> Result<(), String> {
    // Read header line (JSON + \n)
    let mut header_buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = socket.read(&mut byte).await.map_err(|e| e.to_string())?;
        if n == 0 { return Err("connection closed before header".into()); }
        if byte[0] == b'\n' { break; }
        header_buf.push(byte[0]);
        if header_buf.len() > 8192 { return Err("header too large".into()); }
    }
    let header_str = String::from_utf8_lossy(&header_buf).to_string();
    println!("Received header: {}", header_str);
    let header: FileHeader = serde_json::from_slice(&header_buf).map_err(|e| {
        let msg = format!("header json: {e} header={}", header_str);
        eprintln!("{}", msg);
        msg
    })?;
    println!("Parsed header: file_name={} size={} task_id={}", header.file_name, header.file_size, header.task_id);
    let file_path = save_dir.join(&header.file_name);
    println!("Saving to {:?}", file_path);
    // Avoid overwrite: add suffix if exists
    let mut final_path = file_path.clone();
    let mut counter = 1;
    while final_path.exists() {
        let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = file_path.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        final_path = save_dir.join(format!("{stem}_{counter}{ext}"));
        counter += 1;
    }
    println!("Creating file {:?}", final_path);
    let mut file = tokio::fs::File::create(&final_path).await.map_err(|e| {
        let msg = format!("create file {:?}: {e}", final_path);
        eprintln!("{}", msg);
        msg
    })?;
    println!("File created, start receiving");
    let mut received: u64 = 0;
    let total = header.file_size;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let start = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    loop {
        let n = socket.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        received += n as u64;
        let progress = if total > 0 { (received as f64 / total as f64) * 100.0 } else { 0.0 };
        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 { received as f64 / elapsed } else { 0.0 };
        if last_emit.elapsed().as_millis() > 100 || received == total {
            let _ = app.emit("transfer_progress", TransferProgress {
                task_id: header.task_id.clone(),
                file_name: header.file_name.clone(),
                progress,
                speed,
                transferred: received,
                total,
            });
            last_emit = std::time::Instant::now();
        }
        if received >= total { break; }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    let _ = app.emit("transfer_complete", TransferResult {
        task_id: header.task_id.clone(),
        file_name: header.file_name.clone(),
        path: final_path.to_string_lossy().to_string(),
        success: true,
        message: "received".into(),
    });
    // Ack
    let _ = socket.write_all(b"OK\n").await;
    Ok(())
}

pub async fn send_file(
    path: String,
    target_ip: String,
    target_port: u16,
    task_id_opt: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    use std::path::Path;
    println!("send_file request: path={} target={}:{}", path, target_ip, target_port);
    let p = Path::new(&path);
    if !p.exists() { 
        let msg = format!("file not found: {path}");
        eprintln!("{}", msg);
        return Err(msg); 
    }
    let metadata = tokio::fs::metadata(p).await.map_err(|e| e.to_string())?;
    if !metadata.is_file() { return Err("only files supported in MVP, folders TODO".into()); }
    let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let file_size = metadata.len();
    let task_id = task_id_opt.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let header = FileHeader { file_name: file_name.clone(), file_size, task_id: task_id.clone() };
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
    println!("Connected to {}, sending header {}", addr, header_json.trim());
    socket.write_all(header_json.as_bytes()).await.map_err(|e| {
        let msg = format!("write header: {e}");
        eprintln!("{}", msg);
        msg
    })?;
    println!("Header sent, opening file {:?}", p);
    let mut file = tokio::fs::File::open(p).await.map_err(|e| {
        let msg = format!("open file {:?}: {e}", p);
        eprintln!("{}", msg);
        msg
    })?;
    println!("File opened, size {}", file_size);
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
        println!("Sent {}/{} ({:.1}%)", sent, file_size, if file_size>0 { sent as f64 / file_size as f64 *100.0 } else {0.0});
        let progress = if file_size > 0 { (sent as f64 / file_size as f64) * 100.0 } else { 0.0 };
        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 { sent as f64 / elapsed } else { 0.0 };
        if last_emit.elapsed().as_millis() > 100 || sent == file_size {
            println!("Emit progress {}% {}/{} speed {:.1}", progress, sent, file_size, speed);
            let _ = app.emit("transfer_progress", TransferProgress {
                task_id: task_id.clone(),
                file_name: file_name.clone(),
                progress,
                speed,
                transferred: sent,
                total: file_size,
            });
            last_emit = std::time::Instant::now();
        }
    }
    socket.flush().await.map_err(|e| e.to_string())?;
    // Wait for ack with timeout
    let mut ack = [0u8; 3];
    let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut ack)).await;
    let _ = app.emit("transfer_complete", TransferResult {
        task_id: task_id.clone(),
        file_name,
        path: path.clone(),
        success: true,
        message: "sent".into(),
    });
    Ok(task_id)
}

use std::time::Duration;
