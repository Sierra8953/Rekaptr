//! Local HTTP file server for HLS playback.
//!
//! Luma records gameplay as fMP4 segments with an HLS (M3U8) playlist. For
//! playback, we hand this playlist to mpv or other HLS-aware players. The
//! problem: HLS players typically refuse to load fMP4 segments from `file://`
//! URIs because the HLS spec is HTTP-based and players enforce this.
//!
//! This module spins up a minimal HTTP/1.1 server on `127.0.0.1` that serves
//! the recording directory over HTTP with proper MIME types and `Range` request
//! support (required for seeking within large segments).
//!
//! The port is auto-selected from a free port to avoid conflicts with other
//! services. The chosen port is stored in a global `OnceLock` so the rest of
//! the app (playback, etc.) can build URLs against it.
//!
//! It runs on its own single-threaded tokio runtime to stay fully isolated
//! from Luma's main async runtime — a slow file read here can't starve the
//! UI or recording pipeline.

use std::path::PathBuf;
use std::sync::OnceLock;

static SERVER_PORT: OnceLock<u16> = OnceLock::new();

/// Store the port chosen by `start_local_server` so other modules can build URLs.
pub fn set_server_port(port: u16) {
    let _ = SERVER_PORT.set(port);
}

/// Returns the port the HLS server is listening on, or 0 if it hasn't started.
pub fn get_server_port() -> u16 {
    SERVER_PORT.get().copied().unwrap_or(0)
}

/// Returns the base URL for the local HLS server (e.g., `http://127.0.0.1:9123`).
pub fn base_url() -> String {
    format!("http://127.0.0.1:{}", get_server_port())
}

/// Start the local HLS file server on a dedicated thread with its own tokio runtime.
///
/// Binds to `127.0.0.1:0` to let the OS pick a free port, avoiding conflicts with
/// other services on 8080. Returns the actual port bound so the caller can store it.
///
/// Serves files under `root`. Supports GET requests with optional `Range` headers
/// for seeking. Runs until the process exits.
pub fn start_local_server(root: PathBuf) -> u16 {
    // Bind synchronously on the current thread so we can return the port immediately
    let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            log::error!("[Server] Failed to bind local HLS server: {}", e);
            return 0;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    log::info!("[Server] Local HLS server listening on http://127.0.0.1:{}", port);

    // Move the std listener into a background thread with its own tokio runtime
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log::error!("[Server] Failed to create tokio runtime: {}", e);
                return;
            }
        };

        rt.block_on(async move {
            // Convert the std listener to a tokio listener
            listener.set_nonblocking(true).ok();
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(e) => {
                    log::error!("[Server] Failed to convert listener: {}", e);
                    return;
                }
            };

            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(res) => res,
                    Err(_) => continue,
                };

                let root = root.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0; 8192];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };

                    let request = String::from_utf8_lossy(&buf[..n]);
                    let first_line = request.lines().next().unwrap_or("");
                    let parts: Vec<&str> = first_line.split_whitespace().collect();
                    if parts.len() < 2 || parts[0] != "GET" { return; }

                    let url_path = parts[1].split('?').next().unwrap_or("").trim_start_matches('/');
                    let decoded_path = url_path.replace("%20", " ");
                    let file_path = root.join(decoded_path);

                    let range_header = request.lines().find(|l| l.to_lowercase().starts_with("range:"));

                    if file_path.exists() && file_path.is_file() {
                        let content_type = match file_path.extension().and_then(|e| e.to_str()) {
                            Some("m3u8") => "application/vnd.apple.mpegurl",
                            Some("m4s") => "video/iso.segment",
                            Some("mp4") => "video/mp4",
                            Some("ts") => "video/mp2t",
                            _ => "application/octet-stream",
                        };

                        if let Ok(mut file) = tokio::fs::File::open(&file_path).await {
                            let file_len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
                            let mut start = 0;
                            let mut end = file_len.saturating_sub(1);
                            let mut is_partial = false;

                            if let Some(range) = range_header {
                                if let Some(r) = range.to_lowercase().strip_prefix("range: bytes=") {
                                    let r_parts: Vec<&str> = r.trim().split('-').collect();
                                    if r_parts.len() == 2 {
                                        if let Ok(s) = r_parts[0].parse::<u64>() { start = s; }
                                        if !r_parts[1].is_empty() {
                                            if let Ok(e) = r_parts[1].parse::<u64>() { end = e; }
                                        }
                                        is_partial = true;
                                    }
                                }
                            }

                            let content_len = (end + 1).saturating_sub(start);
                            use tokio::io::AsyncSeekExt;
                            let _ = file.seek(std::io::SeekFrom::Start(start)).await;

                            let status = if is_partial { "206 Partial Content" } else { "200 OK" };
                            let mut response = format!(
                                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\n",
                                status, content_type, content_len
                            );
                            if is_partial {
                                response.push_str(&format!("Content-Range: bytes {}-{}/{}\r\n", start, end, file_len));
                            }
                            response.push_str("\r\n");

                            if socket.write_all(response.as_bytes()).await.is_ok() {
                                let mut remaining = content_len;
                                let mut read_buf = [0u8; 16384];
                                while remaining > 0 {
                                    let to_read = remaining.min(read_buf.len() as u64) as usize;
                                    match file.read_exact(&mut read_buf[..to_read]).await {
                                        Ok(_) => {
                                            if socket.write_all(&read_buf[..to_read]).await.is_err() { break; }
                                            remaining -= to_read as u64;
                                        }
                                        Err(_) => break,
                                    }
                                }
                            }
                            return;
                        }
                    }

                    let _ = socket.write_all(b"HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n").await;
                });
            }
        });
    });

    port
}
