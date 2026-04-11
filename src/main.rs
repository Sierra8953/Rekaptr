mod audio;
mod config;
mod db;
mod engine;
mod game_detector;
mod hotkeys;
mod mic_dsp;
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
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use crate::state::TrayCommand;

/// Random token for authenticating local HLS server requests.
/// Prevents other local processes from accessing recording segments.
static HLS_TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn get_hls_token() -> &'static str {
    HLS_TOKEN.get_or_init(|| {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        let s = RandomState::new();
        let mut hasher = s.build_hasher();
        hasher.write_u64(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64);
        format!("{:016x}", hasher.finish())
    })
}

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
    crate::engine::boost_gpu_priority();
    let (major, minor, micro, nano) = gstreamer::version();
    log::info!("[Main] GStreamer version: {}.{}.{}.{}", major, minor, micro, nano);
    // Validate configured encoder is available, auto-fallback if not
    {
        let mut config = crate::config::AppConfig::load();
        config.validate_and_fix_encoder();
    }

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

        // Enumerate audio devices at startup
        {
            *app_state.audio_output_devices.lock() = crate::engine::enumerate_audio_devices(false);
            *app_state.audio_input_devices.lock() = crate::engine::enumerate_audio_devices(true);
        }

        // Start Mic Provider
        {
            let config = crate::config::AppConfig::load();
            let provider_storage = app_state.mic_provider.clone();
            let device_name = config.mic_settings.device_name.clone();
            // In a real app we'd map name to ID, for now we use "Default" or the name directly
            crate::audio::start_mic_provider(provider_storage, device_name);
        }

        // Start the background buffer cleanup thread
        crate::utils::start_buffer_cleanup_thread(crate::utils::get_storage_root());

        // Start the local HLS server
        start_local_server(crate::utils::get_storage_root());

        let workspace_handle: Arc<std::sync::Mutex<Option<WeakEntity<LumaWorkspace>>>> = Arc::new(std::sync::Mutex::new(None));
        let workspace_handle_clone = workspace_handle.clone();

        // --- System Tray Initialization ---
        let tray_menu = Menu::new();
        let show_item = MenuItem::new("Open Luma", true, None);
        let stop_item = MenuItem::new("Stop Recording", false, None); // Disabled by default
        let quit_item = MenuItem::new("Quit", true, None);

        let _ = tray_menu.append_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &stop_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Luma Recording")
            .build()
            .unwrap();

        let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<TrayCommand>();
        *app_state.tray_tx.lock() = Some(tray_tx);

        // --- Tray Event Loop ---
        let workspace_handle_tray = workspace_handle.clone();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let menu_channel = MenuEvent::receiver();
                let _tray_channel = TrayIconEvent::receiver();
                loop {
                    // Handle tray commands from app logic
                    while let Ok(cmd) = tray_rx.try_recv() {
                        match cmd {
                            TrayCommand::SetStopEnabled(enabled) => {
                                stop_item.set_enabled(enabled);
                            }
                        }
                    }

                    if let Ok(event) = menu_channel.try_recv() {
                        let workspace_handle = match workspace_handle_tray.lock() {
                            Ok(h) => h.clone(),
                            Err(_) => continue,
                        };

                        if event.id == show_item.id() {
                            let _ = cx.update(|cx| {
                                if let Some(window) = cx.windows().first() {
                                    let _ = window.update(cx, |_, window: &mut Window, _| {
                                        window.show_window();
                                        window.activate_window();
                                    });
                                }
                            });
                        } else if event.id == stop_item.id() {
                            if let Some(workspace_weak) = workspace_handle {
                                let _ = cx.update(|cx| {
                                    if let Some(workspace_entity) = workspace_weak.upgrade() {
                                        if let Some(window_handle) = cx.windows().first().cloned() {
                                            let _ = window_handle.update(cx, |view: AnyView, window: &mut Window, cx: &mut App| {
                                                if let Ok(view) = view.downcast::<LumaWorkspace>() {
                                                    view.update(cx, |this, cx| {
                                                        if this.app_state.recording.phase.lock().is_recording() {
                                                            this.toggle_recording(window, cx);
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    }
                                });
                            }
                        } else if event.id == quit_item.id() {
                            cx.update(|cx| cx.quit()).ok();
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }).detach();
        // Keep tray_icon alive by moving it into the background task (effectively)
        // or just dropping it at the end of the run closure might be okay if run() blocks,
        // but GPUI's run() actually enters the event loop and might not block in a way that
        // prevents the closure's local variables from being dropped if not captured.
        // Capturing it in the spawn above is best.
        let _keep_alive = tray_icon;

        // --- Disk Space Monitoring ---
        let storage_root = crate::utils::get_storage_root();
        let workspace_handle_disk = workspace_handle.clone();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                use sysinfo::Disks;
                let mut disks = Disks::new_with_refreshed_list();
                loop {
                    cx.background_executor().timer(std::time::Duration::from_secs(10)).await;
                    disks.refresh_list();

                    let target_disk = disks.iter().find(|d| storage_root.starts_with(d.mount_point()));
                    if let Some(disk) = target_disk {
                        let free_gb = disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;

                        if free_gb < 1.0 || free_gb < 5.0 {
                            let workspace_handle = match workspace_handle_disk.lock() {
                                Ok(h) => h.clone(),
                                Err(_) => continue,
                            };
                            if let Some(workspace_weak) = workspace_handle {
                                let _ = cx.update(|cx| {
                                    if let Some(workspace_entity) = workspace_weak.upgrade() {
                                        if let Some(any_window_handle) = cx.windows().first().cloned() {
                                            let _ = any_window_handle.update(cx, |_, window: &mut Window, cx| {
                                                workspace_entity.update(cx, |workspace, cx| {
                                                    if free_gb < 1.0 {
                                                        // CRITICAL: Stop recording
                                                        if workspace.app_state.recording.phase.lock().is_recording() {
                                                            workspace.show_toast(
                                                                "Disk Full",
                                                                Some("Recording stopped to prevent data loss.".to_string()),
                                                                adabraka_ui::overlays::toast::ToastVariant::Error,
                                                                window,
                                                                cx,
                                                            );
                                                            workspace.toggle_recording(window, cx);
                                                        }
                                                    } else {
                                                        // WARNING: Notify user
                                                        workspace.show_toast(
                                                            "Disk Space Low",
                                                            Some(format!("{:.1} GB remaining on recording drive", free_gb)),
                                                            adabraka_ui::overlays::toast::ToastVariant::Default,
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                });
                                            });
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }).detach();
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.0), px(600.0))),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            if let Some(device) = window.direct3d11_device() {
                use windows::Win32::Foundation::HANDLE;
                crate::engine::boost_device_gpu_priority(device as *mut std::ffi::c_void);
                // SAFETY: AddRef the D3D11 device so our stored handle keeps the device alive
                // independently of the GPUI window's own reference. The device pointer comes from
                // GPUI's window and is valid here. We call AddRef to take shared ownership;
                // the matching Release happens when SendHandle is dropped (not currently
                // implemented — acceptable because the device lives for the entire app lifetime
                // alongside the single window).
                // AddRef so our stored copy keeps the device alive
                unsafe {
                    use windows::Win32::Graphics::Direct3D11::ID3D11Device;
                    use windows::core::Interface;
                    // from_raw wraps the pointer; ManuallyDrop prevents Release on drop
                    let d3d = std::mem::ManuallyDrop::new(ID3D11Device::from_raw(device as *mut std::ffi::c_void));
                    // Clone calls AddRef internally via the windows crate
                    let _prevent_drop = std::mem::ManuallyDrop::new((*d3d).clone());
                }
                *app_state.d3d11_device.lock() = Some(crate::video_player::SendHandle(HANDLE(device as _)));
            }
            let view = cx.new(|cx| LumaWorkspace::new(app_state.clone(), window, cx));
            if let Ok(mut handle) = workspace_handle_clone.lock() {
                *handle = Some(view.downgrade());
            }
            view
        }).ok();

        // Global Hotkey Listener
        {
            let hotkey_rx = crate::hotkeys::start_hotkey_listener();
            let workspace_handle_hotkey = workspace_handle.clone();

            cx.spawn(|cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    loop {
                        // Check for hotkey events every 50ms
                        cx.background_executor().timer(std::time::Duration::from_millis(50)).await;

                        while let Ok(action) = hotkey_rx.try_recv() {
                            let workspace_handle = match workspace_handle_hotkey.lock() {
                                Ok(h) => h.clone(),
                                Err(_) => continue,
                            };
                            if let Some(workspace_weak) = workspace_handle {
                                let _ = cx.update(|cx| {
                                    if let Some(workspace_entity) = workspace_weak.upgrade() {
                                        if let Some(any_window) = cx.windows().first().cloned() {
                                            let _ = any_window.update(cx, |_, window, cx| {
                                                workspace_entity.update(cx, |workspace, cx| {
                                                    match action {
                                                        crate::hotkeys::HotkeyAction::ToggleRecording => {
                                                            workspace.toggle_recording(window, cx);
                                                        }
                                                        crate::hotkeys::HotkeyAction::SaveClip => {
                                                            workspace.save_clip(window, cx);
                                                        }
                                                        crate::hotkeys::HotkeyAction::ToggleMic => {
                                                            // Toggle the first Mic track's enabled state
                                                            for track in workspace.form_audio_tracks.iter_mut() {
                                                                if track.source_type == "Mic" {
                                                                    track.enabled = !track.enabled;
                                                                    let status = if track.enabled { "unmuted" } else { "muted" };
                                                                    workspace.show_toast(
                                                                        "Microphone",
                                                                        Some(format!("Mic {}", status)),
                                                                        adabraka_ui::overlays::toast::ToastVariant::Default,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                    break;
                                                                }
                                                            }
                                                            cx.notify();
                                                        }
                                                        crate::hotkeys::HotkeyAction::PushToTalk => {
                                                            // TODO: implement push-to-talk hold/release
                                                        }
                                                        crate::hotkeys::HotkeyAction::MarkerFlag => {
                                                            workspace.add_marker_with_kind(crate::state::MarkerKind::Flag, cx);
                                                        }
                                                        crate::hotkeys::HotkeyAction::MarkerKill => {
                                                            workspace.add_marker_with_kind(crate::state::MarkerKind::Kill, cx);
                                                        }
                                                        crate::hotkeys::HotkeyAction::MarkerDeath => {
                                                            workspace.add_marker_with_kind(crate::state::MarkerKind::Death, cx);
                                                        }
                                                        crate::hotkeys::HotkeyAction::MarkerHighlight => {
                                                            workspace.add_marker_with_kind(crate::state::MarkerKind::Highlight, cx);
                                                        }
                                                    }
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
        }

        // Auto-Record Event-Driven Logic
        let app_state_auto = app_state.clone();
        let workspace_handle_auto = workspace_handle.clone();
        
        cx.spawn(|cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                use windows::Win32::UI::Accessibility::*;
                use windows::Win32::UI::WindowsAndMessaging::*;
                use windows::Win32::Foundation::HWND;

                // Channel to communicate between the hook thread and the processing loop
                let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(10);

                static HOOK_TX: std::sync::OnceLock<tokio::sync::mpsc::Sender<()>> = std::sync::OnceLock::new();

                // Spawn a dedicated thread for the Win32 hook (needs a message loop)
                std::thread::spawn(move || {
                    unsafe {
                        extern "system" fn winevent_callback(
                            _h_win_event_hook: HWINEVENTHOOK,
                            _event: u32,
                            _hwnd: HWND,
                            _id_object: i32,
                            _id_child: i32,
                            _dw_event_thread: u32,
                            _dw_ms_event_time: u32,
                        ) {
                            if let Some(tx) = HOOK_TX.get() {
                                let _ = tx.blocking_send(());
                            }
                        }

                        let _ = HOOK_TX.set(tx);

                        let hook = SetWinEventHook(
                            EVENT_SYSTEM_FOREGROUND,
                            EVENT_SYSTEM_FOREGROUND,
                            None,
                            Some(winevent_callback),
                            0,
                            0,
                            WINEVENT_OUTOFCONTEXT,
                        );

                        if !hook.is_invalid() {
                            let mut msg = MSG::default();
                            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                                DispatchMessageW(&msg);
                            }
                            UnhookWinEvent(hook);
                        }
                    }
                });

                let mut detector = crate::game_detector::GameDetector::new();
                
                // Initial scan on startup
                let _ = detector.enumerate_windows(); 

                loop {
                    // Wait for either a focus change event or a periodic fallback (every 60s)
                    tokio::select! {
                        _ = rx.recv() => {
                            // Focus changed! Wait a moment for the window to fully initialize
                            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                        }
                        _ = cx.background_executor().timer(std::time::Duration::from_secs(60)) => {
                            // Fallback scan
                        }
                    }

                    // Periodically evict oversized caches
                    app_state_auto.evict_caches();

                    let is_recording = app_state_auto.recording.phase.lock().is_recording();
                    if is_recording { continue; }

                    let config = crate::config::AppConfig::load();
                    let windows = detector.enumerate_windows();
                    
                    let mut target_match = None;
                    
                    for (game_title, settings) in &config.game_registry {
                        if !settings.auto_record { continue; }

                        if let Some(target_proc) = &settings.target_process {
                            if let Some(win) = windows.iter().find(|w| &w.process_name == target_proc) {
                                target_match = Some((game_title.clone(), win.hwnd));
                                break;
                            }
                        }
                    }
                    if let Some((game, hwnd)) = target_match {
                        let workspace_handle = match workspace_handle_auto.lock() {
                            Ok(h) => h.clone(),
                            Err(_) => continue,
                        };
                        if let Some(workspace_weak) = workspace_handle {
                            let _ = cx.update(|cx| {
                                if let Some(workspace_entity) = workspace_weak.upgrade() {
                                    if let Some(any_window) = cx.windows().first().cloned() {
                                        let _ = any_window.update(cx, |_, window, cx| {
                                            workspace_entity.update(cx, |workspace, cx| {
                                                log::info!("[Auto-Record] Event-Driven Match Found: {} (HWND: {})", game, hwnd);
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
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build() {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("[Server] Failed to build tokio runtime: {}", e);
                    return;
                }
            };

        rt.block_on(async move {
            let listener = match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
                Ok(l) => l,
                Err(e) => {
                    log::error!("[Server] Failed to bind local HLS server to 8080: {:?}", e);
                    return;
                }
            };
            log::info!("[Server] Local HLS server listening on http://127.0.0.1:8080");
            
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
                    
                    let raw_path = parts[1];
                    let (url_path, query) = raw_path.split_once('?').unwrap_or((raw_path, ""));
                    let url_path = url_path.trim_start_matches('/');

                    let decoded_path = url_path.replace("%20", " ");

                    // Validate token on playlist requests (the entry point).
                    // Segment files (.m4s/.mp4) are served without token since mpv
                    // resolves them from relative URLs in the playlist and can't
                    // forward query params. Server is already bound to 127.0.0.1.
                    let is_playlist = decoded_path.ends_with(".m3u8");
                    if is_playlist {
                        let token_valid = query.split('&')
                            .find_map(|p| p.strip_prefix("token="))
                            .map_or(false, |t| t == get_hls_token());
                        if !token_valid {
                            let _ = socket.write_all(b"HTTP/1.1 403 FORBIDDEN\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await;
                            return;
                        }
                    }
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
                                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: http://127.0.0.1:8080\r\nCache-Control: no-cache\r\n",
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
                    
                    let _ = socket.write_all(b"HTTP/1.1 404 NOT FOUND\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await;
                });
            }
        });
    });
}
