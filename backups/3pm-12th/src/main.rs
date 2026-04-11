mod audio;
mod config;
mod db;
mod engine;
mod game_detector;
mod state;
mod ui;
mod utils;
mod video_player;
pub mod virtual_audio_router;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

use crate::state::AppState;
use crate::ui::LumaWorkspace;
use anyhow::Result;
use gpui::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::path::PathBuf;

struct Assets {
    base: PathBuf,
}

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match std::fs::read(self.base.join(path)) {
            Ok(data) => Ok(Some(std::borrow::Cow::Owned(data))),
            Err(err) => {
                log::error!("[AssetSource] Failed to load asset '{}': {}", path, err);
                Err(err.into())
            }
        }
    }

    fn list(&self, path: &str) -> Result<Vec<gpui::SharedString>> {
        let entries = std::fs::read_dir(self.base.join(path))?;
        let mut list = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(name) = entry.file_name().into_string() {
                    list.push(gpui::SharedString::from(name));
                }
            }
        }
        Ok(list)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("[Main] Starting Luma...");
    gstreamer::init()?;
    let (major, minor, micro, nano) = gstreamer::version();
    eprintln!("[Main] GStreamer version: {}.{}.{}.{}", major, minor, micro, nano);
    let app = gpui::Application::new();

    let assets = Assets {
        base: std::env::current_dir()?.join("assets"),
    };

    app.with_assets(assets).run(move |cx: &mut gpui::App| {
        adabraka_ui::init(cx);
        adabraka_ui::set_icon_base_path("icons");
        
        // Define the Slint-consistent Violet theme
        let mut theme = adabraka_ui::theme::Theme::dark();
        theme.tokens.primary = gpui::hsla(258.0/360.0, 0.90, 0.66, 1.0); // Violet 500 (#8b5cf6)
        theme.tokens.background = gpui::rgb(0x09090b).into(); // Zinc 950
        theme.tokens.card = gpui::rgb(0x18181b).into(); // Zinc 900
        theme.tokens.border = gpui::rgb(0x3f3f46).into(); // Zinc 700
        
        adabraka_ui::theme::install_theme(cx, theme);
        
        let app_state = Arc::new(AppState::new());
        
        // Start Mic Provider
        {
            let config = crate::config::AppConfig::load();
            let provider_storage = app_state.mic_provider.clone();
            let device_name = config.mic_settings.device_name.clone();
            // In a real app we'd map name to ID, for now we use "Default" or the name directly
            crate::audio::start_mic_provider(provider_storage, device_name);
        }
        
        // Periodically update bitrate history for sparkline
        let bitrate_state = app_state.clone();
        cx.spawn(move |cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                use rand::RngExt;
                let mut rng = rand::rng();
                loop {
                    {
                        let mut history = bitrate_state.bitrate_history.lock();
                        // Simulate bitrate based on whether recording is active
                        let is_recording = bitrate_state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                        let current_bitrate = if is_recording {
                            rng.random_range(8000.0..12000.0)
                        } else {
                            0.0
                        };
                        history.remove(0);
                        history.push(current_bitrate);
                    }
                    let _ = cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                }
            }
        }).detach();
        
        // Start the background buffer cleanup thread
        crate::utils::start_buffer_cleanup_thread(crate::utils::get_storage_root());

        // Start the local HLS server
        start_local_server(crate::utils::get_storage_root());

        let workspace_handle = Arc::new(std::sync::Mutex::new(None));
        let workspace_handle_clone = workspace_handle.clone();

        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.0), px(600.0))),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            if let Some(device) = window.direct3d11_device() {
                use windows::Win32::Foundation::HANDLE;
                *app_state.d3d11_device.lock() = Some(crate::video_player::SendHandle(HANDLE(device as _)));
            }
            let view = cx.new(|cx| LumaWorkspace::new(app_state.clone(), window, cx));
            *workspace_handle_clone.lock().unwrap() = Some(view.downgrade());
            view
        }).unwrap();

        // Auto-Record Polling Loop
        let app_state_auto = app_state.clone();
        let workspace_handle_auto = workspace_handle.clone();
        cx.spawn(|cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let mut detector = crate::game_detector::GameDetector::new();
                  loop {
                      cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
                      
                      let is_recording = app_state_auto.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                      if is_recording { continue; }

                      let config = crate::config::AppConfig::load();
                      let windows = detector.enumerate_windows();
                      
                      let mut target_match = None;
                      
                      // Strategy: Iterate through our registry and find any game that has auto-record enabled
                      // and whose target process is currently in the list of open windows.
                      for (game_title, settings) in &config.game_registry {
                          if !settings.auto_record { continue; }
                          
                          if let Some(target_proc) = &settings.target_process {
                              if let Some(win) = windows.iter().find(|w| &w.process_name == target_proc) {
                                  target_match = Some((game_title.clone(), win.hwnd));
                                  break;
                              }
                          } else {
                              // Fallback: If no target process is saved (legacy source), try process mapping
                              for win in &windows {
                                  let mapped_title = config.custom_process_mapping.get(&win.process_name)
                                      .cloned()
                                      .unwrap_or_else(|| win.title.clone());
                                  
                                  if &mapped_title == game_title {
                                      target_match = Some((game_title.clone(), win.hwnd));
                                      break;
                                  }
                              }
                              if target_match.is_some() { break; }
                          }
                      }

                      if let Some((game, hwnd)) = target_match {
                          let workspace_handle = workspace_handle_auto.lock().unwrap().clone();
                          if let Some(workspace_weak) = workspace_handle {
                              let _ = cx.update(|cx| {
                                  if let Some(workspace_entity) = workspace_weak.upgrade() {
                                      if let Some(any_window) = cx.windows().first().cloned() {
                                          let _ = any_window.update(cx, |_, window, cx| {
                                              workspace_entity.update(cx, |workspace, cx| {
                                                  println!("[Auto-Record] Bulletproof Match Found: {} (HWND: {})", game, hwnd);
                                                  workspace.selected_source = Some(game.clone());
                                                  workspace.refresh_available_windows(cx);
                                                  workspace.toggle_recording_ext(Some(hwnd), window, cx);
                                              });
                                          });
                                      }
                                  }
                              });
                          }
                      }
                  }
            }
        }).detach();
        });
    Ok(())
}

fn start_local_server(root: PathBuf) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        rt.block_on(async move {
            let listener = match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[Server] Failed to bind local HLS server to 8080: {:?}", e);
                    return;
                }
            };
            eprintln!("[Server] Local HLS server listening on http://127.0.0.1:8080");
            
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
}
