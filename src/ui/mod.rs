use crate::config::{AppConfig, AudioRouting};
use crate::state::AppState;
use crate::video_player::Video;
use adabraka_ui::prelude::*;
use adabraka_ui::components::dropdown::DropdownState;
use gpui::*;
use std::sync::Arc;
use gstreamer::prelude::*;
use gstreamer as gst;

mod add_source;
mod clips;
mod dashboard;
mod settings;
mod setup_wizard;
mod sidebar;
mod timeline;

use adabraka_ui::overlays::popover_menu::{PopoverMenu, PopoverMenuItem};
use adabraka_ui::display::data_table::DataTable;

pub struct LumaWorkspace {
    pub active_view: ActiveView,
    pub clips_view_mode: ClipsViewMode,
    pub settings_tab_index: usize,
    pub app_state: Arc<AppState>,
    pub video_source: Option<Video>,
    pub preview_video_source: Option<Video>,
    pub selected_source: Option<String>,
    pub show_add_source_modal: bool,
    pub advanced_settings_source: Option<String>,
    pub session_to_delete: Option<i32>,
    pub clip_to_delete: Option<crate::state::Clip>,
    pub clip_to_preview: Option<crate::state::Clip>,
    pub last_preview_mouse_move: std::time::Instant,
    pub show_preview_controls: bool,
    pub last_preview_controls_task: Option<Task<()>>,
    pub is_scrubbing_preview: bool,
    pub preview_scrubbing_progress: f32,
    pub preview_audio_enabled: Vec<bool>,
    pub preview_volume: f64,
    pub preview_volume_dragging: bool,
    pub preview_volume_bounds: Bounds<Pixels>,
    pub clip_popover: Option<(Point<Pixels>, crate::state::Clip)>,
    pub clip_table: Entity<DataTable<crate::state::Clip>>,
    pub clips_list_state: ListState,
    pub clip_start: f64,
    pub clip_end: f64,
    pub timeline_bounds: Bounds<Pixels>,
    pub is_scrubbing: bool,
    pub drag_target: Option<TimelineDragTarget>,
    pub scrubbing_progress: f32,
    pub last_seek_at: std::time::Instant,
    pub last_mix_update_at: std::time::Instant,
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
    pub form_auto_record: bool,
    pub form_target_process: Option<String>,
    pub audio_track_volume_popover: Option<usize>,
    pub last_audio_track_volume_popover: Option<usize>,
    pub volume_slider_last_value: f32,
    pub playback_volumes: Vec<f64>,
    pub popover_fixed_top: f32,
    pub last_notified_position: f64,
    pub timeline_zoom: f32,
    pub timeline_scroll: f32,
    pub is_refreshing_windows: bool,
    pub is_loading_video: bool,
    pub last_volume_update_at: std::time::Instant,
    pub storage_clips_mb: u64,
    pub storage_sessions_mb: u64,
    pub is_calculating_storage: bool,
    pub is_loading_clips: bool,
    pub cached_clips: Vec<crate::state::Clip>,
    pub library_items: Vec<crate::ui::clips::LibraryRow>,
    pub form_max_buffer_size_gb: i32,
    pub clips_search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub selected_clips: std::collections::HashSet<String>,
    pub selected_clip_for_details: Option<crate::state::Clip>,
    pub selected_game_filter: Option<String>,
    pub hovered_clip_idx: Option<usize>,
    pub hovered_clip_preview_progress: f32,
    pub volume_slider_state: Entity<adabraka_ui::components::slider::SliderState>,
    pub recording_start_time: Option<std::time::Instant>,
    pub recording_session_id: Option<u64>,
    // Setup wizard state
    pub show_setup_wizard: bool,
    pub setup_wizard_step: usize,
    pub setup_storage_path: String,
    pub setup_selected_encoder: String,
    pub setup_detected_encoders: Vec<setup_wizard::DetectedEncoder>,
    /// Which hotkey slot is currently listening for a new binding (None = not editing)
    /// Slots: 0=toggle recording, 1=save clip, 2=toggle mic, 3=push-to-talk,
    /// 4=marker flag, 5=marker kill, 6=marker death, 7=marker highlight
    pub hotkey_listening: Option<usize>,
    pub hotkey_focus_handle: FocusHandle,
    pub timeline_markers: Vec<crate::state::TimelineMarker>,
    // Settings form state — video tab
    pub settings_form_encoder: String,
    pub settings_form_resolution: String,
    pub settings_form_fps: i32,
    pub settings_form_rate_control: i32,
    pub settings_form_bitrate: i32,
    pub settings_form_cq: i32,
    pub settings_form_retention: i32,
    pub settings_form_preset: String,
    pub settings_form_gop: i32,
    pub settings_form_bframes: i32,
    pub settings_form_zero_latency: bool,
    pub settings_form_lookahead: bool,
    pub settings_form_lookahead_frames: i32,
    pub settings_form_spatial_aq: bool,
    pub settings_form_temporal_aq: bool,
    pub settings_show_advanced_video: bool,
    // Settings form state — audio/mic tab
    pub settings_form_mic_device: String,
    pub settings_form_mic_force_mono: bool,
    pub settings_form_mic_gain: f32,
    pub settings_form_mic_noise_suppression: bool,
    pub settings_form_mic_gate_enabled: bool,
    pub settings_form_mic_gate_threshold: f32,
    pub settings_form_mic_compressor_enabled: bool,
    pub settings_form_mic_compressor_threshold: f32,
    pub settings_form_mic_compressor_ratio: f32,
    pub settings_form_mic_limiter_enabled: bool,
    pub settings_form_mic_limiter_threshold: f32,
    // Mic monitor loopback
    pub mic_monitor_pipeline: Option<gst::Pipeline>,
    // Settings form state — storage tab
    pub settings_form_auto_delete_enabled: bool,
    pub settings_form_auto_delete_days: i32,
    pub settings_form_export_format: String,
    // Dropdown states (persisted across renders)
    pub custom_res_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub dd_encoder: Entity<DropdownState>,
    pub dd_resolution: Entity<DropdownState>,
    pub dd_fps: Entity<DropdownState>,
    pub dd_rate_control: Entity<DropdownState>,
    pub dd_preset: Entity<DropdownState>,
    pub dd_mic: Entity<DropdownState>,
    pub dd_export_format: Entity<DropdownState>,
    _quit_subscription: Option<Subscription>,
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

        let mut workspace = Self {
            active_view: ActiveView::Dashboard,
            clips_view_mode: ClipsViewMode::Grid,
            settings_tab_index: 0,
            app_state,
            video_source: None,
            preview_video_source: None,
            selected_source: None,
            show_add_source_modal: false,
            advanced_settings_source: None,
            session_to_delete: None,
            clip_to_delete: None,
            clip_to_preview: None,
            last_preview_mouse_move: std::time::Instant::now(),
            show_preview_controls: true,
            last_preview_controls_task: None,
            is_scrubbing_preview: false,
            preview_audio_enabled: Vec::new(),
            preview_volume: 100.0,
            preview_volume_dragging: false,
            preview_volume_bounds: Bounds::default(),
            preview_scrubbing_progress: 0.0,
            clip_popover: None,
            clip_table,
            clips_list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            clip_start: -1.0,
            clip_end: -1.0,
            timeline_bounds: Bounds::default(),
            is_scrubbing: false,
            drag_target: None,
            scrubbing_progress: 0.0,
            last_seek_at: std::time::Instant::now(),
            last_mix_update_at: std::time::Instant::now(),
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
            form_editing_track_index: Option::None,
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
            form_auto_record: false,
            form_target_process: None,
            audio_track_volume_popover: None,
            last_audio_track_volume_popover: None,
            volume_slider_last_value: 100.0,
            playback_volumes: vec![100.0; 10], // Support up to 10 tracks
            popover_fixed_top: 0.0,
            last_notified_position: 0.0,
            timeline_zoom: 1.0,
            timeline_scroll: 0.0,
            is_refreshing_windows: false,
            is_loading_video: false,
            last_volume_update_at: std::time::Instant::now(),
            storage_clips_mb: 0,
            storage_sessions_mb: 0,
            is_calculating_storage: false,
            is_loading_clips: false,
            cached_clips: Vec::new(),
            library_items: Vec::new(),
            form_max_buffer_size_gb: config.max_buffer_size_gb,
            clips_search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            selected_clips: std::collections::HashSet::new(),
            selected_clip_for_details: None,
            selected_game_filter: None,
            hovered_clip_idx: None,
            hovered_clip_preview_progress: 0.0,
            volume_slider_state: cx.new(|cx| adabraka_ui::components::slider::SliderState::new(cx)),
            recording_start_time: None,
            recording_session_id: None,
            // Setup wizard
            show_setup_wizard: config.is_first_run(),
            setup_wizard_step: 0,
            setup_storage_path: config.storage_path.clone(),
            setup_selected_encoder: {
                let detected = setup_wizard::detect_available_encoders();
                let default_enc = detected.first().map(|e| e.id.clone()).unwrap_or_else(|| config.global_video.encoder.clone());
                default_enc
            },
            setup_detected_encoders: setup_wizard::detect_available_encoders(),
            hotkey_listening: None,
            hotkey_focus_handle: cx.focus_handle(),
            timeline_markers: Vec::new(),
            // Settings form state — video
            settings_form_encoder: config.global_video.encoder.clone(),
            settings_form_resolution: config.global_video.resolution.clone(),
            settings_form_fps: config.global_video.fps,
            settings_form_rate_control: config.global_video.rate_control_index,
            settings_form_bitrate: config.global_video.bitrate_kbps,
            settings_form_cq: config.global_video.cq_level,
            settings_form_retention: config.global_video.retention_minutes,
            settings_form_preset: config.global_video.preset.clone(),
            settings_form_gop: config.global_video.gop_size,
            settings_form_bframes: config.global_video.bframes,
            settings_form_zero_latency: config.global_video.zero_latency,
            settings_form_lookahead: config.global_video.lookahead,
            settings_form_lookahead_frames: config.global_video.lookahead_frames,
            settings_form_spatial_aq: config.global_video.spatial_aq,
            settings_form_temporal_aq: config.global_video.temporal_aq,
            settings_show_advanced_video: false,
            // Settings form state — mic
            settings_form_mic_device: config.mic_settings.device_name.clone(),
            settings_form_mic_force_mono: config.mic_settings.force_mono,
            settings_form_mic_gain: config.mic_settings.gain_db,
            settings_form_mic_noise_suppression: config.mic_settings.noise_suppression,
            settings_form_mic_gate_enabled: config.mic_settings.noise_gate_enabled,
            settings_form_mic_gate_threshold: config.mic_settings.noise_gate_threshold,
            settings_form_mic_compressor_enabled: config.mic_settings.compressor_enabled,
            settings_form_mic_compressor_threshold: config.mic_settings.compressor_threshold,
            settings_form_mic_compressor_ratio: config.mic_settings.compressor_ratio,
            settings_form_mic_limiter_enabled: config.mic_settings.limiter_enabled,
            settings_form_mic_limiter_threshold: config.mic_settings.limiter_threshold,
            mic_monitor_pipeline: None,
            // Settings form state — storage
            settings_form_auto_delete_enabled: config.auto_delete_clips_days.is_some(),
            settings_form_auto_delete_days: config.auto_delete_clips_days.unwrap_or(30),
            settings_form_export_format: config.default_export_format.clone(),
            // Dropdown states
            custom_res_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            dd_encoder: cx.new(|cx| DropdownState::new(cx)),
            dd_resolution: cx.new(|cx| DropdownState::new(cx)),
            dd_fps: cx.new(|cx| DropdownState::new(cx)),
            dd_rate_control: cx.new(|cx| DropdownState::new(cx)),
            dd_preset: cx.new(|cx| DropdownState::new(cx)),
            dd_mic: cx.new(|cx| DropdownState::new(cx)),
            dd_export_format: cx.new(|cx| DropdownState::new(cx)),
            _quit_subscription: None,
        };

        // Register graceful shutdown handler
        let quit_sub = cx.on_app_quit(|this, _cx| {
            log::info!("[Shutdown] Graceful shutdown initiated...");

            // 0. Stop mic monitor if active
            if let Some(pipeline) = this.mic_monitor_pipeline.take() {
                let _ = pipeline.set_state(gstreamer::State::Null);
                if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                    provider.subscribers.remove(&0xFFFF_FFFF_FFFF_FFFFu64);
                }
            }

            // 1. Stop recording pipeline with EOS
            let pipeline = this.app_state.recording.pipeline.lock().take();
            if let Some(pipeline) = pipeline {
                log::info!("[Shutdown] Stopping recording pipeline...");
                *this.app_state.recording.phase.lock() = crate::state::RecordingPhase::Stopping;
                pipeline.send_event(gstreamer::event::Eos::new());
                if let Some(bus) = pipeline.bus() {
                    let _ = bus.timed_pop_filtered(
                        gstreamer::ClockTime::from_seconds(3),
                        &[gstreamer::MessageType::Eos],
                    );
                }
                let _ = pipeline.set_state(gstreamer::State::Null);

                // Fixup EOS segments
                let source = this.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                let game_dir = crate::utils::get_storage_root().join(crate::utils::clean_title(&source));
                crate::utils::fixup_eos_segments(&game_dir);
                *this.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
                log::info!("[Shutdown] Recording pipeline stopped.");
            }

            // 2. Stop mic provider pipeline
            if let Some(mic) = this.app_state.mic_provider.lock().take() {
                log::info!("[Shutdown] Mic provider released ({} subscribers)", mic.subscribers.len());
                mic.subscribers.clear();
            }

            // 3. Stop virtual audio routers
            {
                let mut routers = this.app_state.virtual_audio_routers.lock();
                if !routers.is_empty() {
                    log::info!("[Shutdown] Stopping {} virtual audio routers", routers.len());
                    routers.clear(); // Drop calls stop on each router
                }
            }

            // 4. Stop video players
            this.video_source = None;
            this.preview_video_source = None;

            log::info!("[Shutdown] Graceful shutdown complete.");
            async {}
        });
        workspace._quit_subscription = Some(quit_sub);

        // High-performance refresh loop for video playback
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
                                if (pos - this.last_notified_position).abs() > 0.05 || this.is_scrubbing {
                                    this.last_notified_position = pos;
                                    should_notify = true;
                                    
                                    // Continuously re-check the audio mix to handle segments with different track counts.
                                    // This ensures that when moving from a 1-track recording to a 2-track recording (or vice-versa),
                                    // the audio engine correctly adapts without requiring a manual volume adjustment.
                                    this.update_mpv_audio_mix();
                                }
                            }
                        }
                    });

                    if should_notify {
                        this.update(&mut cx, |_, cx| cx.notify()).ok();
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(32)) // Drop to 30fps for UI sync to save CPU
                            .await;
                    } else {
                        // Sleep much longer when idle (10fps check)
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
            // Get the actual number of audio tracks present in the current file/segment
            let actual_track_count = v.audio_tracks().len();
            
            let active_tracks = self.get_current_audio_tracks();
            let mut enabled_aids = Vec::new();
            for (i, t) in active_tracks.iter().enumerate() {
                // ONLY attempt to mix tracks that actually exist in the current stream.
                // If a user added a Mic track recently, older segments won't have it,
                // and trying to access [aid2] would crash the filter chain.
                if t.enabled && i < actual_track_count {
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

    pub fn update_preview_audio_mix(&self) {
        if let Some(v) = &self.preview_video_source {
            let enabled_ids: Vec<usize> = self.preview_audio_enabled.iter()
                .enumerate()
                .filter(|(_, &on)| on)
                .map(|(i, _)| i)
                .collect();

            if enabled_ids.is_empty() {
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", "");
            } else if enabled_ids.len() == 1 {
                let idx = enabled_ids[0];
                let _ = v.read().mpv.set_property("lavfi-complex", "");
                let _ = v.read().mpv.set_property("aid", (idx + 1) as i64);
            } else {
                let mut complex = String::new();
                for &idx in &enabled_ids {
                    complex.push_str(&format!("[aid{}]anull[a{}];", idx + 1, idx + 1));
                }
                for &idx in &enabled_ids {
                    complex.push_str(&format!("[a{}]", idx + 1));
                }
                complex.push_str(&format!("amix=inputs={}:normalize=0[ao]", enabled_ids.len()));
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
            }
        }
    }

    pub fn init_preview_audio_tracks(&mut self) {
        if let Some(v) = &self.preview_video_source {
            let tracks = v.audio_tracks();
            let count = tracks.len();
            if self.preview_audio_enabled.len() != count {
                self.preview_audio_enabled = vec![true; count];
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
                .unwrap_or_default()
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
        self.hotkey_listening = None;
        
        if view == ActiveView::Clips {
            self.refresh_clips(cx);
        } else {
            // Clear metadata when not in library to save RAM
            self.cached_clips.clear();
            self.clip_table.update(cx, |table, cx| {
                table.set_data(Vec::new(), cx);
            });
        }

        if view == ActiveView::Settings {
            let config = AppConfig::load();
            self.form_max_buffer_size_gb = config.max_buffer_size_gb;
            // Reload all settings form state from config
            self.settings_form_encoder = config.global_video.encoder.clone();
            self.settings_form_resolution = config.global_video.resolution.clone();
            self.settings_form_fps = config.global_video.fps;
            self.settings_form_rate_control = config.global_video.rate_control_index;
            self.settings_form_bitrate = config.global_video.bitrate_kbps;
            self.settings_form_cq = config.global_video.cq_level;
            self.settings_form_retention = config.global_video.retention_minutes;
            self.settings_form_preset = config.global_video.preset.clone();
            self.settings_form_gop = config.global_video.gop_size;
            self.settings_form_bframes = config.global_video.bframes;
            self.settings_form_zero_latency = config.global_video.zero_latency;
            self.settings_form_lookahead = config.global_video.lookahead;
            self.settings_form_lookahead_frames = config.global_video.lookahead_frames;
            self.settings_form_spatial_aq = config.global_video.spatial_aq;
            self.settings_form_temporal_aq = config.global_video.temporal_aq;
            self.settings_form_mic_device = config.mic_settings.device_name.clone();
            self.settings_form_mic_force_mono = config.mic_settings.force_mono;
            self.settings_form_mic_gain = config.mic_settings.gain_db;
            self.settings_form_mic_noise_suppression = config.mic_settings.noise_suppression;
            self.settings_form_mic_gate_enabled = config.mic_settings.noise_gate_enabled;
            self.settings_form_mic_gate_threshold = config.mic_settings.noise_gate_threshold;
            self.settings_form_mic_compressor_enabled = config.mic_settings.compressor_enabled;
            self.settings_form_mic_compressor_threshold = config.mic_settings.compressor_threshold;
            self.settings_form_mic_compressor_ratio = config.mic_settings.compressor_ratio;
            self.settings_form_mic_limiter_enabled = config.mic_settings.limiter_enabled;
            self.settings_form_mic_limiter_threshold = config.mic_settings.limiter_threshold;
            self.settings_form_auto_delete_enabled = config.auto_delete_clips_days.is_some();
            self.settings_form_auto_delete_days = config.auto_delete_clips_days.unwrap_or(30);
            self.settings_form_export_format = config.default_export_format.clone();
            
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

    pub fn refresh_clips(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_clips { return; }
        self.is_loading_clips = true;
        
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let clips = cx.background_executor().spawn(async move {
                    crate::utils::fetch_all_clips()
                }).await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.cached_clips = clips.clone();
                    this.is_loading_clips = false;
                    this.rebuild_library_items(cx);

                    // Also update the table if it's currently being used
                    this.clip_table.update(cx, |table, cx| {
                        table.set_data(clips, cx);
                    });
                    
                    cx.notify();
                });
            }
        }).detach();
        
        cx.notify();
    }

    pub fn rebuild_library_items(&mut self, cx: &mut Context<Self>) {
        let clips = &self.cached_clips;
        let mut rows = Vec::new();

        if let Some(game_title) = &self.selected_game_filter {
            // Filtered view: show only clips for this game
            let game_clips: Vec<_> = clips.iter().filter(|c| &c.title == game_title).cloned().collect();
            for chunk in game_clips.chunks(4) {
                rows.push(crate::ui::clips::LibraryRow::ClipChunk(chunk.to_vec()));
            }
        } else {
            // Dashboard view: recent + game groups
            if !clips.is_empty() {
                rows.push(crate::ui::clips::LibraryRow::SectionHeader("MOST RECENT".to_string()));
                let recent: Vec<_> = clips.iter().take(4).cloned().collect();
                rows.push(crate::ui::clips::LibraryRow::ClipChunk(recent));
            }

            rows.push(crate::ui::clips::LibraryRow::SectionHeader("GAMES".to_string()));

            let mut game_groups: std::collections::BTreeMap<String, Vec<crate::state::Clip>> = std::collections::BTreeMap::new();
            for clip in clips.iter() {
                game_groups.entry(clip.title.clone()).or_default().push(clip.clone());
            }

            let game_titles: Vec<String> = game_groups.keys().cloned().collect();

            // Trigger portrait artwork fetches for all game titles
            self.fetch_portrait_artwork(&game_titles, cx);

            for chunk in game_titles.chunks(4) {
                let titles_with_data: Vec<(String, usize)> = chunk.iter()
                    .map(|title| {
                        let count = game_groups.get(title).map_or(0, |v| v.len());
                        (title.clone(), count)
                    })
                    .collect();
                rows.push(crate::ui::clips::LibraryRow::GameChunk(titles_with_data));
            }
        }

        self.library_items = rows;
        self.clips_list_state.reset(self.library_items.len());
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

        // Always create a fresh Video instance when switching sessions.
        // Reusing an existing mpv instance with loadfile("replace") can leave
        // stale decoder state (especially with HLS + AV1), causing libdav1d
        // OBU parsing errors and falling back to software decoding.
        self.video_source = None;

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
                        let url = format!("http://127.0.0.1:8080/{}/master.m3u8?token={}", safe_title, crate::get_hls_token());
                        (Some(url), b)
                    } else {
                        (None, Vec::new())
                    }
                };

                let _ = this.update(&mut cx, |this, cx| {
                    this.is_loading_video = false;
                    *this.app_state.recording.current_session_blocks.lock() = blocks;

                    if let Some(url) = video_url {
                        let d3d_device_ptr = this.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
                        match crate::video_player::Video::new_with_options(
                            &url,
                            crate::video_player::VideoOptions { source_name: Some(source_name_str), ..Default::default() },
                            d3d_device_ptr,
                        ) {
                            Ok(video) => {
                                this.video_source = Some(video);
                                this.timeline_markers.clear();
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

    /// Notify the mic provider that DSP settings have changed.
    pub fn notify_mic_dsp_changed(&self) {
        if let Some(provider) = self.app_state.mic_provider.lock().as_ref() {
            provider.notify_settings_changed();
        }
    }

    /// Restart the mic provider (needed when the GStreamer pipeline structure changes,
    /// e.g. toggling noise suppression which adds/removes the audiornnoise element).
    pub fn restart_mic_provider(&self) {
        // Drop the old provider (its pipeline thread will exit when subscribers are gone)
        let _ = self.app_state.mic_provider.lock().take();
        let config = crate::config::AppConfig::load();
        let provider_storage = self.app_state.mic_provider.clone();
        let device_name = config.mic_settings.device_name.clone();
        crate::audio::start_mic_provider(provider_storage, device_name);
        log::info!("[MicProvider] Restarted with noise_suppression={}", config.mic_settings.noise_suppression);
    }

    pub fn add_marker(&mut self, cx: &mut Context<Self>) {
        self.add_marker_with_kind(crate::state::MarkerKind::Flag, cx);
    }

    pub fn add_marker_with_kind(&mut self, kind: crate::state::MarkerKind, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            let time = v.position().as_secs_f64();
            // Don't add duplicate markers within 0.5s of each other
            if !self.timeline_markers.iter().any(|m| (m.time_secs - time).abs() < 0.5) {
                self.timeline_markers.push(crate::state::TimelineMarker {
                    time_secs: time,
                    kind,
                    label: None,
                });
                self.timeline_markers.sort_by(|a, b| a.time_secs.partial_cmp(&b.time_secs).unwrap());
                cx.notify();
            }
        }
    }

    pub fn remove_marker(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.timeline_markers.len() {
            self.timeline_markers.remove(index);
            cx.notify();
        }
    }

    /// Remove all markers whose time falls within deleted footage.
    /// Called after retention cleanup trims the start of the buffer.
    pub fn prune_markers_before(&mut self, min_time_secs: f64) {
        self.timeline_markers.retain(|m| m.time_secs >= min_time_secs);
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

        // Ensure playlist is up to date before exporting
        crate::utils::generate_master_playlist(&source_name);

        let safe_title = crate::utils::clean_title(&source_name);
        let storage_root = crate::utils::get_storage_root();
        let playlist_path = storage_root.join(&safe_title).join("master.m3u8");

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
            .unwrap_or_default()
            .as_secs();
        let output_path = clips_dir.join(format!("clip_{}_{}.mp4", safe_title, timestamp));

        let ffmpeg_path = crate::utils::get_ffmpeg_path();

        if ffmpeg_path.to_str() != Some("ffmpeg") && !ffmpeg_path.exists() {
            self.show_toast(
                "FFmpeg Not Found",
                Some("Place ffmpeg.exe in the bin/ folder next to Luma, or install it to PATH."),
                adabraka_ui::overlays::toast::ToastVariant::Error,
                window,
                cx,
            );
            return;
        }

        // Start export
        *self.app_state.export.phase.lock() = crate::state::ExportPhase::Exporting;
        *self.app_state.export.progress.lock() = 0.0;
        
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
                    *app_state_for_progress.export.progress.lock() = progress;
                    let _ = view_handle.update(&mut cx, |_, cx| cx.notify());
                    let _ = cx.background_executor().timer(std::time::Duration::from_millis(if export_reencode { 50 } else { 5 })).await;
                    if *app_state_for_progress.export.phase.lock() != crate::state::ExportPhase::Exporting {
                        break;
                    }
                }
            }
        }).detach();

        let ffmpeg_task = cx.background_spawn(async move {
            use std::process::Command;

            let build_cmd = |hwaccel: bool| {
                let mut cmd = Command::new(ffmpeg_path.clone());
                cmd.arg("-y");
                if hwaccel {
                    cmd.arg("-hwaccel").arg("cuda")
                       .arg("-hwaccel_output_format").arg("cuda");
                }
                cmd.arg("-ss").arg(format!("{:.3}", start))
                   .arg("-to").arg(format!("{:.3}", end))
                   .arg("-allowed_extensions").arg("ALL")
                   .arg("-i").arg(playlist_path.clone())
                   .arg("-map").arg("0:v:0");

                let mut physical_stream_idx = 0;
                for track in &audio_tracks {
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
                        .arg(format!("{}k", bitrate));
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
                cmd
            };

            // Try with CUDA hardware decoding first, fall back to software
            let mut cmd = build_cmd(true);
            log::info!("[UI] Running FFmpeg for clip (hwaccel cuda): {:?}", cmd);
            let clip_output = match cmd.output() {
                Ok(out) if out.status.success() => Ok(out),
                _ => {
                    log::warn!("[UI] CUDA decode failed, retrying with software decoder");
                    let mut cmd = build_cmd(false);
                    log::info!("[UI] Running FFmpeg for clip (software): {:?}", cmd);
                    cmd.output()
                }
            };

            // Extract a thumbnail from the middle of the exported clip
            let duration = end - start;
            let thumb_time = duration / 2.0;
            let mut thumb_path = output_path.clone();
            thumb_path.set_extension("jpg");

            // Only generate thumbnail if the clip exported successfully
            if clip_output.as_ref().map_or(false, |o| o.status.success()) {
                let mut thumb_cmd = Command::new(&ffmpeg_path);
                thumb_cmd.arg("-y")
                         .arg("-ss").arg(format!("{:.3}", thumb_time))
                         .arg("-i").arg(&output_path)
                         .arg("-vframes").arg("1")
                         .arg("-q:v").arg("2")
                         .arg(&thumb_path);

                log::info!("[UI] Running FFmpeg for thumbnail: {:?}", thumb_cmd);
                if let Ok(out) = thumb_cmd.output() {
                    if !out.status.success() {
                        log::warn!("[UI] Thumbnail generation failed: {}", String::from_utf8_lossy(&out.stderr));
                    }
                }
            }

            (clip_output, output_path, clips_dir)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (result, output_path, clips_dir) = ffmpeg_task.await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.show_export_modal = false;
                    *this.app_state.export.phase.lock() = crate::state::ExportPhase::Idle;
                    
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
                                        log::error!("[UI] FFmpeg failed: {}", err);
                                        let err_summary = err.lines().rev()
                                            .find(|l| !l.trim().is_empty())
                                            .unwrap_or("FFmpeg returned an error.")
                                            .to_string();
                                        this.show_toast(
                                            SharedString::from("Export Failed"),
                                            Some(SharedString::from(err_summary)),
                                            adabraka_ui::overlays::toast::ToastVariant::Error,
                                            window,
                                            cx,
                                        );
                                    }
                                }
                                Err(e) => {
                                    log::error!("[UI] Failed to run FFmpeg: {}", e);
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
                    cx.notify();
                });
            }
        }).detach();
    }

    pub fn toggle_recording_internal(&mut self) {
        let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
        let game_dir = crate::utils::get_storage_root().join(crate::utils::clean_title(&source));

        if let Some(pipeline) = self.app_state.recording.pipeline.lock().take() {
            let _ = pipeline.set_state(gst::State::Null);
            
            // Perform fixup even on unexpected stops
            std::thread::spawn(move || {
                // Wait a moment for file handles to be released
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
                    // Wait for muxer to release file handles, then fixup EOS segments
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
        } else {
            if !phase.is_idle() { return; } // Only start from Idle
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

            // Unified Folder: Just use the game directory root
            let game_dir = crate::utils::get_storage_root()
                .join(crate::utils::clean_title(&source_name));
            let _ = std::fs::create_dir_all(&game_dir);

            // Normalize path for GStreamer
            let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");

            // Calculate the DTS offset by summing all segment filename durations.
            // This matches the working test pipeline's approach (compute_total_duration).
            let total_existing_duration = crate::utils::compute_total_duration(&game_dir);
            let ts_offset_ns = (total_existing_duration * 1_000_000_000.0) as i64;

            // Reset duration for the new recording
            *self.app_state.recording.current_recording_duration.lock() = ts_offset_ns as f64 / 1_000_000_000.0;

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
                    let pipeline = match pipeline.dynamic_cast::<gst::Pipeline>() {
                        Ok(p) => p,
                        Err(_) => {
                            *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Idle;
                            self.show_toast("Pipeline Error", Some("Failed to initialize recording pipeline."), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                            return;
                        }
                    };
                    
                    // Share D3D11 device handle with GStreamer for GPU-accelerated encoding.
                    // SAFETY: The device handle has an AddRef'd reference (see main.rs) and
                    // lives for the entire app lifetime. GStreamer receives the pointer as a
                    // u64 value and uses it for its own D3D11 device sharing mechanism.
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
                                    // 1. Capture all specific apps assigned to this track
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
                                        // 2. Fallback: If no specific apps, capture the recorded window/process
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
                                
                                // Track the start time of the currently writing fragment
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
                                                        
                                                        // Event-driven duration update: No disk polling needed!
                                                        {
                                                            let mut total_dur = app_state_bus.recording.current_recording_duration.lock();
                                                            *total_dur += duration_secs;

                                                            // Sync to database
                                                            if let Ok(db) = crate::db::GameDatabase::open(&game_dir_bus) {
                                                                let _ = db.update_duration(recording_id_bus, *total_dur);
                                                            }
                                                        }

                                                        // Update recording performance stats
                                                        {
                                                            use std::sync::atomic::Ordering::Relaxed;
                                                            let stats = &app_state_bus.recording.rec_stats;
                                                            stats.segments_written.fetch_add(1, Relaxed);

                                                            // Get file size for bitrate calculation
                                                            let seg_path = std::path::Path::new(&location);
                                                            let file_size = seg_path.metadata().map(|m| m.len()).unwrap_or(0);
                                                            stats.last_segment_bytes.store(file_size, Relaxed);

                                                            if duration_secs > 0.0 {
                                                                // Bitrate = file_size_bits / duration_secs / 1000
                                                                let bitrate = (file_size as f64 * 8.0) / duration_secs / 1000.0;
                                                                *stats.bitrate_kbps.lock() = bitrate;

                                                                // Disk write rate = file_size_bytes / duration_secs / 1MB
                                                                *stats.disk_write_mbps.lock() = file_size as f64 / duration_secs / (1024.0 * 1024.0);
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
                                                            
                                                            // Foreground retry loop (safe for AsyncApp)
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
                                            log::error!("[GStreamer] Pipeline Error: {} ({:?})", err.error(), err.debug());
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
                                            log::warn!("[GStreamer] Pipeline Warning: {} ({:?})", warn.error(), warn.debug());
                                        }
                                        gst::MessageView::Qos(qos) => {
                                            // Track dropped frames from QoS events
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

                    *self.app_state.recording.pipeline.lock() = Some(pipeline);
                    *self.app_state.recording.phase.lock() = crate::state::RecordingPhase::Recording;
                    
                    self.app_state.recording.reset_stats();
                    self.recording_start_time = Some(std::time::Instant::now());
                    self.recording_session_id = Some(timestamp);

                    // Register session in DB
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
                                let export_running = *self.app_state.export.phase.lock() == crate::state::ExportPhase::Exporting;
                                let progress = *self.app_state.export.progress.lock();
                                
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
        if self.audio_track_volume_popover == Some(track_idx) {
            self.audio_track_volume_popover = None;
            // Flush final volume to mpv in case throttling skipped the last update
            self.update_mpv_audio_mix();
            cx.notify();
            return;
        }

        self.audio_track_volume_popover = Some(track_idx);
        self.last_audio_track_volume_popover = Some(track_idx);
        
        let current_playback_volume = self.playback_volumes.get(track_idx).copied().unwrap_or(100.0);
        self.volume_slider_last_value = current_playback_volume as f32;

        self.volume_slider_state.update(cx, |state, cx| {
            state.set_step(0.1, cx);
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
        self.refresh_clips(cx);
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
                div()
                    .id("workspace-main")
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .child(match self.active_view {
                        ActiveView::Dashboard => self.render_dashboard(window, cx).into_any_element(),
                        ActiveView::Settings => self.render_settings_view(window, cx).into_any_element(),
                        ActiveView::Clips => self.render_clips(window, cx).into_any_element(),
                    })
            );

        if self.show_setup_wizard {
            root = root.child(self.render_setup_wizard(window, cx));
        }

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
                div()
                    .absolute()
                    .inset_0()
                    .bg(gpui::rgba(0x000000_cc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}) // Block clicks through
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
                                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Delete Source"))
                                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Are you sure you want to remove this source? This will stop any active recordings for it."))
                                    )
                                    .child(
                                        HStack::new()
                                            .justify_end()
                                            .gap_3()
                                            .child(
                                                Button::new("cancel-delete-source", "Cancel")
                                                    .variant(ButtonVariant::Ghost)
                                                    .on_click({
                                                        let view = view.clone();
                                                        move |_, _, cx| { let _ = view.update(cx, |this, cx| { this.session_to_delete = None; cx.notify(); }); }
                                                    })
                                            )
                                            .child(
                                                Button::new("confirm-delete-source", "Delete")
                                                    .variant(ButtonVariant::Destructive)
                                                    .on_click({
                                                        let view = view.clone();
                                                        move |_, window, cx| { let _ = view.update(cx, |this, cx| { this.delete_session(session_id, window, cx); }); }
                                                    })
                                            )
                                    )
                            )
                    )
            );
        }

        if let Some(clip) = self.clip_to_delete.clone() {
            let view = cx.entity().downgrade();
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(gpui::rgba(0x000000_cc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}) // Block clicks through
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
                                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Delete Clip"))
                                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(format!("Permanently delete '{}'?", clip.title)))
                                    )
                                    .child(
                                        HStack::new()
                                            .justify_end()
                                            .gap_3()
                                            .child(
                                                Button::new("cancel-delete-clip", "Cancel")
                                                    .variant(ButtonVariant::Ghost)
                                                    .on_click({
                                                        let view = view.clone();
                                                        move |_, _, cx| { let _ = view.update(cx, |this, cx| { this.clip_to_delete = None; cx.notify(); }); }
                                                    })
                                            )
                                            .child(
                                                Button::new("confirm-delete-clip", "Delete")
                                                    .variant(ButtonVariant::Destructive)
                                                    .on_click({
                                                        let view = view.clone();
                                                        move |_, window, cx| { let _ = view.update(cx, |this, cx| { this.delete_clip(clip.clone(), window, cx); }); }
                                                    })
                                            )
                                    )
                            )
                    )
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

        root.child(self.toast_manager.clone())
    }
}

impl Render for LumaWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_workspace(window, cx)
    }
}
