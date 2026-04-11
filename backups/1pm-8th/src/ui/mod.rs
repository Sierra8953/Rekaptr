use crate::config::{AppConfig, AudioRouting};
use crate::state::AppState;
use crate::video_player::Video;
use adabraka_ui::prelude::*;
use gpui::*;
use std::sync::Arc;
use gstreamer::prelude::*;
use gstreamer as gst;
use std::path::PathBuf;

mod add_source;
mod clips;
mod dashboard;
mod settings;
mod sidebar;
mod timeline;

use adabraka_ui::overlays::alert_dialog::AlertDialog;
use adabraka_ui::overlays::popover_menu::{PopoverMenu, PopoverMenuItem};
use adabraka_ui::display::data_table::{DataTable, ColumnDef};

pub struct LumaWorkspace {
    pub active_view: ActiveView,
    pub clips_view_mode: ClipsViewMode,
    pub settings_tab_index: usize,
    pub app_state: Arc<AppState>,
    pub video_source: Option<Video>,
    pub selected_source: Option<String>,
    pub show_add_source_modal: bool,
    pub advanced_settings_source: Option<String>,
    pub session_to_delete: Option<i32>,
    pub clip_to_delete: Option<crate::state::Clip>,
    pub clip_popover: Option<(Point<Pixels>, crate::state::Clip)>,
    pub clip_table: Entity<DataTable<crate::state::Clip>>,
    pub clips_scroll_handle: ScrollHandle,
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
    pub audio_track_volume_popover: Option<usize>,
    pub last_audio_track_volume_popover: Option<usize>,
    pub volume_slider_last_value: f32,
    pub playback_volumes: Vec<f64>,
    pub popover_fixed_top: f32,
    pub last_notified_position: f64,
    pub is_refreshing_windows: bool,
    pub is_loading_video: bool,
    pub is_adjusting_volume: bool,
    pub storage_clips_mb: u64,
    pub storage_sessions_mb: u64,
    pub is_calculating_storage: bool,
    pub form_max_buffer_size_gb: i32,
    pub clips_search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub form_custom_process_exe: Entity<adabraka_ui::components::input_state::InputState>,
    pub form_custom_process_title: Entity<adabraka_ui::components::input_state::InputState>,
    pub volume_slider_state: Entity<adabraka_ui::components::slider::SliderState>,
    pub recording_start_time: Option<std::time::Instant>,
    pub recording_session_id: Option<u64>,
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ClipsViewMode {
    Grid,
    Table,
}

impl LumaWorkspace {
    pub fn new(app_state: Arc<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let toast_manager = cx.new(|cx| adabraka_ui::overlays::toast::ToastManager::new(cx));
        let config = AppConfig::load();

        // Initialize empty DataTable for clips
        let clip_table = cx.new(|cx| {
            DataTable::new(Vec::new(), Self::create_clip_columns(), cx)
        });

        // Populate app_state with saved games from config
        if app_state.game_registry.is_empty() {
            for (title, settings) in &config.game_registry {
                app_state.game_registry.insert(title.clone(), settings.clone());
                let id = app_state.manual_sessions.len() as i32 + 100;
                app_state.manual_sessions.insert(
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
            clips_view_mode: ClipsViewMode::Grid,
            settings_tab_index: 0,
            app_state,
            video_source: None,
            selected_source: None,
            show_add_source_modal: false,
            advanced_settings_source: None,
            session_to_delete: None,
            clip_to_delete: None,
            clip_popover: None,
            clip_table,
            clips_scroll_handle: ScrollHandle::new(),
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
            audio_track_volume_popover: None,
            last_audio_track_volume_popover: None,
            volume_slider_last_value: 100.0,
            playback_volumes: vec![100.0; 10], // Support up to 10 tracks
            popover_fixed_top: 0.0,
            last_notified_position: 0.0,
            is_refreshing_windows: false,
            is_loading_video: false,
            is_adjusting_volume: false,
            storage_clips_mb: 0,
            storage_sessions_mb: 0,
            is_calculating_storage: false,
            form_max_buffer_size_gb: config.max_buffer_size_gb,
            clips_search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            form_custom_process_exe: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            form_custom_process_title: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            volume_slider_state: cx.new(|cx| adabraka_ui::components::slider::SliderState::new(cx)),
            recording_start_time: None,
            recording_session_id: None,
        };

        // Add a high-performance refresh loop for video playback
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let this = this.clone();
            let mut cx = cx.clone();
            async move {
                loop {
                    let mut should_notify = false;
                    let _ = this.update(&mut cx, |this, _| {
                        if let Some(v) = &this.video_source {
                            if !v.paused() || this.is_scrubbing {
                                let pos = v.position().as_secs_f64();
                                if (pos - this.last_notified_position).abs() > 0.001 || this.is_scrubbing {
                                    this.last_notified_position = pos;
                                    should_notify = true;
                                }
                            }
                        }
                    });

                    if should_notify {
                        this.update(&mut cx, |_, cx| cx.notify()).ok();
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    } else {
                        // Sleep longer when paused or no change to save CPU
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

    pub fn update_mpv_audio_mix(&self) {
        if let Some(v) = &self.video_source {
            let active_tracks = self.get_current_audio_tracks();
            let mut enabled_aids = Vec::new();
            for (i, t) in active_tracks.iter().enumerate() {
                if t.enabled {
                    enabled_aids.push(i);
                }
            }

            if enabled_aids.is_empty() {
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", "");
            } else if enabled_aids.len() == 1 {
                let idx = enabled_aids[0];
                let vol = self.playback_volumes.get(idx).copied().unwrap_or(100.0) / 100.0;
                let complex = format!("[aid{}]volume=volume={}[ao]", idx + 1, vol);
                
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
            } else {
                let mut complex = String::new();
                for &idx in &enabled_aids {
                    let vol = self.playback_volumes.get(idx).copied().unwrap_or(100.0) / 100.0;
                    complex.push_str(&format!("[aid{}]volume=volume={}[a{}];", idx + 1, vol, idx + 1));
                }
                for &idx in &enabled_aids {
                    complex.push_str(&format!("[a{}]", idx + 1));
                }
                // Normalize=0 prevents volume dropping when mixing multiple tracks
                complex.push_str(&format!("amix=inputs={}:normalize=0[ao]", enabled_aids.len()));
                
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
            }
        }
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
        
        if view == ActiveView::Settings {
            // Refresh max buffer size from config just in case
            let config = AppConfig::load();
            self.form_max_buffer_size_gb = config.max_buffer_size_gb;
            
            if !self.is_calculating_storage {
                self.is_calculating_storage = true;
                let task = cx.background_spawn(async move {
                    let root = crate::utils::get_storage_root();
                    let clips_dir = root.join("Clips");
                    
                    let clips_size = crate::utils::get_dir_size(&clips_dir).unwrap_or(0);
                    let mut sessions_size = 0;
                    
                    if let Ok(entries) = std::fs::read_dir(&root) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let path = entry.path();
                            if path.is_dir() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if name != "Clips" && name != "Cache" && !name.starts_with(".") {
                                    sessions_size += crate::utils::get_dir_size(&path).unwrap_or(0);
                                }
                            }
                        }
                    }
                    
                    (clips_size, sessions_size)
                });
                
                cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        let (clips_bytes, sessions_bytes) = task.await;
                        let _ = this.update(&mut cx, |this, cx| {
                            this.storage_clips_mb = clips_bytes / (1024 * 1024);
                            this.storage_sessions_mb = sessions_bytes / (1024 * 1024);
                            this.is_calculating_storage = false;
                            cx.notify();
                        });
                    }
                }).detach();
            }
        }
        
        cx.notify();
    }

    pub fn get_current_audio_tracks(&self) -> Vec<AudioRouting> {
        let config = AppConfig::load();
        if let Some(source) = &self.selected_source {
            if source == "monitor" {
                return config.global_audio_tracks.clone();
            }
            if let Some(game) = config.game_registry.get(source) {
                if let Some(audio) = &game.audio_routing {
                    return audio.clone();
                }
            }
        }
        config.global_audio_tracks.clone()
    }

    pub fn refresh_available_windows(&mut self, cx: &mut Context<Self>) {
        if self.is_refreshing_windows {
            return;
        }

        self.is_refreshing_windows = true;
        
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let windows = cx.background_executor().spawn(async move {
                    let mut detector = crate::game_detector::GameDetector::new();
                    detector.enumerate_windows()
                }).await;

                let _ = this.update(&mut cx, |this, cx: &mut Context<Self>| {
                    this.is_refreshing_windows = false;
                    *this.app_state.available_windows.lock() = windows;
                    cx.notify();
                });
            }
        }).detach();
        
        cx.notify();
    }

    pub fn load_video(&mut self, source_name: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let path = std::path::Path::new(source_name);
        let is_direct_file = path.exists() && path.extension().map_or(false, |ext| ext == "mkv" || ext == "mp4");

        let already_loaded = if let Some(v) = &self.video_source {
            v.read().source_name == source_name
        } else { false };

        if already_loaded && !is_direct_file {
            let source_name_str = source_name.to_string();
            let recording_id = self.recording_session_id;
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                let name_for_bg = source_name_str.clone();
                async move {
                    if let Some((_, blocks)) = cx.background_executor().spawn(async move {
                        crate::utils::generate_session_playlist(&name_for_bg, recording_id)
                    }).await {
                        let _ = this.update(&mut cx, |this, cx| {
                            *this.app_state.current_session_blocks.lock() = blocks;
                            if let Some(v) = &this.video_source {
                                let safe_title = if source_name_str == "monitor" { "monitor".to_string() } else { crate::utils::clean_title(&source_name_str) };
                                let url = format!("http://127.0.0.1:8080/{}/master.m3u8", safe_title);
                                let _ = v.load_file(&url);
                            }
                            cx.notify();
                        });
                    }
                }
            }).detach();
            return;
        }

        if self.is_loading_video { return; }
        self.is_loading_video = true;
        let source_name_str = source_name.to_string();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let recording_id = this.read_with(&cx, |this, _| this.recording_session_id).ok().flatten();
                let (video_url, blocks) = if is_direct_file {
                    (Some(source_name_str.clone()), Vec::new())
                } else {
                    let name_clone = source_name_str.clone();
                    if let Some((_, b)) = cx.background_executor().spawn(async move {
                        crate::utils::generate_session_playlist(&name_clone, recording_id)
                    }).await {
                        let safe_title = if source_name_str == "monitor" { "monitor".to_string() } else { crate::utils::clean_title(&source_name_str) };
                        // Use the local server for the master playlist
                        let url = format!("http://127.0.0.1:8080/{}/master.m3u8", safe_title);
                        (Some(url), b)
                    } else {
                        (None, Vec::new())
                    }
                };

                let _ = this.update(&mut cx, |this, cx| {
                    this.is_loading_video = false;
                    *this.app_state.current_session_blocks.lock() = blocks;

                    if let Some(url) = video_url {
                        let d3d_device_handle = this.app_state.d3d11_device.lock().unwrap().0;
                        match crate::video_player::Video::new_with_options(
                            &url,
                            crate::video_player::VideoOptions { source_name: Some(source_name_str), ..Default::default() },
                            Some(d3d_device_handle.0),
                        ) {
                            Ok(video) => {
                                this.video_source = Some(video);
                                this.update_mpv_audio_mix();
                            }
                            Err(_) => this.video_source = None,
                        }
                    } else {
                        this.video_source = None;
                    }
                    cx.notify();
                });
            }
        }).detach();
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
                self.clip_start = -1.0;
                self.clip_end = -1.0;
            } else {
                self.clip_end = v.position().as_secs_f64();
            }
            cx.notify();
        }
    }

    pub fn save_clip(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.clip_start >= 0.0 && self.clip_end >= 0.0 {
            self.show_export_modal = true;
            cx.notify();
        }
    }

    pub fn perform_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.clip_start;
        let end = self.clip_end;
        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        // Ensure ffconcat is up to date before exporting
        crate::utils::generate_session_playlist(&source_name, self.recording_session_id);

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

        // Start export
        self.app_state.export_running.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.app_state.export_progress.lock() = 0.0;
        
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

        let app_state_for_progress = self.app_state.clone();
        let view_handle = cx.entity().downgrade();
        
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                for i in 1..=100 {
                    let progress = i as f32 / 100.0;
                    *app_state_for_progress.export_progress.lock() = progress;
                    let _ = view_handle.update(&mut cx, |_, cx| cx.notify());
                    let _ = cx.background_executor().timer(std::time::Duration::from_millis(if export_reencode { 50 } else { 5 })).await;
                    if !app_state_for_progress.export_running.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                }
            }
        }).detach();

        let ffmpeg_task = cx.background_spawn(async move {
            use std::process::Command;

            let mut cmd = Command::new(ffmpeg_path.clone());
            cmd.arg("-y")
               .arg("-ss").arg(format!("{:.3}", start))
               .arg("-to").arg(format!("{:.3}", end))
               .arg("-f").arg("concat")
               .arg("-safe").arg("0")
               .arg("-i").arg(playlist_path.clone())
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

            eprintln!("[UI] Running FFmpeg for clip: {:?}", cmd);
            let clip_output = cmd.output();

            // Extract a thumbnail from the middle of the clip
            let duration = end - start;
            let thumb_time = start + (duration / 2.0);
            let mut thumb_path = output_path.clone();
            thumb_path.set_extension("jpg");

            let mut thumb_cmd = Command::new(&ffmpeg_path);
            thumb_cmd.arg("-y")
                     .arg("-ss").arg(format!("{:.3}", thumb_time))
                     .arg("-i").arg(&playlist_path)
                     .arg("-vframes").arg("1")
                     .arg("-q:v").arg("2")
                     .arg(&thumb_path);
            
            eprintln!("[UI] Running FFmpeg for thumbnail: {:?}", thumb_cmd);
            let _ = thumb_cmd.output(); // Ignore thumbnail errors, it's non-critical

            (clip_output, output_path, clips_dir)
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

    pub fn toggle_recording_internal(&mut self) {
        if let Some(pipeline) = self.app_state.pipeline.lock().take() {
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.app_state.is_recording.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let is_recording = self
            .app_state
            .is_recording
            .load(std::sync::atomic::Ordering::SeqCst);

        if is_recording {
            if let Some(pipeline) = self.app_state.pipeline.lock().take() {
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

            let mut video_settings = config.global_video.clone();
            let mut audio_routing = config.global_audio_tracks.clone();

            let hwnd = if source_name == "monitor" {
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

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            // Unified Folder: Just use the game directory root
            let game_dir = crate::utils::get_storage_root()
                .join(crate::utils::clean_title(&source_name));
            let _ = std::fs::create_dir_all(&game_dir);

            // Normalize path for GStreamer
            let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");

            // Calculate the current total duration across all existing segments 
            // by scanning the actual playlist data, ensuring we aren't using stale UI state.
            let ts_offset_ns = if let Some((_, blocks)) = crate::utils::generate_session_playlist(&source_name, None) {
                let total_secs: f64 = blocks.iter().map(|b| b.duration_secs).sum();
                (total_secs * 1_000_000_000.0) as i64
            } else {
                0
            };

            let pipeline_str = crate::engine::generate_pipeline_string(
                &video_settings,
                &game_dir_str,
                &audio_routing,
                &config.mic_settings,
                hwnd,
                target_pid,
                config.selected_adapter_index,
                timestamp, // Pass the timestamp as the unique session ID
                ts_offset_ns, // Pass the calculated DTS offset
            );

            match gst::parse::launch(&pipeline_str) {
                Ok(pipeline) => {
                    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
                    
                    // Share D3D11 device handle
                    if let Some(handle) = self.app_state.d3d11_device.lock().as_ref() {
                        let device_ptr = handle.0.0 as u64;
                        let mut context = gst::Context::new("gst.d3d11.device.handle", true);
                        if let Some(c) = context.get_mut() {
                            c.structure_mut().set("device-handle", &device_ptr);
                        }
                        pipeline.set_context(&context);
                    }

                    let _ = pipeline.set_state(gst::State::Playing);
                    
                    // Monitor the bus for errors
                    if let Some(bus) = pipeline.bus() {
                        let view_handle = cx.entity().downgrade();
                        cx.spawn(|_, cx: &mut AsyncApp| {
                            let mut cx = cx.clone();
                            async move {
                                let mut messages = bus.stream();
                                use futures_util::StreamExt;
                                while let Some(msg) = messages.next().await {
                                    let msg_view = msg.view();
                                    match msg_view {
                                        gst::MessageView::Error(err) => {
                                            eprintln!("[GStreamer] Pipeline Error: {} ({:?})", err.error(), err.debug());
                                            let _ = view_handle.update(&mut cx, |this, cx| {
                                                if let Some(any_window) = cx.windows().first() {
                                                    let _ = any_window.update(cx, |_, window, cx| {
                                                        this.show_toast("Recording Error", Some("The GStreamer pipeline crashed."), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                                                    });
                                                }
                                                this.toggle_recording_internal(); // Helper to clean up state
                                            });
                                            break;
                                        }
                                        gst::MessageView::Warning(warn) => {
                                            eprintln!("[GStreamer] Pipeline Warning: {} ({:?})", warn.error(), warn.debug());
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

                    // Register session in DB
                    if let Ok(db) = crate::db::GameDatabase::open(&game_dir) {
                        let _ = db.register_session(timestamp);
                    }

                    // Periodic DB update loop
                    let view_handle = cx.entity().downgrade();
                    let game_dir_for_loop = game_dir.clone();
                    let source_name_for_loop = source_name.clone();
                    cx.spawn(move |_, cx: &mut AsyncApp| {
                        let mut cx = cx.clone();
                        async move {
                            loop {
                                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                                let mut is_still_recording = false;
                                let _ = view_handle.update(&mut cx, |this, _| {
                                    is_still_recording = this.app_state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                                    if is_still_recording {
                                        if let Some(id) = this.recording_session_id {
                                            // Scan disk for actual recorded duration to ensure database accuracy
                                            if let Some((_, blocks)) = crate::utils::generate_session_playlist(&source_name_for_loop, Some(id)) {
                                                if let Some(block) = blocks.iter().find(|b| b.start_timestamp == id) {
                                                    if let Ok(db) = crate::db::GameDatabase::open(&game_dir_for_loop) {
                                                        let _ = db.update_duration(id, block.duration_secs);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                                if !is_still_recording { break; }
                            }
                        }
                    }).detach();

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
            .bg(gpui::rgba(0x000000_cc))
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
                            .child({
                                let export_running = self.app_state.export_running.load(std::sync::atomic::Ordering::SeqCst);
                                let progress = *self.app_state.export_progress.lock();
                                
                                div()
                                    .when(export_running, |this| {
                                        this.absolute()
                                            .inset_0()
                                            .bg(theme.tokens.card)
                                            .rounded_xl()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .p_6()
                                            .gap_4()
                                            .child(
                                                Spinner::new()
                                                    .size(SpinnerSize::Xl)
                                            )
                                            .child(div().text_lg().font_weight(FontWeight::BOLD).child("Exporting Clip..."))
                                            .child(
                                                VStack::new()
                                                    .w_full()
                                                    .gap_1()
                                                    .child(
                                                        adabraka_ui::components::progress::ProgressBar::new(progress)
                                                            .h(px(8.0))
                                                    )
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).text_center().child(format!("{:.0}%", progress * 100.0)))
                                            )
                                    })
                            })
                    )
            )
    }

    pub fn open_volume_popover(&mut self, track_idx: usize, cx: &mut Context<Self>) {
        self.audio_track_volume_popover = Some(track_idx);
        self.last_audio_track_volume_popover = Some(track_idx);
        
        let current_playback_volume = self.playback_volumes.get(track_idx).copied().unwrap_or(100.0);
        self.volume_slider_last_value = current_playback_volume as f32;
        
        // Lock vertical position on open so it doesn't move during scroll
        let header_offset = 36.0 + (track_idx as f32 * 33.0); 
        self.popover_fixed_top = f32::from(self.timeline_bounds.top()) + header_offset + 16.0;

        self.volume_slider_state.update(cx, |state, cx| {
            state.set_value((current_playback_volume / 1.5) as f32, cx);
        });
        
        cx.notify();
    }

    pub fn delete_session(&mut self, session_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = self.app_state.manual_sessions.get(&session_id) {
            let title = session.title.clone();
            self.app_state.manual_sessions.remove(&session_id);
            self.app_state.game_registry.remove(&title);
            let mut config = AppConfig::load();
            config.game_registry.remove(&title);
            config.save();
            self.show_toast("Source Deleted", Some(format!("Removed {} from library.", title)), adabraka_ui::overlays::toast::ToastVariant::Default, window, cx);
            self.session_to_delete = None;
            cx.notify();
        }
    }

    pub fn delete_clip(&mut self, clip: crate::state::Clip, window: &mut Window, cx: &mut Context<Self>) {
        let _ = std::fs::remove_file(&clip.path);
        if let Some(thumb) = &clip.thumbnail_path {
            let _ = std::fs::remove_file(thumb);
        }
        self.show_toast("Clip Deleted", Some("The file has been removed from disk."), adabraka_ui::overlays::toast::ToastVariant::Default, window, cx);
        self.clip_to_delete = None;
        cx.notify();
    }

    pub fn render_workspace(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        if let Some(source) = &self.advanced_settings_source {
            root = root.child(self.render_advanced_settings_dialog(&source, window, cx));
        }

        if self.show_export_modal {
            root = root.child(self.render_export_modal(window, cx));
        }

        // Deletion Confirmations
        if let Some(session_id) = self.session_to_delete {
            let view = cx.entity().downgrade();
            root = root.child(
                cx.new(|cx| {
                    AlertDialog::new(cx)
                        .title("Delete Source")
                        .description("Are you sure you want to remove this source?")
                        .destructive(true)
                        .on_cancel({
                            let view = view.clone();
                            move |_, cx| { let _ = view.update(cx, |this, cx| { this.session_to_delete = None; cx.notify(); }); }
                        })
                        .on_action({
                            let view = view.clone();
                            move |window, cx| { let _ = view.update(cx, |this, cx| { this.delete_session(session_id, window, cx); }); }
                        })
                })
            );
        }

        if let Some(clip) = self.clip_to_delete.clone() {
            let view = cx.entity().downgrade();
            root = root.child(
                cx.new(|cx| {
                    AlertDialog::new(cx)
                        .title("Delete Clip")
                        .description(format!("Permanently delete '{}'?", clip.title))
                        .destructive(true)
                        .on_cancel({
                            let view = view.clone();
                            move |_, cx| { let _ = view.update(cx, |this, cx| { this.clip_to_delete = None; cx.notify(); }); }
                        })
                        .on_action({
                            let view = view.clone();
                            move |window, cx| { let _ = view.update(cx, |this, cx| { this.delete_clip(clip.clone(), window, cx); }); }
                        })
                })
            );
        }

        // Clip Popover Menu
        if let Some((pos, clip)) = self.clip_popover.clone() {
            let items = vec![
                PopoverMenuItem::new("play", "Play Clip")
                    .icon("play")
                    .on_click({
                        let view = cx.entity().downgrade();
                        let clip = clip.clone();
                        move |window, cx| { let _ = view.update(cx, |this, cx| { 
                            this.clip_popover = None;
                            this.set_active_view(ActiveView::Dashboard, cx);
                            this.load_video(&clip.path.to_string_lossy(), window, cx);
                        }); }
                    }),
                PopoverMenuItem::new("folder", "Show in Folder")
                    .icon("folder")
                    .on_click({
                        let view = cx.entity().downgrade();
                        let clip = clip.clone();
                        move |_, cx| { let _ = view.update(cx, |this, cx| {
                            this.clip_popover = None;
                            let _ = std::process::Command::new("explorer").arg("/select,").arg(&clip.path).spawn();
                            cx.notify();
                        }); }
                    }),
                PopoverMenuItem::new("delete", "Delete Clip")
                    .icon("trash")
                    .on_click({
                        let view = cx.entity().downgrade();
                        let clip = clip.clone();
                        move |_, cx| { let _ = view.update(cx, |this, cx| {
                            this.clip_popover = None;
                            this.clip_to_delete = Some(clip.clone());
                            cx.notify();
                        }); }
                    }),
            ];

            root = root.child(
                PopoverMenu::new(pos, items)
                    .on_close({
                        let view = cx.entity().downgrade();
                        move |_, cx| { let _ = view.update(cx, |this, cx| { this.clip_popover = None; cx.notify(); }); }
                    })
            );
        }

        // Floating Volume Popover
        if let Some(_) = self.audio_track_volume_popover {
            let theme = use_theme();
            let view = cx.entity().downgrade();
            
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .on_mouse_up(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.is_adjusting_volume = false;
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.audio_track_volume_popover = None;
                        this.last_audio_track_volume_popover = None;
                        this.is_adjusting_volume = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(self.timeline_bounds.left() + px(95.0))
                            .top(px(self.popover_fixed_top - 70.0))
                            .w(px(60.0))
                            .h(px(140.0))
                            .p_3()
                            .bg(theme.tokens.card)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .rounded_md()
                            .shadow_lg()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.is_adjusting_volume = true;
                                cx.notify();
                            }))
                            .child(
                                VStack::new()
                                    .size_full()
                                    .items_center()
                                    .justify_between()
                                    .child(div().text_xs().font_weight(FontWeight::BOLD).child(format!("{:.0}%", self.volume_slider_last_value)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .w_full()
                                            .flex()
                                            .justify_center()
                                            .child(
                                                adabraka_ui::components::slider::Slider::new(self.volume_slider_state.clone())
                                                    .vertical()
                                                    .on_change(move |val, _window, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            let volume = (val as f64 * 1.5).clamp(0.0, 150.0);
                                                            if let Some(track_idx) = this.last_audio_track_volume_popover {
                                                                if this.playback_volumes.len() <= track_idx {
                                                                    this.playback_volumes.resize(track_idx + 1, 100.0);
                                                                }
                                                                this.playback_volumes[track_idx] = volume;
                                                                this.update_mpv_audio_mix();
                                                            }
                                                            this.volume_slider_last_value = volume as f32;
                                                            cx.notify();
                                                        });
                                                    })
                                            )
                                    )
                            )
                    )
            );
        }

        root.child(self.toast_manager.clone())
    }
}

impl Render for LumaWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_workspace(window, cx)
    }
}
