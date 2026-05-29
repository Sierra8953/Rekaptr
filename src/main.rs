#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod audio;
mod config;
mod db;
mod engine;
mod game_detector;
mod hotkeys;
mod mic_dsp;
mod migration;
mod state;
mod ui;
mod updater;
mod utils;
mod video_player;
pub mod virtual_audio_router;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

use crate::state::AppState;
use crate::ui::RekaptrWorkspace;
use anyhow::Result;
use gpui::*;
use std::sync::Arc;
use std::path::PathBuf;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use crate::state::TrayCommand;

/// Cryptographically random token for authenticating local HLS server requests.
/// Prevents other local processes from accessing recording segments.
static HLS_TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Port the local HLS server is bound to. 0 means "not yet started or bind failed".
/// Picked at startup from a fallback range so a conflict on 8080 doesn't break playback.
static HLS_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

pub fn get_hls_token() -> &'static str {
    HLS_TOKEN.get_or_init(|| {
        use rand::RngExt;
        let bytes: [u8; 32] = rand::rng().random();
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    })
}

pub fn get_hls_port() -> u16 {
    HLS_PORT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Decode percent-encoded URL path components (e.g. %20 -> space, %2F -> /).
fn percent_decode_path(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                hex_val(bytes[i + 1]),
                hex_val(bytes[i + 2]),
            ) {
                result.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Initialize logging. In release builds the binary has `windows_subsystem =
/// "windows"` set, so there is no console attached and stderr writes go
/// nowhere. We route output to `%LOCALAPPDATA%\Rekaptr\rekaptr.log` (rolling
/// the previous run to `rekaptr.log.prev` on each launch). Debug builds keep
/// the default stderr destination so `cargo run` still prints.
fn init_logging() {
    let env = env_logger::Env::default().default_filter_or("info");
    let mut builder = env_logger::Builder::from_env(env);

    #[cfg(not(debug_assertions))]
    {
        let log_dir = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Rekaptr");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("rekaptr.log");
        let prev_path = log_dir.join("rekaptr.log.prev");
        let _ = std::fs::remove_file(&prev_path);
        let _ = std::fs::rename(&log_path, &prev_path);
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
    }

    let _ = builder.try_init();
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

/// Named event used to ask the running instance to surface its window.
#[cfg(target_os = "windows")]
const FOCUS_EVENT_NAME: windows::core::PCWSTR = windows::core::w!("Rekaptr_FocusEvent_v1");

/// Returns true if another instance of Rekaptr is already running.
///
/// Creates a named mutex; if it already exists, another instance owns it.
/// The handle is intentionally leaked so the mutex lives for this process's
/// lifetime — Windows releases it automatically when the process exits.
#[cfg(target_os = "windows")]
fn another_instance_running() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        match CreateMutexW(None, false, w!("Rekaptr_SingleInstance_Mutex_v1")) {
            Ok(_handle) => GetLastError() == ERROR_ALREADY_EXISTS,
            // If the mutex can't be created, fail open and allow launch.
            Err(_) => false,
        }
    }
}

/// Signals the already-running instance to bring its window to the foreground.
#[cfg(target_os = "windows")]
fn signal_existing_instance() {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};
    unsafe {
        if let Ok(handle) = OpenEventW(EVENT_MODIFY_STATE, false, FOCUS_EVENT_NAME) {
            let _ = SetEvent(handle);
            let _ = CloseHandle(handle);
        }
    }
}

/// Creates the auto-reset focus event for this (primary) instance. The running
/// instance polls it; a second launch sets it to request a window focus.
#[cfg(target_os = "windows")]
fn create_focus_event() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::Win32::System::Threading::CreateEventW;
    // auto-reset, initially non-signaled
    unsafe { CreateEventW(None, false, false, FOCUS_EVENT_NAME).ok() }
}

#[tokio::main]
async fn main() -> Result<()> {
    // --- Single-instance guard ---
    #[cfg(target_os = "windows")]
    if another_instance_running() {
        signal_existing_instance();
        return Ok(());
    }

    // --- Portable Path Configuration ---
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop(); // Root directory (next to rekaptr.exe)

        // Isolate GStreamer to only use bundled plugins. Windows resolves
        // top-level DLL imports from the EXE's own directory, so the support
        // DLLs alongside rekaptr.exe are picked up automatically — no PATH
        // manipulation is needed (and wouldn't help anyway, since imports
        // resolve before main() runs).
        let plugin_path = exe_path.join("lib").join("gstreamer-1.0");
        if plugin_path.exists() {
            let path_str = plugin_path.display().to_string();
            std::env::set_var("GST_PLUGIN_PATH", &path_str);
            // Block the system install to avoid the "already registered" error.
            // std::env::set_var("GST_PLUGIN_SYSTEM_PATH", &path_str);

            // Persist the plugin registry per-user (writable, survives launches).
            let registry_dir = std::env::var_os("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| exe_path.clone())
                .join("Rekaptr");
            let _ = std::fs::create_dir_all(&registry_dir);
            std::env::set_var(
                "GST_REGISTRY",
                registry_dir.join("gst-registry.bin").display().to_string(),
            );
        }
    }

    init_logging();
    log::info!("[Main] Starting Rekaptr...");
    crate::migration::run();
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

    // 3. Fix Asset Loading to be relative to the EXE
    let mut assets_base = std::env::current_exe()?;
    assets_base.pop(); 
    let assets = Assets {
        base: assets_base.join("assets"),
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
            crate::audio::start_mic_provider(provider_storage, device_name);
        }

        // Start the background buffer cleanup thread
        crate::utils::start_buffer_cleanup_thread(crate::utils::get_storage_root());

        // Start the local HLS server
        start_local_server(crate::utils::get_storage_root());

        let workspace_handle: Arc<std::sync::Mutex<Option<WeakEntity<RekaptrWorkspace>>>> = Arc::new(std::sync::Mutex::new(None));
        let workspace_handle_clone = workspace_handle.clone();

        // --- System Tray Initialization ---
        let tray_menu = Menu::new();
        let show_item = MenuItem::new("Open Rekaptr", true, None);
        let stop_item = MenuItem::new("Stop Recording", false, None); // Disabled by default
        let quit_item = MenuItem::new("Quit", true, None);

        // Build two icon variants: idle (base) and recording (red-dot overlay).
        // Each variant must own its own pixel buffer because tray_icon::Icon::from_rgba consumes it.
        fn make_recording_overlay(base_rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
            let mut buf = base_rgba.to_vec();
            // Red dot anchored to bottom-right, sized ~38% of the smaller dimension.
            let r = ((w.min(h) as f32) * 0.19).max(2.0);
            let cx = w as f32 - r - 1.0;
            let cy = h as f32 - r - 1.0;
            let r2 = r * r;
            let edge = (r * 0.85).powi(2); // inner solid radius for anti-aliased edge
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f32 + 0.5 - cx;
                    let dy = y as f32 + 0.5 - cy;
                    let d2 = dx * dx + dy * dy;
                    if d2 > r2 { continue; }
                    let alpha = if d2 < edge {
                        1.0
                    } else {
                        1.0 - ((d2.sqrt() - edge.sqrt()) / (r - edge.sqrt())).clamp(0.0, 1.0)
                    };
                    let i = ((y * w + x) * 4) as usize;
                    // Solid red over existing pixel.
                    buf[i]     = ((1.0 - alpha) * buf[i] as f32     + alpha * 230.0) as u8;
                    buf[i + 1] = ((1.0 - alpha) * buf[i + 1] as f32 + alpha *  30.0) as u8;
                    buf[i + 2] = ((1.0 - alpha) * buf[i + 2] as f32 + alpha *  30.0) as u8;
                    buf[i + 3] = 255;
                }
            }
            buf
        }

        // Keep the raw RGBA buffers so we can mint a fresh tray_icon::Icon each
        // time we switch state (the crate's Icon API consumes the buffer).
        let icon_buffers: Option<(Vec<u8>, Vec<u8>, u32, u32)> = {
            const ICON_BYTES: &[u8] = include_bytes!("../crates/gpui/examples/image/app-icon.ico");
            match image::load_from_memory(ICON_BYTES) {
                Ok(img) => {
                    let rgba = img.into_rgba8();
                    let (w, h) = rgba.dimensions();
                    let base = rgba.into_raw();
                    let rec = make_recording_overlay(&base, w, h);
                    Some((base, rec, w, h))
                }
                Err(e) => {
                    log::error!("[TrayIcon] decode failed: {}", e);
                    None
                }
            }
        };
        let make_icon = |rgba: &[u8], w: u32, h: u32| -> Option<tray_icon::Icon> {
            tray_icon::Icon::from_rgba(rgba.to_vec(), w, h)
                .map_err(|e| log::error!("[TrayIcon] from_rgba failed: {}", e))
                .ok()
        };

        let _ = tray_menu.append_items(&[
            &show_item,
            &PredefinedMenuItem::separator(),
            &stop_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

        let mut tray_builder = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Rekaptr");

        if let Some((base, _, w, h)) = icon_buffers.as_ref() {
            if let Some(icon) = make_icon(base, *w, *h) {
                tray_builder = tray_builder.with_icon(icon);
            }
        }

        let tray_icon = match tray_builder.build() {
            Ok(icon) => icon,
            Err(e) => {
                log::error!("[TrayIcon] Failed to create system tray: {}", e);
                panic!("System tray is required for Rekaptr to run");
            }
        };
        let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<TrayCommand>();
        *app_state.tray_tx.lock() = Some(tray_tx);

        // --- Tray Event Loop ---
        let workspace_handle_tray = workspace_handle.clone();
        cx.spawn(move |cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                // Hold ownership of the TrayIcon here so it lives for the entire
                // app lifetime. Dropping it calls Shell_NotifyIcon(NIM_DELETE)
                // and removes the icon from the system tray.
                let tray_icon = tray_icon;
                let menu_channel = MenuEvent::receiver();
                let _tray_channel = TrayIconEvent::receiver();
                #[cfg(target_os = "windows")]
                let focus_event = create_focus_event();
                loop {
                    // A second launch signals this event to ask us to surface
                    // the window instead of starting a new process.
                    #[cfg(target_os = "windows")]
                    if let Some(ev) = focus_event {
                        use windows::Win32::Foundation::WAIT_OBJECT_0;
                        use windows::Win32::System::Threading::WaitForSingleObject;
                        if unsafe { WaitForSingleObject(ev, 0) } == WAIT_OBJECT_0 {
                            let _ = cx.update(|cx| {
                                if let Some(window) = cx.windows().first() {
                                    let _ = window.update(cx, |_, window: &mut Window, _| {
                                        window.show_window();
                                        window.activate_window();
                                    });
                                }
                            });
                        }
                    }

                    // Handle tray commands from app logic
                    while let Ok(cmd) = tray_rx.try_recv() {
                        match cmd {
                            TrayCommand::SetStopEnabled(enabled) => {
                                stop_item.set_enabled(enabled);
                            }
                            TrayCommand::SetRecording(recording) => {
                                if let Some((base, rec, w, h)) = icon_buffers.as_ref() {
                                    let buf = if recording { rec } else { base };
                                    let _ = tray_icon.set_icon(make_icon(buf, *w, *h));
                                }
                                let _ = tray_icon.set_tooltip(Some(
                                    if recording { "Rekaptr — Recording" } else { "Rekaptr" }
                                ));
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
                                    if workspace_weak.upgrade().is_some() {
                                        if let Some(window_handle) = cx.windows().first().cloned() {
                                            let _ = window_handle.update(cx, |view: AnyView, window: &mut Window, cx: &mut App| {
                                                if let Ok(view) = view.downcast::<RekaptrWorkspace>() {
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
                    cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                }
            }
        }).detach();

        // --- Disk Space Monitoring ---
        let storage_root = crate::utils::get_storage_root();
        let workspace_handle_disk = workspace_handle.clone();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                use sysinfo::Disks;
                let mut disks = Disks::new_with_refreshed_list();
                loop {
                    cx.background_executor().timer(std::time::Duration::from_secs(10)).await;
                    disks.refresh_list();

                    let target_disk = disks.iter().find(|d| storage_root.starts_with(d.mount_point()));
                    if let Some(disk) = target_disk {
                        let free_gb = disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;

                        const LOW_DISK_WARNING_GB: f64 = 5.0;
                        if free_gb < LOW_DISK_WARNING_GB {
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
                                                    const CRITICAL_DISK_GB: f64 = 1.0;
                                                    if free_gb < CRITICAL_DISK_GB {
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
            window.on_window_should_close(cx, |window, _cx| {
                if crate::config::AppConfig::load().minimize_to_tray {
                    window.hide_window();
                    false
                } else {
                    true
                }
            });
            if let Some(device) = window.direct3d11_device() {
                use windows::Win32::Foundation::HANDLE;
                crate::engine::boost_device_gpu_priority(device as *mut std::ffi::c_void);
                // AddRef the D3D11 device so our handle outlives the GPUI window's reference.
                // No matching Release — the device lives for the entire app lifetime.
                unsafe {
                    use windows::Win32::Graphics::Direct3D11::ID3D11Device;
                    use windows::core::Interface;
                    let d3d = std::mem::ManuallyDrop::new(ID3D11Device::from_raw(device as *mut std::ffi::c_void));
                    let _prevent_drop = std::mem::ManuallyDrop::new((*d3d).clone()); // AddRef
                }
                *app_state.d3d11_device.lock() = Some(crate::video_player::SendHandle(HANDLE(device as _)));
            }
            let view = cx.new(|cx| RekaptrWorkspace::new(app_state.clone(), window, cx));
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
                let cx = cx.clone();
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
                                                            // Push-to-talk acts as a toggle until hold/release is implemented
                                                            log::debug!("[Hotkeys] Push-to-talk triggered (toggle mode)");
                                                            for track in workspace.form_audio_tracks.iter_mut() {
                                                                if track.source_type == "Mic" {
                                                                    track.enabled = !track.enabled;
                                                                    break;
                                                                }
                                                            }
                                                            cx.notify();
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
            let cx = cx.clone();
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
                            let _ = UnhookWinEvent(hook);
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
            // Try the conventional port first, then 8081..=8089, then fall back to an
            // OS-assigned ephemeral port so playback still works if all are taken.
            let mut bound: Option<tokio::net::TcpListener> = None;
            for port in 8080u16..=8089 {
                if let Ok(l) = tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                    bound = Some(l);
                    break;
                }
            }
            let listener = match bound {
                Some(l) => l,
                None => match tokio::net::TcpListener::bind("127.0.0.1:0").await {
                    Ok(l) => l,
                    Err(e) => {
                        log::error!("[Server] Failed to bind local HLS server (8080-8089 and ephemeral): {:?}", e);
                        return;
                    }
                },
            };
            let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
            HLS_PORT.store(port, std::sync::atomic::Ordering::Relaxed);
            log::info!("[Server] Local HLS server listening on http://127.0.0.1:{}", port);
            
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

                    let decoded_path = percent_decode_path(url_path);

                    // Validate auth token on all requests to prevent other
                    // local processes from reading recording data.
                    // Accept token from query param (?token=...) or HTTP header (X-Rekaptr-Token: ...).
                    let query_token = query.split('&')
                        .find_map(|p| p.strip_prefix("token="));
                    let header_token = request.lines()
                        .find(|l| l.to_lowercase().starts_with("x-rekaptr-token:"))
                        .map(|l| l.splitn(2, ':').nth(1).unwrap_or("").trim());
                    let expected = get_hls_token();
                    let token_valid = query_token.map_or(false, |t| t == expected)
                        || header_token.map_or(false, |t| t == expected);
                    if !token_valid {
                        let _ = socket.write_all(b"HTTP/1.1 403 FORBIDDEN\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await;
                        return;
                    }

                    let file_path = root.join(&decoded_path);

                    // Canonicalize and verify the resolved path stays within root
                    // to prevent path traversal (e.g. "../../etc/passwd").
                    let canonical = match tokio::fs::canonicalize(&file_path).await {
                        Ok(p) => p,
                        Err(_) => {
                            let _ = socket.write_all(b"HTTP/1.1 404 NOT FOUND\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await;
                            return;
                        }
                    };
                    let canonical_root = match tokio::fs::canonicalize(&root).await {
                        Ok(p) => p,
                        Err(_) => { return; }
                    };
                    if !canonical.starts_with(&canonical_root) {
                        let _ = socket.write_all(b"HTTP/1.1 403 FORBIDDEN\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await;
                        return;
                    }
                    let file_path = canonical;
                    
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
                                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: http://127.0.0.1:{}\r\nCache-Control: no-cache\r\n",
                                status, content_type, content_len, port
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
