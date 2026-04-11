use crate::config::{AppConfig, AudioRouting, VideoSettings};
use crate::state::AppState;
use crate::video_player::Video;
use adabraka_ui::prelude::*;
use gpui::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use gstreamer::prelude::*;
use gstreamer as gst;

mod add_source;
mod clips;
mod dashboard;
mod settings;
mod sidebar;
mod timeline;

pub struct LumaWorkspace {
    pub active_view: ActiveView,
    pub app_state: Arc<Mutex<AppState>>,
    pub video_source: Option<Video>,
    pub selected_source: Option<String>,
    pub show_add_source_modal: bool,
    pub advanced_settings_source: Option<String>,
    pub clip_start: f64,
    pub clip_end: f64,
    pub timeline_bounds: Bounds<Pixels>,
    pub is_scrubbing: bool,
    pub drag_target: Option<TimelineDragTarget>,
    pub scrubbing_progress: f32,
    pub last_seek_at: std::time::Instant,
    pub toast_manager: Entity<adabraka_ui::overlays::toast::ToastManager>,
    pub show_export_modal: bool,
    pub export_reencode: bool,
    pub export_encoder: String,
    pub export_bitrate: i32,
    pub export_preset: String,
    pub export_crf: i32,
    // Add Source Form State
    pub form_title: String,
    pub form_hwnd: Option<u64>,
    pub form_active_tab: usize,
    pub form_editing_track_index: Option<usize>,
    pub form_encoder: String,
    pub form_rate_control: i32, // 0: CQP, 1: VBR, 2: CBR
    pub form_bitrate: i32,
    pub form_cq: i32,
    pub form_retention: i32,
    pub form_resolution: String,
    pub form_fps: i32,
    pub form_gop: i32,
    pub form_bframes: i32,
    pub form_preset: String,
    pub form_zero_latency: bool,
    pub form_lookahead: bool,
    pub form_lookahead_frames: i32,
    pub form_spatial_aq: bool,
    pub form_temporal_aq: bool,
    pub form_audio_tracks: Vec<AudioRouting>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TimelineDragTarget {
    Playhead,
    InMarker,
    OutMarker,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Dashboard,
    Settings,
    Clips,
}

impl LumaWorkspace {
    pub fn new(app_state: Arc<Mutex<AppState>>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let toast_manager = cx.new(|cx| adabraka_ui::overlays::toast::ToastManager::new(cx));
        let config = AppConfig::load();

        // Populate app_state with saved games from config
        {
            let mut state = app_state.lock();
            for (title, settings) in &config.game_registry {
                state.game_registry.insert(title.clone(), settings.clone());
                let id = state.manual_sessions.len() as i32 + 100;
                state.manual_sessions.insert(
                    id,
                    crate::state::GameSession {
                        id,
                        title: title.clone(),
                        auto_record: settings.auto_record,
                        retention: settings.retention_minutes as i32,
                        bitrate: settings
                            .video_overrides
                            .as_ref()
                            .map(|v| v.bitrate_kbps)
                            .unwrap_or(10000),
                        cq: settings
                            .video_overrides
                            .as_ref()
                            .map(|v| v.cq_level)
                            .unwrap_or(23),
                    },
                );
            }
        }

        let workspace = Self {
            active_view: ActiveView::Dashboard,
            app_state,
            video_source: None,
            selected_source: None,
            show_add_source_modal: false,
            advanced_settings_source: None,
            clip_start: -1.0,
            clip_end: -1.0,
            timeline_bounds: Bounds::default(),
            is_scrubbing: false,
            drag_target: None,
            scrubbing_progress: 0.0,
            last_seek_at: std::time::Instant::now(),
            toast_manager,
            show_export_modal: false,
            export_reencode: false,
            export_encoder: "h264_nvenc".to_string(),
            export_bitrate: 50000,
            export_preset: "p4".to_string(),
            export_crf: 23,
            form_title: "New Source".to_string(),
            form_hwnd: None,
            form_active_tab: 0,
            form_editing_track_index: None,
            form_encoder: config.global_video.encoder.clone(),
            form_rate_control: config.global_video.rate_control_index,
            form_bitrate: config.global_video.bitrate_kbps,
            form_cq: config.global_video.cq_level,
            form_retention: config.global_video.retention_minutes,
            form_resolution: config.global_video.resolution.clone(),
            form_fps: config.global_video.fps,
            form_gop: config.global_video.gop_size,
            form_bframes: config.global_video.bframes,
            form_preset: config.global_video.preset.clone(),
            form_zero_latency: config.global_video.zero_latency,
            form_lookahead: config.global_video.lookahead,
            form_lookahead_frames: config.global_video.lookahead_frames,
            form_spatial_aq: config.global_video.spatial_aq,
            form_temporal_aq: config.global_video.temporal_aq,
            form_audio_tracks: config.global_audio_tracks.clone(),
        };

        // Add a high-performance refresh loop for video playback
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let this = this.clone();
            let mut cx = cx.clone();
            async move {
                loop {
                    let is_playing = this
                        .update(&mut cx, |this, _| {
                            this.video_source.as_ref().map_or(false, |v| !v.paused())
                        })
                        .unwrap_or(false);

                    if is_playing {
                        this.update(&mut cx, |_, cx| cx.notify()).ok();
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    } else {
                        // Sleep longer when paused to save CPU
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                    }
                }
            }
        })
        .detach();

        workspace
    }

    pub fn show_toast(
        &self,
        title: impl Into<SharedString>,
        description: Option<impl Into<SharedString>>,
        variant: adabraka_ui::overlays::toast::ToastVariant,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut toast = adabraka_ui::overlays::toast::ToastItem::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            title.into(),
        )
        .variant(variant);

        if let Some(desc) = description {
            toast = toast.description(desc.into());
        }

        self.toast_manager.update(cx, |manager, cx| {
            manager.add_toast(toast, window, cx);
        });
    }

    pub fn set_active_view(&mut self, view: ActiveView, cx: &mut Context<Self>) {
        self.active_view = view;
        cx.notify();
    }

    pub fn load_video(&mut self, source_name: &str, _window: &mut Window, cx: &mut Context<Self>) {
        eprintln!("[UI] load_video called for: {}", source_name);

        // Check if source_name is actually a direct file path (e.g. for clips)
        let path = std::path::Path::new(source_name);
        let is_direct_file = path.exists() && path.extension().map_or(false, |ext| ext == "mkv");

        // Check if we already have a video source and if it's the SAME source name
        let already_loaded = if let Some(v) = &self.video_source {
            let inner = v.read();
            inner.source_name == source_name
        } else {
            false
        };

        if already_loaded && !is_direct_file {
            if let Some(v) = &self.video_source {
                eprintln!(
                    "[UI] Source {} already loaded. Refreshing segments.",
                    source_name
                );
                v.refresh_segments();
                v.set_paused(false);
                cx.notify();
                return;
            }
        }

        let playlist_path = if is_direct_file {
            Some(path.to_path_buf())
        } else {
            crate::utils::generate_session_playlist(source_name)
        };

        if playlist_path.is_none() {
            eprintln!("[UI] ERROR: No segments or file found for: {}", source_name);
            self.video_source = None;
            cx.notify();
            return;
        }

        let playlist_path = playlist_path.unwrap();
        eprintln!("[UI] Loading video from: {:?}", playlist_path);

        let d3d_device_handle = self.app_state.lock().d3d11_device.lock().unwrap().0;
        
        match Video::new_with_options(
            &playlist_path.to_string_lossy(),
            crate::video_player::VideoOptions {
                source_name: Some(source_name.to_string()),
                ..Default::default()
            },
            Some(d3d_device_handle.0),
        ) {
            Ok(video) => {
                self.video_source = Some(video);
                println!("Video source loaded: {}", source_name);
            }
            Err(e) => {
                println!("Failed to load video: {:?}", e);
                self.video_source = None;
            }
        }
        cx.notify();
    }

    pub fn toggle_play_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            v.set_paused(!v.paused());
            cx.notify();
        }
    }

    pub fn set_clip_in(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_start >= 0.0 {
                self.clip_start = -1.0;
                self.clip_end = -1.0;
            } else {
                self.clip_start = v.position().as_secs_f64();
                self.clip_end = -1.0;
            }
            cx.notify();
        }
    }

    pub fn set_clip_out(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_end >= 0.0 {
                self.clip_end = -1.0;
            } else {
                self.clip_end = v.position().as_secs_f64();
            }
            cx.notify();
        }
    }

    pub fn save_clip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clip_start < 0.0 || self.clip_end < 0.0 || self.clip_end <= self.clip_start {
            self.show_toast(
                "Invalid Clip Range",
                Some("Please set both IN and OUT markers."),
                adabraka_ui::overlays::toast::ToastVariant::Error,
                window,
                cx,
            );
            return;
        }
        self.show_export_modal = true;
        cx.notify();
    }

    pub fn perform_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.clip_start;
        let end = self.clip_end;
        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        // Ensure ffconcat is up to date before exporting
        crate::utils::generate_session_playlist(&source_name);

        let safe_title = crate::utils::clean_title(&source_name);
        let storage_root = crate::utils::get_storage_root();
        let playlist_path = storage_root.join(&safe_title).join("view.ffconcat");

        if !playlist_path.exists() {
            self.show_toast(
                "Source Error",
                Some("Recording segments not found."),
                adabraka_ui::overlays::toast::ToastVariant::Error,
                window,
                cx,
            );
            return;
        }

        let clips_dir = storage_root.join("Clips").join(&safe_title);
        let _ = std::fs::create_dir_all(&clips_dir);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let output_path = clips_dir.join(format!("clip_{}_{}.mp4", safe_title, timestamp));

        let ffmpeg_path = crate::utils::get_ffmpeg_path();

        // Reset markers and close modal
        self.clip_start = -1.0;
        self.clip_end = -1.0;
        self.show_export_modal = false;
        
        let encoder = self.export_encoder.clone();
        let bitrate = self.export_bitrate;
        let preset = self.export_preset.clone();
        let export_reencode = self.export_reencode;

        cx.notify();

        let config = AppConfig::load();
        let audio_tracks = if source_name == "monitor" {
            config.global_audio_tracks.clone()
        } else {
            config
                .game_registry
                .get(&source_name)
                .and_then(|g| g.audio_routing.as_ref())
                .cloned()
                .unwrap_or(config.global_audio_tracks.clone())
        };

        let ffmpeg_task = cx.background_spawn(async move {
            use std::process::Command;

            let mut cmd = Command::new(ffmpeg_path);
            cmd.arg("-y")
               .arg("-ss").arg(format!("{:.3}", start))
               .arg("-to").arg(format!("{:.3}", end))
               .arg("-f").arg("concat")
               .arg("-safe").arg("0")
               .arg("-i").arg(playlist_path)
               .arg("-map").arg("0:v:0");

            let mut physical_stream_idx = 0;
            for track in audio_tracks {
                if track.enabled {
                    cmd.arg("-map")
                        .arg(format!("0:a:{}?", physical_stream_idx));
                }
                physical_stream_idx += 1;
            }

            if export_reencode {
                cmd.arg("-c:v")
                    .arg(&encoder)
                    .arg("-preset")
                    .arg(&preset)
                    .arg("-b:v")
                    .arg(format!("{}K", bitrate))
                    .arg("-rc")
                    .arg("vbr");
            } else {
                cmd.arg("-c:v").arg("copy");
            }

            cmd.arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("320k")
                .arg("-ar")
                .arg("48000")
                .arg("-movflags")
                .arg("+faststart")
                .arg(&output_path);

            eprintln!("[UI] Running FFmpeg: {:?}", cmd);

            (cmd.output(), output_path, clips_dir)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (result, output_path, clips_dir) = ffmpeg_task.await;

                let _ = this.update(&mut cx, |this, cx| {
                    if let Some(any_window) = cx.windows().first() {
                        let _ = any_window.update(cx, |_, window, cx| {
                            match result {
                                Ok(output) => {
                                    if output.status.success() {
                                        this.show_toast(
                                            SharedString::from("Clip Saved"),
                                            Some(SharedString::from(format!(
                                                "Exported to {:?}",
                                                output_path
                                            ))),
                                            adabraka_ui::overlays::toast::ToastVariant::Success,
                                            window,
                                            cx,
                                        );
                                        let _ = std::process::Command::new("explorer")
                                            .arg(&clips_dir)
                                            .spawn();
                                    } else {
                                        let err = String::from_utf8_lossy(&output.stderr);
                                        eprintln!("[UI] FFmpeg failed: {}", err);
                                        this.show_toast(
                                            SharedString::from("Export Failed"),
                                            Some(SharedString::from("FFmpeg returned an error.")),
                                            adabraka_ui::overlays::toast::ToastVariant::Error,
                                            window,
                                            cx,
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[UI] Failed to run FFmpeg: {}", e);
                                    this.show_toast(
                                        SharedString::from("FFmpeg Error"),
                                        Some(SharedString::from("Could not locate or run ffmpeg.exe")),
                                        adabraka_ui::overlays::toast::ToastVariant::Error,
                                        window,
                                        cx,
                                    );
                                }
                            }
                        });
                    }
                });
            }
        }).detach();
    }

    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let is_recording = self
            .app_state
            .lock()
            .is_recording
            .load(std::sync::atomic::Ordering::SeqCst);

        if is_recording {
            if let Some(pipeline) = self.app_state.lock().pipeline.take() {
                std::thread::spawn(move || {
                    pipeline.send_event(gst::event::Eos::new());
                    if let Some(bus) = pipeline.bus() {
                        let _ = bus.timed_pop_filtered(
                            gst::ClockTime::from_seconds(5),
                            &[gst::MessageType::Eos],
                        );
                    }
                    let _ = pipeline.set_state(gst::State::Null);
                });
            }
            self.app_state
                .lock()
                .is_recording
                .store(false, std::sync::atomic::Ordering::SeqCst);
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
            let config = AppConfig::load();
            let source_name = self
                .selected_source
                .clone()
                .unwrap_or_else(|| "monitor".to_string());

            let hwnd = if source_name == "monitor" {
                None
            } else {
                let state = self.app_state.lock();
                state
                    .available_windows
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

            let safe_title = crate::utils::clean_title(&source_name).replace(' ', "_");
            let s_dir = crate::utils::get_storage_root()
                .join(&safe_title)
                .join(format!(
                    "session_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                ));

            if let Err(_) = std::fs::create_dir_all(&s_dir) {
                println!("Failed to create session directory.");
                self.show_toast(
                    "Error",
                    Some("Failed to create session directory"),
                    adabraka_ui::overlays::toast::ToastVariant::Error,
                    window,
                    cx,
                );
                return;
            }

            let mut video_settings = config.global_video.clone();
            let mut audio_routing = config.global_audio_tracks.clone();

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

            let pipeline_str = crate::engine::generate_pipeline_string(
                &video_settings,
                &s_dir.to_string_lossy(),
                &audio_routing,
                &config.mic_settings,
                hwnd,
                target_pid,
                config.selected_adapter_index,
            );

            match gst::parse::launch(&pipeline_str) {
                Ok(pipeline) => {
                    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
                    
                    if let Some(sink) = pipeline.by_name("sink") {
                        let s_path = s_dir.to_string_lossy().replace('\\', "/");
                        sink.connect("format-location", false, move |args| {
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let id = args.get(1).and_then(|v| v.get::<u32>().ok()).unwrap_or(0);
                                
                                if id == 3 {
                                    // Cleanup temp files (fire and forget)
                                    let cleanup_path = s_path.clone();
                                    std::thread::spawn(move || {
                                        // Wait for file locks to release (especially for temp_3)
                                        std::thread::sleep(std::time::Duration::from_secs(2));
                                        for i in 1..=3 {
                                            let temp_file = format!("{}/temp_{}.mkv", cleanup_path, i);
                                            let _ = std::fs::remove_file(temp_file);
                                        }
                                    });
                                }

                                let fname = if id < 3 {
                                    format!("{}/temp_{}.mkv", s_path, id + 1)
                                } else {
                                    format!("{}/segment_{:05}.mkv", s_path, id - 3)
                                };
                                Some(fname.to_value())
                            })).ok().flatten()
                        });
                    }

                    let _ = pipeline.set_state(gst::State::Playing);
                    self.app_state.lock().pipeline = Some(pipeline);
                    self.app_state
                        .lock()
                        .is_recording
                        .store(true, std::sync::atomic::Ordering::SeqCst);
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

    pub fn render_export_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
        div()
            .absolute()
            .inset_0()
            .bg(theme.tokens.background.opacity(0.8))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Card::new()
                    .w(px(400.0))
                    .content(
                        VStack::new()
                            .p_6()
                            .gap_6()
                            .child(
                                VStack::new()
                                    .gap_1()
                                    .child(div().text_xl().font_weight(FontWeight::BOLD).child("Export Settings"))
                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Choose how you want to save this clip."))
                            )
                            .child(
                                VStack::new()
                                    .gap_4()
                                    .child(
                                        HStack::new()
                                            .gap_3()
                                            .items_center()
                                            .child({
                                                let view = cx.entity().downgrade();
                                                adabraka_ui::components::radio::Radio::new("instant-copy")
                                                    .checked(!self.export_reencode)
                                                    .on_click(move |_, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            this.export_reencode = false;
                                                            cx.notify();
                                                        });
                                                    })
                                            })
                                            .child(
                                                VStack::new()
                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Instant Copy (Recommended)"))
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Lossless, saves in less than a second."))
                                            )
                                    )
                                    .child(
                                        HStack::new()
                                            .gap_3()
                                            .items_center()
                                            .child({
                                                let view = cx.entity().downgrade();
                                                adabraka_ui::components::radio::Radio::new("re-encode")
                                                    .checked(self.export_reencode)
                                                    .on_click(move |_, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            this.export_reencode = true;
                                                            cx.notify();
                                                        });
                                                    })
                                            })
                                            .child(
                                                VStack::new()
                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Re-encode (Complete MP4)"))
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Choose quality and format for best compatibility."))
                                            )
                                    )
                            )
                            .when(self.export_reencode, |this| {
                                this.child(
                                    VStack::new()
                                        .gap_4()
                                        .p_4()
                                        .bg(theme.tokens.muted.opacity(0.5))
                                        .rounded_md()
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("ENCODER"))
                                                .child(
                                                    HStack::new()
                                                        .gap_2()
                                                        .child(
                                                            Button::new("exp-enc-h264", "H.264")
                                                                .variant(if self.export_encoder == "h264_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "h264_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-enc-hevc", "HEVC")
                                                                .variant(if self.export_encoder == "hevc_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "hevc_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-enc-av1", "AV1")
                                                                .variant(if self.export_encoder == "av1_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "av1_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                )
                                        )
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("QUALITY PRESET"))
                                                .child(
                                                    HStack::new()
                                                        .gap_2()
                                                        .child(
                                                            Button::new("exp-pre-fast", "Fast")
                                                                .variant(if self.export_preset == "p1" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p1".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-pre-bal", "Balanced")
                                                                .variant(if self.export_preset == "p4" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p4".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-pre-hq", "High Quality")
                                                                .variant(if self.export_preset == "p7" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p7".to_string(); cx.notify(); }))
                                                        )
                                                )
                                        )
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("BITRATE (kbps)"))
                                                .child(
                                                    HStack::new()
                                                        .gap_4()
                                                        .items_center()
                                                        .child(
                                                            Button::new("exp-bit-dec", "-")
                                                                .variant(ButtonVariant::Outline)
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate - 5000).max(1000); cx.notify(); }))
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .p_2()
                                                                .bg(theme.tokens.background)
                                                                .rounded_md()
                                                                .child(div().text_center().font_weight(FontWeight::BOLD).child(format!("{}k", self.export_bitrate)))
                                                        )
                                                        .child(
                                                            Button::new("exp-bit-inc", "+")
                                                                .variant(ButtonVariant::Outline)
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate + 5000).min(100000); cx.notify(); }))
                                                        )
                                                )
                                        )
                                )
                            })
                            .child(
                                HStack::new()
                                    .justify_end()
                                    .gap_3()
                                    .child(
                                        Button::new("cancel-export", "Cancel")
                                            .variant(ButtonVariant::Ghost)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_export_modal = false;
                                                cx.notify();
                                            }))
                                    )
                                    .child(
                                        Button::new("start-export", "Start Export")
                                            .variant(ButtonVariant::Default)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.perform_export(window, cx);
                                            }))
                                    )
                            )
                    )
            )
    }
}

impl Render for LumaWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let mut root = div()
            .size_full()
            .flex()
            .bg(theme.tokens.background)
            .text_color(theme.tokens.foreground)
            .child(self.render_sidebar(window, cx))
            .child(
                VStack::new()
                    .flex_1()
                    .child(match self.active_view {
                        ActiveView::Dashboard => self.render_dashboard(window, cx).into_any_element(),
                        ActiveView::Settings => self.render_settings_view(window, cx).into_any_element(),
                        ActiveView::Clips => self.render_clips(window, cx).into_any_element(),
                    })
            );

        if self.show_add_source_modal {
            root = root.child(self.render_add_source_modal(window, cx));
        }

        if let Some(source) = self.advanced_settings_source.clone() {
            root = root.child(self.render_advanced_settings_dialog(&source, window, cx));
        }

        if self.show_export_modal {
            root = root.child(self.render_export_modal(window, cx));
        }

        root.child(self.toast_manager.clone())
    }
}
