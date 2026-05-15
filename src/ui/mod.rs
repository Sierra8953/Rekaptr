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
mod export;
mod recording;
pub mod select;
mod settings;
pub mod volume_slider;
mod setup_wizard;
mod sidebar;
mod timeline;

use adabraka_ui::overlays::popover_menu::{PopoverMenu, PopoverMenuItem};
use adabraka_ui::display::data_table::DataTable;

#[allow(dead_code)]
pub struct RekaptrWorkspace {
    pub active_view: ActiveView,
    pub clips_view_mode: ClipsViewMode,
    pub settings_tab: SettingsTab,
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
    pub preview_vol_slider: Entity<volume_slider::VolumeSlider>,
    pub clip_popover: Option<(Point<Pixels>, crate::state::Clip)>,
    pub clip_table: Entity<DataTable<crate::state::Clip>>,
    pub clip_start: f64,
    pub clip_end: f64,
    pub clip_start_mark: Option<crate::state::ClipMark>,
    pub clip_end_mark: Option<crate::state::ClipMark>,
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
    pub add_source_search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub add_source_title_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub add_source_show_overrides: bool,
    pub playback_volumes: Vec<f64>,
    pub track_vol_sliders: Vec<Entity<volume_slider::VolumeSlider>>,
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
    pub form_max_buffer_size_gb: i32,
    pub clips_search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub clips_search_expanded: bool,
    pub favorite_clips: std::collections::HashSet<String>,
    pub selected_clips: std::collections::HashSet<String>,
    pub selected_clip_for_details: Option<crate::state::Clip>,
    pub clips_filter: crate::ui::clips::ClipsFilter,
    pub hovered_clip_idx: Option<usize>,
    pub hovered_clip_preview_progress: f32,
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
    pub dd_mic: Entity<DropdownState>,
    pub dd_export_format: Entity<DropdownState>,
    // AppSelect entities for settings
    pub select_encoder: Entity<select::AppSelect>,
    pub select_resolution: Entity<select::AppSelect>,
    pub select_fps: Entity<select::AppSelect>,
    pub select_preset: Entity<select::AppSelect>,
    pub update_state: crate::updater::UpdateState,
    pub update_has_receipt: bool,
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Startup,
    Video,
    Audio,
    Hotkeys,
    Storage,
    Export,
    About,
}

pub struct SettingsNavGroup {
    pub title: &'static str,
    pub items: &'static [SettingsTab],
}

pub const SETTINGS_NAV: &[SettingsNavGroup] = &[
    SettingsNavGroup { title: "GENERAL", items: &[SettingsTab::General, SettingsTab::Startup] },
    SettingsNavGroup { title: "CAPTURE", items: &[SettingsTab::Video, SettingsTab::Audio, SettingsTab::Hotkeys] },
    SettingsNavGroup { title: "STORAGE", items: &[SettingsTab::Storage, SettingsTab::Export] },
    SettingsNavGroup { title: "SYSTEM", items: &[SettingsTab::About] },
];

impl SettingsTab {
    pub const ALL: &[SettingsTab] = &[
        SettingsTab::General,
        SettingsTab::Startup,
        SettingsTab::Video,
        SettingsTab::Audio,
        SettingsTab::Hotkeys,
        SettingsTab::Storage,
        SettingsTab::Export,
        SettingsTab::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "Behavior",
            SettingsTab::Startup => "Startup",
            SettingsTab::Video => "Video",
            SettingsTab::Audio => "Audio",
            SettingsTab::Hotkeys => "Hotkeys",
            SettingsTab::Storage => "Storage",
            SettingsTab::Export => "Export",
            SettingsTab::About => "About",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            SettingsTab::General => "sliders-horizontal",
            SettingsTab::Startup => "power",
            SettingsTab::Video => "video",
            SettingsTab::Audio => "mic",
            SettingsTab::Hotkeys => "keyboard",
            SettingsTab::Storage => "hard-drive",
            SettingsTab::Export => "scissors",
            SettingsTab::About => "info",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            SettingsTab::General | SettingsTab::Startup => "General",
            SettingsTab::Video | SettingsTab::Audio | SettingsTab::Hotkeys => "Capture",
            SettingsTab::Storage | SettingsTab::Export => "Storage",
            SettingsTab::About => "System",
        }
    }
}

impl RekaptrWorkspace {
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
            settings_tab: SettingsTab::General,
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
            preview_vol_slider: {
                let vh = cx.entity().downgrade();
                cx.new(|cx| {
                    volume_slider::VolumeSlider::new(cx)
                        .with_value(100.0 / 150.0)
                        .on_change(move |value, _window, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                this.preview_volume = (value * 150.0) as f64;
                                if let Some(v) = &this.preview_video_source {
                                    v.set_volume(this.preview_volume);
                                }
                                cx.notify();
                            });
                        })
                })
            },
            preview_scrubbing_progress: 0.0,
            clip_popover: None,
            clip_table,
            clip_start: -1.0,
            clip_end: -1.0,
            clip_start_mark: None,
            clip_end_mark: None,
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
            add_source_search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            add_source_title_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            add_source_show_overrides: false,
            playback_volumes: vec![100.0; 10],
            track_vol_sliders: Vec::new(),
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
            form_max_buffer_size_gb: config.max_buffer_size_gb,
            clips_search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            clips_search_expanded: false,
            favorite_clips: crate::config::AppConfig::load_favorites(),
            selected_clips: std::collections::HashSet::new(),
            selected_clip_for_details: None,
            clips_filter: crate::ui::clips::ClipsFilter::All,
            hovered_clip_idx: None,
            hovered_clip_preview_progress: 0.0,
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
            dd_mic: cx.new(|cx| DropdownState::new(cx)),
            dd_export_format: cx.new(|cx| DropdownState::new(cx)),
            select_encoder: {
                let vh = cx.entity().downgrade();
                cx.new(|cx| {
                    select::AppSelect::new(cx)
                        .items(vec![
                            ("h264_nvenc", "H.264 (NVENC)"),
                            ("hevc_nvenc", "HEVC (NVENC)"),
                            ("av1_nvenc", "AV1 (NVENC)"),
                            ("x264", "H.264 (x264)"),
                        ])
                        .selected_index(match config.global_video.encoder.as_str() {
                            "h264_nvenc" => 0,
                            "hevc_nvenc" => 1,
                            "av1_nvenc" => 2,
                            "x264" => 3,
                            _ => 0,
                        })
                        .on_change(move |val, _, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                this.settings_form_encoder = val.to_string();
                                let mut c = crate::config::AppConfig::load();
                                c.global_video.encoder = val.to_string();
                                c.save();
                                cx.notify();
                            });
                        })
                })
            },
            select_resolution: {
                let vh = cx.entity().downgrade();
                cx.new(|cx| {
                    let res = &config.global_video.resolution;
                    select::AppSelect::new(cx)
                        .items(vec![
                            ("Original", "Original"),
                            ("3840x2160", "3840x2160"),
                            ("2560x1440", "2560x1440"),
                            ("1920x1080", "1920x1080"),
                            ("1280x720", "1280x720"),
                        ])
                        .selected_index(match res.as_str() {
                            "3840x2160" => 1,
                            "2560x1440" => 2,
                            "1920x1080" => 3,
                            "1280x720" => 4,
                            _ => 0,
                        })
                        .on_change(move |val, _, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                this.settings_form_resolution = val.to_string();
                                let mut c = crate::config::AppConfig::load();
                                c.global_video.resolution = val.to_string();
                                c.save();
                                cx.notify();
                            });
                        })
                })
            },
            select_fps: {
                let vh = cx.entity().downgrade();
                cx.new(|cx| {
                    let fps = config.global_video.fps;
                    select::AppSelect::new(cx)
                        .items(vec![
                            ("30", "30 FPS"),
                            ("60", "60 FPS"),
                            ("120", "120 FPS"),
                            ("144", "144 FPS"),
                            ("165", "165 FPS"),
                            ("240", "240 FPS"),
                        ])
                        .selected_index(match fps {
                            30 => 0, 60 => 1, 120 => 2, 144 => 3, 165 => 4, 240 => 5, _ => 1,
                        })
                        .on_change(move |val, _, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                if let Ok(fps) = val.parse::<i32>() {
                                    this.settings_form_fps = fps;
                                    let mut c = crate::config::AppConfig::load();
                                    c.global_video.fps = fps;
                                    c.save();
                                    cx.notify();
                                }
                            });
                        })
                })
            },
            select_preset: {
                let vh = cx.entity().downgrade();
                cx.new(|cx| {
                    let preset = &config.global_video.preset;
                    select::AppSelect::new(cx)
                        .items(vec![
                            ("p1", "P1 (Fastest)"),
                            ("p2", "P2"),
                            ("p3", "P3"),
                            ("p4", "P4 (Balanced)"),
                            ("p5", "P5"),
                            ("p6", "P6"),
                            ("p7", "P7 (Best Quality)"),
                        ])
                        .selected_index(match preset.as_str() {
                            "p1" => 0, "p2" => 1, "p3" => 2, "p4" => 3,
                            "p5" => 4, "p6" => 5, "p7" => 6, _ => 3,
                        })
                        .on_change(move |val, _, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                this.settings_form_preset = val.to_string();
                                let mut c = crate::config::AppConfig::load();
                                c.global_video.preset = val.to_string();
                                c.save();
                                cx.notify();
                            });
                        })
                })
            },
            update_state: crate::updater::UpdateState::Idle,
            update_has_receipt: crate::updater::has_install_receipt(),
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
                                    
                                    // Re-check audio mix in case track count changed between segments.
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
            self.sync_settings_form_from_config(&config);
            
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
                                let name_lower = name.to_lowercase();
                                // Ignore system/build/dependency folders to prevent massive recursive scans
                                if name != "Clips" && name != "Cache" && !name.starts_with(".") 
                                   && name_lower != "target" && name_lower != "dist" 
                                   && !name_lower.contains("gstreamer") {
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

    pub fn toggle_favorite(&mut self, clip_path: &str, cx: &mut Context<Self>) {
        let is_fav = self.favorite_clips.contains(clip_path);
        if is_fav {
            self.favorite_clips.remove(clip_path);
        } else {
            self.favorite_clips.insert(clip_path.to_string());
        }
        crate::config::AppConfig::set_favorite(clip_path, !is_fav);
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

                    // Prefetch per-game artwork for any new games.
                    let game_titles: Vec<String> = {
                        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                        for c in &this.cached_clips {
                            seen.insert(c.title.clone());
                        }
                        seen.into_iter().collect()
                    };
                    this.fetch_portrait_artwork(&game_titles, cx);

                    // Keep the table in sync if it's ever shown.
                    this.clip_table.update(cx, |table, cx| {
                        table.set_data(clips, cx);
                    });

                    cx.notify();
                });
            }
        }).detach();

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

    pub fn load_video(&mut self, source_name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let path = std::path::Path::new(source_name);
        let is_direct_file = path.exists() && path.extension().map_or(false, |ext| ext == "mkv" || ext == "mp4");

        // Always create a fresh Video instance when switching sessions.
        // Reusing an existing mpv instance with loadfile("replace") can leave
        // stale decoder state (especially with HLS + AV1), causing libdav1d
        // OBU parsing errors and falling back to software decoding.
        if let Some(old) = self.video_source.take() {
            window.drop_image(old.render_image()).ok();
        }

        if self.is_loading_video { return; }
        self.is_loading_video = true;
        let is_same_source = self.selected_source.as_deref() == Some(source_name);
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
                                if !is_same_source {
                                    this.timeline_markers.clear();
                                }
                                this.clip_start = -1.0;
                                this.clip_end = -1.0;
                                this.clip_start_mark = None;
                                this.clip_end_mark = None;
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
                self.clip_start_mark = None;
                self.clip_end_mark = None;
            } else {
                let pos = v.position().as_secs_f64();
                let stream = v.current_stream_filename();
                let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                self.clip_start = pos;
                self.clip_end = -1.0;
                self.clip_start_mark = stream
                    .as_deref()
                    .and_then(|s| crate::utils::mark_from_mpv_state(&source, s, pos));
                self.clip_end_mark = None;
                match (&self.clip_start_mark, stream.as_deref()) {
                    (Some(m), Some(s)) => log::info!(
                        "[Clip] in-point: stream={} time_pos={:.3} -> sid={:?} idx={} off={:.3}",
                        s, pos, m.session_id, m.segment_index, m.offset_in_segment
                    ),
                    (None, s) => log::warn!(
                        "[Clip] could not derive in-point mark (stream={:?}, time_pos={:.3})",
                        s, pos
                    ),
                    _ => {}
                }
            }
            cx.notify();
        }
    }

    pub fn set_clip_out(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_end >= 0.0 {
                self.clip_start = -1.0;
                self.clip_end = -1.0;
                self.clip_start_mark = None;
                self.clip_end_mark = None;
            } else {
                let pos = v.position().as_secs_f64();
                let stream = v.current_stream_filename();
                let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                self.clip_end = pos;
                self.clip_end_mark = stream
                    .as_deref()
                    .and_then(|s| crate::utils::mark_from_mpv_state(&source, s, pos));
                match (&self.clip_end_mark, stream.as_deref()) {
                    (Some(m), Some(s)) => log::info!(
                        "[Clip] out-point: stream={} time_pos={:.3} -> sid={:?} idx={} off={:.3}",
                        s, pos, m.session_id, m.segment_index, m.offset_in_segment
                    ),
                    (None, s) => log::warn!(
                        "[Clip] could not derive out-point mark (stream={:?}, time_pos={:.3})",
                        s, pos
                    ),
                    _ => {}
                }
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

    #[allow(dead_code)]
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
                self.timeline_markers.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
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
    #[allow(dead_code)]
    pub fn prune_markers_before(&mut self, min_time_secs: f64) {
        self.timeline_markers.retain(|m| m.time_secs >= min_time_secs);
    }

    pub fn ensure_track_vol_sliders(&mut self, track_count: usize, cx: &mut Context<Self>) {
        if self.track_vol_sliders.len() == track_count {
            return;
        }
        let view = cx.entity().downgrade();
        self.track_vol_sliders.clear();
        for idx in 0..track_count {
            let vh = view.clone();
            let initial_vol = self.playback_volumes.get(idx).copied().unwrap_or(100.0);
            let slider = cx.new(|cx| {
                volume_slider::VolumeSlider::new(cx)
                    .with_value((initial_vol / 150.0) as f32)
                    .on_change(move |value, _window, cx| {
                        let _ = vh.update(cx, |this, cx| {
                            let volume = (value * 150.0) as f64;
                            if this.playback_volumes.len() <= idx {
                                this.playback_volumes.resize(idx + 1, 100.0);
                            }
                            this.playback_volumes[idx] = volume;
                            let now = std::time::Instant::now();
                            if now.duration_since(this.last_volume_update_at).as_millis() > 50 {
                                this.last_volume_update_at = now;
                                this.update_mpv_audio_mix();
                            }
                            cx.notify();
                        });
                    })
            });
            self.track_vol_sliders.push(slider);
        }
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

    pub fn render_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            let clip_path_str = clip.path.to_string_lossy().to_string();
            let is_favorited = self.favorite_clips.contains(&clip_path_str);
            let fav_label = if is_favorited { "Unfavorite" } else { "Favorite" };
            let items = vec![
                PopoverMenuItem::new("favorite", fav_label)
                    .icon(if is_favorited { "star-off" } else { "star" })
                    .on_click({
                        let view = cx.entity().downgrade();
                        let path = clip_path_str.clone();
                        move |_, cx| { let _ = view.update(cx, |this, cx| {
                            this.clip_popover = None;
                            this.toggle_favorite(&path.clone(), cx);
                        }); }
                    }),
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

impl Render for RekaptrWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_workspace(window, cx)
    }
}
