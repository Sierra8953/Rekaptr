//! Recording start/stop logic using GStreamer.
//!
//! Luma records via a GStreamer pipeline that captures a D3D11 window texture (or monitor)
//! and muxes it into fragmented MP4 segments using `splitmuxsink`. Each segment is a self-
//! contained `.m4s` file, enabling instant clip export without re-encoding.
//!
//! ## Stop flow
//! Stopping a recording is intentionally done on a background thread:
//! 1. Send EOS event to the pipeline (flushes all buffered data)
//! 2. Wait up to 5s for the EOS message on the bus (ensures muxer finalizes)
//! 3. Set pipeline to Null state (releases all resources)
//! 4. Run `fixup_eos_segments` — the final segment from splitmuxsink may not get a
//!    `fragment-closed` signal, so it won't have been renamed with its duration. The fixup
//!    pass detects these unrenamed segments and corrects their filenames.
//!
//! ## D3D11 device sharing
//! The GStreamer pipeline's D3D11 elements need GPU access. Rather than creating a second
//! D3D11 device (which wastes VRAM and can cause driver issues), we share the app's existing
//! device handle via GStreamer's context mechanism (`gst.d3d11.device.handle`). This lets
//! GStreamer's `d3d11screencapturesrc` use the same GPU device as GPUI's renderer.
//!
//! ## Audio capture
//! Audio tracks are fed into the pipeline via GStreamer `appsrc` elements:
//! - **System loopback**: Captured by GStreamer's `wasapi2src` directly in the pipeline
//! - **Microphone**: A shared `MicProvider` pushes PCM buffers to `appsrc` subscribers,
//!   allowing multiple pipelines to share one mic capture session
//! - **App-specific**: WASAPI process-specific capture targets a single application's audio
//!   by PID, fed into `appsrc` on a dedicated thread

use crate::config::AppConfig;
use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;
use gstreamer as gst;
use gstreamer::prelude::*;

impl LumaWorkspace {
    /// Emergency stop: immediately kills the pipeline without graceful EOS shutdown.
    /// Used when we need to force-stop (e.g., pipeline error recovery).
    pub fn toggle_recording_internal(&mut self) {
        if let Some(pipeline) = self.app_state.pipeline.lock().take() {
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.app_state.is_recording.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_recording_ext(None, window, cx);
    }

    /// Main recording toggle with full pipeline lifecycle management.
    ///
    /// When starting, this method:
    /// 1. Resolves video/audio settings (global defaults, overridden per-game)
    /// 2. Computes the timestamp offset so new segments continue from where the last
    ///    recording left off (enables seamless multi-session timelines)
    /// 3. Builds and launches the GStreamer pipeline
    /// 4. Shares the D3D11 device context with the pipeline
    /// 5. Wires up audio appsrc feeders for each configured track
    /// 6. Spawns a bus monitor that handles fragment renaming and error reporting
    ///
    /// The bus handler listens for `splitmuxsink-fragment-closed` events and renames each
    /// segment file to include its duration in milliseconds (e.g., `seg_001_5000ms.m4s`).
    /// This duration-in-filename convention lets the HLS playlist generator and export logic
    /// know each segment's length without having to probe the file.
    pub fn toggle_recording_ext(&mut self, explicit_hwnd: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        let is_recording = self
            .app_state
            .is_recording
            .load(std::sync::atomic::Ordering::SeqCst);

        if is_recording {
            let game_dir_for_fixup = crate::utils::get_storage_root()
                .join(crate::utils::clean_title(
                    &self.selected_source.clone().unwrap_or_else(|| "monitor".to_string()),
                ));
            if let Some(pipeline) = self.app_state.pipeline.lock().take() {
                // Mark pipeline as stopping so a restart attempt waits for teardown
                self.app_state.pipeline_stopping.store(true, std::sync::atomic::Ordering::SeqCst);
                let stopping_flag = self.app_state.pipeline_stopping.clone();

                std::thread::spawn(move || {
                    // catch_unwind so a GStreamer panic doesn't crash the whole app
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pipeline.send_event(gst::event::Eos::new());
                        if let Some(bus) = pipeline.bus() {
                            let _ = bus.timed_pop_filtered(
                                gst::ClockTime::from_seconds(5),
                                &[gst::MessageType::Eos],
                            );
                        }
                        let _ = pipeline.set_state(gst::State::Null);
                        // Give muxer a moment to release file handles
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        // Fix EOS segments: rename unrenamed + correct wrong durations
                        crate::segment_verify::fixup_eos_segments(&game_dir_for_fixup);
                    }));

                    if result.is_err() {
                        log::error!("[Recording] Pipeline teardown panicked — forcing Null state");
                        // Pipeline was moved into the closure, so it's dropped here regardless
                    }

                    // Clear the stopping guard so a new recording can start
                    stopping_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                });
            }
            self.app_state
                .is_recording
                .store(false, std::sync::atomic::Ordering::SeqCst);
            *self.app_state.recording_source.lock() = None;
            crate::tray::update_recording_state(false);
            println!("Recording stopped.");
            self.show_toast(
                "Recording Stopped",
                None::<&str>,
                adabraka_ui::overlays::toast::ToastVariant::Default,
                window,
                cx,
            );
            cx.notify();
        } else {
            // Block start while previous pipeline is still tearing down
            if self.app_state.pipeline_stopping.load(std::sync::atomic::Ordering::SeqCst) {
                self.show_toast(
                    "Please Wait",
                    Some("Previous recording is still stopping"),
                    adabraka_ui::overlays::toast::ToastVariant::Default,
                    window,
                    cx,
                );
                return;
            }

            let config = AppConfig::load();
            let source_name = self
                .selected_source
                .clone()
                .unwrap_or_else(|| "monitor".to_string());

            let mut video_settings = config.global_video.clone();
            let mut audio_routing = config.global_audio_tracks.clone();

            let hwnd = if let Some(h) = explicit_hwnd {
                Some(h)
            } else if source_name == "monitor" {
                None
            } else {
                let windows = self.app_state.available_windows.lock();
                windows
                    .iter()
                    .find(|w| w.title == source_name)
                    .map(|w| w.hwnd)
            };

            let target_pid = if let Some(h) = hwnd {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
                    let mut pid = 0;
                    let _ = GetWindowThreadProcessId(
                        windows::Win32::Foundation::HWND(h as *mut core::ffi::c_void),
                        Some(&mut pid),
                    );
                    if pid > 0 {
                        Some(pid)
                    } else {
                        None
                    }
                }
            } else {
                None
            };

            if source_name != "monitor" {
                if let Some(gs) = config.game_registry.get(&source_name) {
                    if let Some(vs) = &gs.video_overrides {
                        video_settings = vs.clone();
                    }
                    if let Some(ar) = &gs.audio_routing {
                        audio_routing = ar.clone();
                    }
                }
            }

            // Validate encoder support before launching
            if let Err(e) = crate::engine::validate_encoder(&video_settings) {
                self.show_toast(
                    "Encoder Error",
                    Some(&format!("{}", e)),
                    adabraka_ui::overlays::toast::ToastVariant::Error,
                    window,
                    cx,
                );
                return;
            }

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Unified Folder: Just use the game directory root
            let game_dir = crate::utils::get_storage_root()
                .join(crate::utils::clean_title(&source_name));
            let _ = std::fs::create_dir_all(&game_dir);

            // Normalize path for GStreamer
            let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");

            // Calculate the current total duration across all existing segments
            let ts_offset_ns = if let Some((_, blocks)) = crate::utils::generate_session_playlist(&source_name, None) {
                let total_secs: f64 = blocks.iter().map(|b| b.duration_secs).sum();
                (total_secs * 1_000_000_000.0) as i64
            } else {
                0
            };

            // Reset duration for the new recording
            *self.app_state.current_recording_duration.lock() = ts_offset_ns as f64 / 1_000_000_000.0;

            let pipeline_str = crate::engine::generate_pipeline_string(
                &video_settings,
                &game_dir_str,
                &audio_routing,
                &config.mic_settings,
                hwnd,
                target_pid,
                config.selected_adapter_index,
                timestamp,
                ts_offset_ns,
            );

            match gst::parse::launch(&pipeline_str) {
                Ok(pipeline) => {
                    let pipeline = match pipeline.dynamic_cast::<gst::Pipeline>() {
                        Ok(p) => p,
                        Err(_) => {
                            log::error!("[Recording] Failed to cast GStreamer element to Pipeline");
                            self.show_toast("Pipeline Error", Some("Internal pipeline cast failed"), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                            return;
                        }
                    };

                    // Share D3D11 device handle
                    if let Some(handle) = self.app_state.d3d11_device.lock().as_ref() {
                        let device_ptr = handle.0.0 as u64;
                        let mut context = gst::Context::new("gst.d3d11.device.handle", true);
                        if let Some(c) = context.get_mut() {
                            c.structure_mut().set("device-handle", &device_ptr);
                        }
                        pipeline.set_context(&context);
                    }

                    // Setup Audio Feeders
                    let audio_routing_for_setup = audio_routing.clone();
                    for (i, track) in audio_routing_for_setup.iter().enumerate() {
                        if !track.enabled { continue; }

                        if track.source_type == "Mic" {
                            if let Some(src) = pipeline.by_name(&format!("mic_src_{}", i)) {
                                if let Ok(appsrc) = src.dynamic_cast::<gstreamer_app::AppSrc>() {
                                    if let Some(provider) = self.app_state.mic_provider.lock().as_ref() {
                                        provider.subscribers.insert(i as u64, appsrc);
                                    }
                                }
                            }
                        } else if track.source_type == "App" {
                            if let Some(src) = pipeline.by_name(&format!("audio_app_{}", i)) {
                                if let Ok(appsrc) = src.dynamic_cast::<gstreamer_app::AppSrc>() {
                                    if !track.app_targets.is_empty() {
                                        for app_name in &track.app_targets {
                                            let appsrc_clone = appsrc.clone();
                                            let app_name_clone = app_name.clone();
                                            std::thread::spawn(move || {
                                                log::info!("[UI] Starting capture for target '{}' on track {}", app_name_clone, i);
                                                if let Err(e) = crate::engine::start_app_capture(app_name_clone, None, appsrc_clone) {
                                                    log::error!("App capture error for track {}: {:?}", i, e);
                                                }
                                            });
                                        }
                                    } else {
                                        let app_name = if track.device_name.is_empty() || track.device_name == "Default" {
                                            source_name.clone()
                                        } else {
                                            track.device_name.clone()
                                        };

                                        std::thread::spawn(move || {
                                            log::info!("[UI] Starting fallback capture for '{}' on track {}", app_name, i);
                                            if let Err(e) = crate::engine::start_app_capture(app_name, target_pid, appsrc) {
                                                log::error!("App capture error: {:?}", e);
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let _ = pipeline.set_state(gst::State::Playing);

                    // Monitor the bus for errors
                    if let Some(bus) = pipeline.bus() {
                        let view_handle = cx.entity().downgrade();
                        let app_state_bus = self.app_state.clone();
                        let game_dir_bus = game_dir.clone();
                        let recording_id_bus = timestamp;

                        cx.spawn(move |_, cx: &mut AsyncApp| {
                            let mut cx = cx.clone();
                            async move {
                                let mut messages = bus.stream();
                                use futures_util::StreamExt;

                                let mut current_fragment_start_time: Option<u64> = None;

                                while let Some(msg) = messages.next().await {
                                    let msg_view = msg.view();
                                    match msg_view {
                                        gst::MessageView::Element(element_msg) => {
                                            if let Some(structure) = element_msg.structure() {
                                                let name = structure.name();

                                                if name == "splitmuxsink-fragment-opened" {
                                                    let running_time: u64 = structure.get("running-time").unwrap_or(0);
                                                    current_fragment_start_time = Some(running_time);
                                                } else if name == "splitmuxsink-fragment-closed" {
                                                    let location: String = structure.get("location").unwrap_or_default();
                                                    let running_time: u64 = structure.get("running-time").unwrap_or(0);

                                                    if let Some(start_time) = current_fragment_start_time {
                                                        let duration_ns = running_time.saturating_sub(start_time);
                                                        let duration_ms = duration_ns / 1_000_000;
                                                        let duration_secs = duration_ns as f64 / 1_000_000_000.0;

                                                        {
                                                            let mut total_dur = app_state_bus.current_recording_duration.lock();
                                                            *total_dur += duration_secs;

                                                            if let Ok(db) = crate::db::GameDatabase::open(&game_dir_bus) {
                                                                let _ = db.update_duration(recording_id_bus, *total_dur);
                                                            }
                                                        }

                                                        let old_path = std::path::PathBuf::from(&location);
                                                        let mut new_path = old_path.clone();
                                                        let file_stem = match old_path.file_stem() {
                                                            Some(s) => s.to_string_lossy().into_owned(),
                                                            None => continue,
                                                        };

                                                        if !file_stem.contains("ms") {
                                                            new_path.set_file_name(format!("{}_{}ms.m4s", file_stem, duration_ms));

                                                            cx.spawn(|cx: &mut AsyncApp| {
                                                                let mut cx = cx.clone();
                                                                async move {
                                                                    for _ in 0..25 {
                                                                        cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                                                                        if std::fs::rename(&old_path, &new_path).is_ok() {
                                                                            return;
                                                                        }
                                                                    }
                                                                }
                                                            }).detach();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        gst::MessageView::Eos(_) => {
                                            log::info!("[GStreamer] EOS Received. Finalizing recording.");
                                            break;
                                        }
                                        gst::MessageView::Error(err) => {
                                            let error_msg = format!("{}", err.error());
                                            let debug_msg = format!("{:?}", err.debug());
                                            log::error!("[GStreamer] Pipeline Error: {} ({:?})", error_msg, err.debug());

                                            // Detect GPU device-lost / driver crash
                                            let is_device_lost = error_msg.contains("device removed")
                                                || error_msg.contains("device lost")
                                                || error_msg.contains("DXGI_ERROR_DEVICE_REMOVED")
                                                || error_msg.contains("DXGI_ERROR_DEVICE_RESET")
                                                || debug_msg.contains("DXGI_ERROR_DEVICE_REMOVED")
                                                || debug_msg.contains("device removed");

                                            let toast_msg = if is_device_lost {
                                                "GPU device lost — recording stopped. This usually means a driver crash or GPU timeout.".to_string()
                                            } else {
                                                error_msg
                                            };

                                            let _ = view_handle.update(&mut cx, |this, cx| {
                                                if let Some(any_window) = cx.windows().first() {
                                                    let _ = any_window.update(cx, |_, window, cx| {
                                                        this.show_toast("Recording Error", Some(toast_msg), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                                                    });
                                                }
                                                this.toggle_recording_internal();
                                            });
                                            break;
                                        }
                                        gst::MessageView::Warning(warn) => {
                                            log::warn!("[GStreamer] Pipeline Warning: {} ({:?})", warn.error(), warn.debug());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }).detach();
                    }

                    *self.app_state.pipeline.lock() = Some(pipeline);
                    self.app_state
                        .is_recording
                        .store(true, std::sync::atomic::Ordering::SeqCst);

                    self.recording_start_time = Some(std::time::Instant::now());
                    self.recording_session_id = Some(timestamp);
                    *self.app_state.recording_source.lock() = Some(source_name.clone());
                    crate::tray::update_recording_state(true);

                    // Register session in DB
                    if let Ok(db) = crate::db::GameDatabase::open(&game_dir) {
                        let _ = db.register_session(timestamp);
                    }

                    println!("Recording started for: {}", source_name);
                    self.show_toast(
                        "Recording Started",
                        Some(&format!("Capturing {}", source_name)),
                        adabraka_ui::overlays::toast::ToastVariant::Success,
                        window,
                        cx,
                    );
                    cx.notify();
                }
                Err(e) => {
                    println!("Failed to launch pipeline: {:?}", e);
                    self.show_toast(
                        "Pipeline Error",
                        Some("GStreamer failed to start"),
                        adabraka_ui::overlays::toast::ToastVariant::Error,
                        window,
                        cx,
                    );
                }
            }
        }
    }
}
