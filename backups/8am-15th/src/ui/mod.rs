//! Root UI module for the Luma workspace.
//!
//! Luma's UI is built on GPUI, which uses an immediate-mode rendering model where a single
//! root entity owns all view state. This is why [`LumaWorkspace`] has 100+ fields — GPUI has
//! no Redux-style stores or separate state containers. Every piece of UI state (form fields,
//! modal visibility, playback position, timeline scrub state, etc.) must live on this one struct
//! because GPUI re-renders the entire view tree from it on each frame.
//!
//! The workspace is organized into sub-modules that each implement methods on `LumaWorkspace`:
//! - `recording` — GStreamer pipeline lifecycle (start/stop/bus monitoring)
//! - `playback` — mpv-based video playback with multi-track audio mixing
//! - `export` — FFmpeg clip extraction (instant copy or re-encode)
//! - `library` — Clip library with search, filtering, and async loading
//! - `timeline` — Scrubbing, zoom, and in/out marker logic
//! - `dashboard` — Main recording dashboard view
//! - `sidebar` — Navigation sidebar
//! - `add_source` — Game/window source configuration modal
//! - `clips` — Clip grid/table rendering
//! - `settings` — Global settings UI

use crate::config::{AppConfig, AudioRouting};
use crate::state::AppState;
use crate::video_player::Video;
use adabraka_ui::prelude::*;
use gpui::*;
use std::sync::Arc;

mod add_source;
pub mod clips;
mod dashboard;
mod export;
mod library;
mod playback;
mod recording;
mod settings;
mod sidebar;
mod timeline;

use adabraka_ui::overlays::popover_menu::{PopoverMenu, PopoverMenuItem};
use adabraka_ui::display::data_table::DataTable;

/// The root GPUI view for Luma's entire UI.
///
/// GPUI requires a single entity to own all state — there are no component-local state hooks
/// or external stores. This struct therefore consolidates everything: navigation state, video
/// playback handles, form fields for the add-source modal, export settings, timeline scrub
/// state, clip library cache, and more.
///
/// Fields are grouped by concern:
/// - **Navigation**: `active_view`, `settings_tab_index`, `clips_view_mode`
/// - **Video playback**: `video_source`, `preview_video_source`, `playback_volumes`
/// - **Timeline**: `clip_start`/`clip_end`, `is_scrubbing`, `timeline_zoom`/`timeline_scroll`
/// - **Recording**: `recording_start_time`, `recording_session_id`
/// - **Add Source form**: all `form_*` fields (transient form state for the modal)
/// - **Export**: `export_reencode`, `export_encoder`, `export_bitrate`, etc.
/// - **Library**: `cached_clips`, `library_items`, `selected_game_filter`
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
    pub form_rate_control: i32,
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
    pub volume_slider_bounds: Bounds<Pixels>,
    pub playback_volumes: Vec<f64>,
    pub popover_fixed_top: f32,
    pub last_notified_position: f64,
    pub timeline_zoom: f32,
    pub timeline_scroll: f32,
    pub is_refreshing_windows: bool,
    pub is_loading_video: bool,
    pub is_adjusting_volume: bool,
    pub storage_clips_mb: u64,
    pub storage_sessions_mb: u64,
    pub is_calculating_storage: bool,
    pub is_loading_clips: bool,
    pub cached_clips: Vec<crate::state::Clip>,
    pub library_items: Vec<crate::ui::clips::LibraryRow>,
    pub form_max_buffer_size_gb: i32,
    pub clips_search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub storage_path_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub selected_clips: std::collections::HashSet<String>,
    pub selected_clip_for_details: Option<crate::state::Clip>,
    pub selected_game_filter: Option<String>,
    pub hovered_clip_idx: Option<usize>,
    pub hovered_clip_preview_progress: f32,
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
    /// Creates the workspace and kicks off the adaptive refresh loop.
    ///
    /// The refresh loop runs continuously to track video playback position for the timeline.
    /// It uses an adaptive tick rate: 32ms (~30fps) while video is playing or scrubbing
    /// (for smooth playhead updates), and 100ms when idle (to avoid wasting CPU).
    /// Only triggers a GPUI re-render (`cx.notify()`) when the position has actually changed
    /// by more than 50ms, preventing unnecessary repaints.
    pub fn new(app_state: Arc<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let toast_manager = cx.new(|cx| adabraka_ui::overlays::toast::ToastManager::new(cx));
        let config = AppConfig::load();

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
            volume_slider_bounds: Bounds::default(),
            playback_volumes: vec![100.0; 10],
            popover_fixed_top: 0.0,
            last_notified_position: 0.0,
            timeline_zoom: 1.0,
            timeline_scroll: 0.0,
            is_refreshing_windows: false,
            is_loading_video: false,
            is_adjusting_volume: false,
            storage_clips_mb: 0,
            storage_sessions_mb: 0,
            is_calculating_storage: false,
            is_loading_clips: false,
            cached_clips: Vec::new(),
            library_items: Vec::new(),
            form_max_buffer_size_gb: config.max_buffer_size_gb,
            clips_search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            storage_path_input: cx.new(|cx| {
                let mut state = adabraka_ui::components::input_state::InputState::new(cx);
                state.content = crate::utils::get_storage_root().to_string_lossy().to_string().into();
                state
            }),
            selected_clips: std::collections::HashSet::new(),
            selected_clip_for_details: None,
            selected_game_filter: None,
            hovered_clip_idx: None,
            hovered_clip_preview_progress: 0.0,
            volume_slider_state: cx.new(|cx| adabraka_ui::components::slider::SliderState::new(cx)),
            recording_start_time: None,
            recording_session_id: None,
        };

        // Save window bounds on move/resize (debounced — only saves after 1s of no changes)
        let _bounds_sub = cx.observe_window_bounds(window, |_this, window, cx| {
            let bounds = window.bounds();
            let wb = crate::config::WindowBounds {
                x: bounds.origin.x.0 as f64,
                y: bounds.origin.y.0 as f64,
                width: bounds.size.width.0 as f64,
                height: bounds.size.height.0 as f64,
            };
            // Debounce: spawn a background save so we don't hit SQLite on every pixel of drag
            cx.spawn(|_, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                    let mut config = crate::config::AppConfig::load();
                    config.window_bounds = Some(wb);
                    config.save();
                }
            }).detach();
        });

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
                                }
                            }
                        }
                    });

                    if should_notify {
                        this.update(&mut cx, |_, cx| cx.notify()).ok();
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(32))
                            .await;
                    } else {
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

        if view == ActiveView::Clips {
            self.refresh_clips(cx);
        } else {
            self.cached_clips.clear();
            self.clip_table.update(cx, |table, cx| {
                table.set_data(Vec::new(), cx);
            });
        }

        if view == ActiveView::Settings {
            self.settings_tab_index = 0;
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

    /// Renders the full workspace layout: sidebar + active view + modal overlays.
    ///
    /// Modals (add source, export, deletion confirmations) are layered on top of the main
    /// content using absolute positioning with a semi-transparent backdrop. This is the
    /// standard GPUI pattern for overlays since there's no portal/z-index system — modals
    /// are simply appended as children of the root div and positioned over everything else.
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
                div()
                    .absolute()
                    .inset_0()
                    .bg(gpui::rgba(0x000000_cc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
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
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
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
