use crate::config::AppConfig;
use crate::ui::RekaptrWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::Arc;

impl RekaptrWorkspace {
    pub fn toggle_recording_internal(&mut self) {
        let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
        let game_dir = crate::utils::get_storage_root().join(crate::utils::clean_title(&source));

        if let Some(pipeline) = self.app_state.recording.pipeline.lock().take() {
            let _ = pipeline.set_state(gst::State::Null);

            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                crate::utils::fixup_eos_segments(&game_dir);
            });
        }
        *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
    }

    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_recording_ext(None, window, cx);
    }

    pub fn toggle_recording_ext(&mut self, explicit_hwnd: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        let phase = *self.app_state.recording.phase.lock();
        let is_recording = phase.is_recording();

        if is_recording {
            self.stop_recording(window, cx);
        } else {
            if !phase.is_idle() { return; }
            self.start_recording(explicit_hwnd, window, cx);
        }
    }

    fn stop_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let game_dir_for_fixup = crate::utils::get_storage_root()
            .join(crate::utils::clean_title(
                &self.selected_source.clone().unwrap_or_else(|| "monitor".to_string()),
            ));
        *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Stopping;
        if let Some(pipeline) = self.app_state.recording.pipeline.lock().take() {
            let phase_handle = Arc::clone(&self.app_state.recording.phase);
            std::thread::spawn(move || {
                pipeline.send_event(gst::event::Eos::new());
                if let Some(bus) = pipeline.bus() {
                    let _ = bus.timed_pop_filtered(
                        gst::ClockTime::from_seconds(5),
                        &[gst::MessageType::Eos],
                    );
                }
                let _ = pipeline.set_state(gst::State::Null);
                std::thread::sleep(std::time::Duration::from_millis(200));
                crate::utils::fixup_eos_segments(&game_dir_for_fixup);
                *phase_handle.lock() = crate::state::RecordingPhase::Idle;
            });
        } else {
            *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
        }
        log::info!("[Recording] Stopped.");
        if let Some(tx) = self.app_state.tray_tx.lock().as_ref() {
            let _ = tx.send(crate::state::TrayCommand::SetStopEnabled(false));
        }
        self.show_toast(
            "Recording Stopped",
            None::<&str>,
            adabraka_ui::overlays::toast::ToastVariant::Default,
            window,
            cx,
        );
        cx.notify();
    }

    fn start_recording(&mut self, explicit_hwnd: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Starting;

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
                if pid > 0 { Some(pid) } else { None }
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

        if let Err(e) = crate::engine::validate_encoder(&video_settings) {
            *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
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

        let game_dir = crate::utils::get_storage_root()
            .join(crate::utils::clean_title(&source_name));
        let _ = std::fs::create_dir_all(&game_dir);

        let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");

        let total_existing_duration = crate::utils::compute_total_duration(&game_dir);
        let ts_offset_ns = (total_existing_duration * 1_000_000_000.0) as i64;

        *self.app_state.recording.current_recording_duration.lock() = ts_offset_ns as f64 / 1_000_000_000.0;

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
                        *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
                        self.show_toast("Pipeline Error", Some("Failed to initialize recording pipeline."), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                        return;
                    }
                };

                // Share D3D11 device with GStreamer for GPU-accelerated encoding.
                if let Some(handle) = self.app_state.d3d11_device.lock().as_ref() {
                    let device_ptr = handle.0.0 as u64;
                    let mut context = gst::Context::new("gst.d3d11.device.handle", true);
                    if let Some(c) = context.get_mut() {
                        c.structure_mut().set("device-handle", &device_ptr);
                    }
                    pipeline.set_context(&context);
                }

                self.setup_audio_feeders(&pipeline, &audio_routing, &source_name, target_pid);

                let _ = pipeline.set_state(gst::State::Playing);

                if let Some(bus) = pipeline.bus() {
                    self.spawn_bus_monitor(bus, &game_dir, timestamp, cx);
                }

                *self.app_state.recording.pipeline.lock() = Some(pipeline);
                *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Recording;

                self.app_state.recording.reset_stats();
                self.recording_start_time = Some(std::time::Instant::now());
                self.recording_session_id = Some(timestamp);

                if let Ok(db) = crate::db::GameDatabase::open(&game_dir) {
                    let _ = db.register_session(timestamp);
                }

                log::info!("[Recording] Started for: {}", source_name);
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
                *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
                log::error!("[Recording] Failed to launch pipeline: {:?}", e);
                crate::engine::diagnose_pipeline_failure(&pipeline_str, &e);
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

    fn setup_audio_feeders(
        &self,
        pipeline: &gst::Pipeline,
        audio_routing: &[crate::config::AudioRouting],
        source_name: &str,
        target_pid: Option<u32>,
    ) {
        for (i, track) in audio_routing.iter().enumerate() {
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
                                    log::info!("[Recording] Starting capture for target '{}' on track {}", app_name_clone, i);
                                    if let Err(e) = crate::engine::start_app_capture(app_name_clone, None, appsrc_clone) {
                                        log::error!("App capture error for track {}: {:?}", i, e);
                                    }
                                });
                            }
                        } else {
                            let app_name = if track.device_name.is_empty() || track.device_name == "Default" {
                                source_name.to_string()
                            } else {
                                track.device_name.clone()
                            };

                            std::thread::spawn(move || {
                                log::info!("[Recording] Starting fallback capture for '{}' on track {}", app_name, i);
                                if let Err(e) = crate::engine::start_app_capture(app_name, target_pid, appsrc) {
                                    log::error!("App capture error: {:?}", e);
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    fn spawn_bus_monitor(
        &self,
        bus: gst::Bus,
        game_dir: &std::path::Path,
        recording_id: u64,
        cx: &mut Context<Self>,
    ) {
        let view_handle = cx.entity().downgrade();
        let app_state_bus = self.app_state.clone();
        let game_dir_bus = game_dir.to_path_buf();

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
                                    Self::handle_fragment_closed(
                                        &app_state_bus,
                                        &game_dir_bus,
                                        recording_id,
                                        structure,
                                        current_fragment_start_time,
                                        &mut cx,
                                    ).await;
                                }
                            }
                        }
                        gst::MessageView::Eos(_) => {
                            log::info!("[GStreamer] EOS Received. Finalizing recording.");
                            break;
                        }
                        gst::MessageView::Error(err) => {
                            log::error!("[GStreamer] Pipeline Error: {} ({:?})", err.error(), err.debug());
                            let _ = view_handle.update(&mut cx, |this, cx| {
                                if let Some(any_window) = cx.windows().first() {
                                    let _ = any_window.update(cx, |_, window, cx| {
                                        this.show_toast("Recording Error", Some("The GStreamer pipeline crashed."), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                                    });
                                }
                                this.toggle_recording_internal();
                            });
                            break;
                        }
                        gst::MessageView::Warning(warn) => {
                            log::warn!("[GStreamer] Pipeline Warning: {} ({:?})", warn.error(), warn.debug());
                        }
                        gst::MessageView::Qos(qos) => {
                            if let Some(structure) = qos.message().structure() {
                                if let Ok(dropped) = structure.get::<u64>("dropped") {
                                    if dropped > 0 {
                                        app_state_bus.recording.rec_stats.dropped_frames.store(
                                            dropped,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }).detach();
    }

    async fn handle_fragment_closed(
        app_state: &crate::state::AppState,
        game_dir: &std::path::Path,
        recording_id: u64,
        structure: &gst::StructureRef,
        fragment_start_time: Option<u64>,
        cx: &mut AsyncApp,
    ) {
        let location: String = structure.get("location").unwrap_or_default();
        let running_time: u64 = structure.get("running-time").unwrap_or(0);

        let Some(start_time) = fragment_start_time else { return };
        let duration_ns = running_time.saturating_sub(start_time);
        let duration_ms = duration_ns / 1_000_000;
        let duration_secs = duration_ns as f64 / 1_000_000_000.0;

        {
            let mut total_dur = app_state.recording.current_recording_duration.lock();
            *total_dur += duration_secs;

            if let Ok(db) = crate::db::GameDatabase::open(game_dir) {
                let _ = db.update_duration(recording_id, *total_dur);
            }
        }

        {
            use std::sync::atomic::Ordering::Relaxed;
            let stats = &app_state.recording.rec_stats;
            stats.segments_written.fetch_add(1, Relaxed);

            let seg_path = std::path::Path::new(&location);
            let file_size = seg_path.metadata().map(|m| m.len()).unwrap_or(0);
            stats.last_segment_bytes.store(file_size, Relaxed);

            if duration_secs > 0.0 {
                *stats.bitrate_kbps.lock() = (file_size as f64 * 8.0) / duration_secs / 1000.0;
                *stats.disk_write_mbps.lock() = file_size as f64 / duration_secs / (1024.0 * 1024.0);
            }
        }

        let old_path = std::path::PathBuf::from(&location);
        let file_stem = match old_path.file_stem() {
            Some(s) => s.to_string_lossy().into_owned(),
            None => return,
        };

        if !file_stem.contains("ms") {
            let mut new_path = old_path.clone();
            new_path.set_file_name(format!("{}_{}ms.m4s", file_stem, duration_ms));

            cx.spawn(|cx: &mut AsyncApp| {
                let cx = cx.clone();
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
