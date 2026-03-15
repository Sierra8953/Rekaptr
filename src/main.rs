//! Luma — GPU-accelerated screen recording for Windows.
//!
//! Boot sequence:
//! 1. GStreamer init (must happen before any pipeline construction)
//! 2. GPUI application + window creation with custom violet theme
//! 3. Asset loading from `./assets/` for UI icons and fonts
//! 4. Mic provider startup (shared audio capture context, reused across recordings)
//! 5. Background buffer cleanup thread (filesystem watcher, enforces retention limits)
//! 6. Local HLS server for instant in-app playback of recorded segments
//! 7. Auto-record event loop (polls for game windows, starts/stops recording automatically)
//!
//! The D3D11 device handle is extracted from the GPUI window and shared with the video
//! player so decoded frames stay on the GPU — no readback needed for preview rendering.

mod audio;
mod auto_record;
mod config;
mod db;
mod engine;
mod game_detector;
mod segment_verify;
mod server;
mod state;
mod tray;
mod ui;
mod utils;
mod video_player;
pub mod virtual_audio_router;

/// mimalloc significantly reduces allocation overhead for the high-throughput
/// audio/video buffer paths compared to the default Windows allocator.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

use crate::state::AppState;
use crate::ui::LumaWorkspace;
use anyhow::Result;
use gpui::*;
use std::sync::Arc;
use std::path::PathBuf;

/// Runtime asset loader for GPUI. Reads icons and fonts from disk rather than
/// embedding them in the binary, which keeps build times fast and allows asset
/// hot-swapping during development.
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
    let total_start = std::time::Instant::now();

    // Handle --version before any heavy init
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("Luma {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("[Main] Starting Luma v{}...", env!("CARGO_PKG_VERSION"));

    // Check for ffmpeg before we get too far — export and segment probing depend on it
    let ffmpeg_path = crate::utils::get_ffmpeg_path();
    if !ffmpeg_path.exists() {
        log::warn!("[Main] ffmpeg.exe not found at {:?} — clip export will not work", ffmpeg_path);
    }

    let _app_start = std::time::Instant::now();
    let app = gpui::Application::new();

    let assets = Assets {
        base: std::env::current_dir()?.join("assets"),
    };

    app.with_assets(assets).run(move |cx: &mut gpui::App| {
        adabraka_ui::init(cx);
        adabraka_ui::set_icon_base_path("icons");

        // Define the Slint-consistent Violet theme
        let mut theme = adabraka_ui::theme::Theme::dark();
        theme.tokens.primary = gpui::hsla(258.0/360.0, 0.90, 0.66, 1.0);
        theme.tokens.background = gpui::rgb(0x09090b).into();
        theme.tokens.card = gpui::rgb(0x18181b).into();
        theme.tokens.border = gpui::rgb(0x3f3f46).into();

        adabraka_ui::theme::install_theme(cx, theme);

        let app_state = Arc::new(AppState::new());

        // Start Mic Provider
        {
            let config = crate::config::AppConfig::load();
            let provider_storage = app_state.mic_provider.clone();
            let device_name = config.mic_settings.device_name.clone();
            crate::audio::start_mic_provider(provider_storage, device_name);
        }

        // Start the background buffer cleanup thread
        crate::utils::start_buffer_cleanup_thread(crate::utils::get_storage_root());

        // Start the local HLS server (auto-selects a free port)
        let hls_port = server::start_local_server(crate::utils::get_storage_root());
        server::set_server_port(hls_port);

        let workspace_handle = Arc::new(std::sync::Mutex::new(None));
        let workspace_handle_clone = workspace_handle.clone();

        // Restore saved window bounds, or center a default-sized window
        let config_for_bounds = crate::config::AppConfig::load();
        let bounds = if let Some(wb) = &config_for_bounds.window_bounds {
            Bounds::new(
                point(px(wb.x as f32), px(wb.y as f32)),
                size(px(wb.width as f32), px(wb.height as f32)),
            )
        } else {
            Bounds::centered(None, size(px(1400.0), px(900.0)), cx)
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.0), px(600.0))),
            ..Default::default()
        };

        match cx.open_window(options, |window, cx| {
            if let Some(device) = window.direct3d11_device() {
                use windows::Win32::Foundation::HANDLE;
                *app_state.d3d11_device.lock() = Some(crate::video_player::SendHandle(HANDLE(device as _)));
            }
            let view = cx.new(|cx| LumaWorkspace::new(app_state.clone(), window, cx));
            if let Ok(mut handle) = workspace_handle_clone.lock() {
                *handle = Some(view.downgrade());
            }
            
            log::info!("[Main] Boot sequence complete in {:?}", total_start.elapsed());
            view
        }) {
            Ok(_) => {}
            Err(e) => {
                log::error!("[Main] Failed to open window: {:?}", e);
                return;
            }
        }

        // Start system tray icon and global hotkey
        {
            let (instant_replay_tx, instant_replay_rx) = std::sync::mpsc::channel::<()>();
            tray::start_tray_thread(app_state.is_recording.clone(), instant_replay_tx);

            // Spawn a background thread that listens for instant replay hotkey presses
            let replay_state = app_state.clone();
            std::thread::Builder::new()
                .name("Instant Replay Listener".to_string())
                .spawn(move || {
                    while let Ok(()) = instant_replay_rx.recv() {
                        log::info!("[Tray] Instant replay hotkey triggered");
                        if !replay_state.is_recording.load(std::sync::atomic::Ordering::Relaxed) {
                            log::warn!("[Tray] Not recording — ignoring instant replay hotkey");
                            continue;
                        }
                        let config = crate::config::AppConfig::load();
                        let secs = config.instant_replay_secs;
                        let state = replay_state.clone();
                        // Fire the export on a separate thread so we don't block the listener
                        std::thread::spawn(move || {
                            crate::engine::export_instant_replay(&state, secs);
                        });
                    }
                })
                .ok();
        }

        // Auto-Record Event-Driven Logic
        auto_record::spawn_auto_record_loop(cx, app_state.clone(), workspace_handle);
    });
    Ok(())
}
