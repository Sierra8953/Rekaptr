mod audio;
mod config;
mod engine;
mod game_detector;
mod state;
mod ui;
mod utils;
mod video_player;

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
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(|err| err.into())
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
    eprintln!("[Main] Starting Luma...");
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
            cx.new(|cx| LumaWorkspace::new(app_state.clone(), window, cx))
        }).unwrap();
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
                    let mut buf = [0; 4096];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let lines: Vec<&str> = request.lines().collect();
                    if lines.is_empty() { return; }
                    
                    let parts: Vec<&str> = lines[0].split_whitespace().collect();
                    if parts.len() < 2 || parts[0] != "GET" { return; }
                    
                    let path = parts[1].trim_start_matches('/');
                    // Simple URL decode for spaces
                    let path = path.replace("%20", " ");
                    let file_path = root.join(path);
                    
                    if file_path.exists() && file_path.is_file() {
                        let content_type = match file_path.extension().and_then(|e| e.to_str()) {
                            Some("m3u8") => "application/vnd.apple.mpegurl",
                            Some("m4s") => "video/iso.segment",
                            Some("mp4") => "video/mp4",
                            Some("mkv") => "video/x-matroska",
                            Some("ts") => "video/mp2t",
                            _ => "application/octet-stream",
                        };
                        
                        if let Ok(mut file) = tokio::fs::File::open(&file_path).await {
                            let mut contents = Vec::new();
                            if file.read_to_end(&mut contents).await.is_ok() {
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\n\r\n",
                                    content_type, contents.len()
                                );
                                let _ = socket.write_all(response.as_bytes()).await;
                                let _ = socket.write_all(&contents).await;
                                return;
                            }
                        }
                    }
                    
                    let _ = socket.write_all(b"HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n").await;
                });
            }
        });
    });
}
