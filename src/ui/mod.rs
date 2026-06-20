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
pub mod teams;
pub mod volume_slider;
mod setup_wizard;
mod shared;
mod sidebar;

pub use shared::*;

use adabraka_ui::overlays::popover_menu::{PopoverMenu, PopoverMenuItem};
use adabraka_ui::display::data_table::DataTable;

pub struct RekaptrWorkspace {
    pub active_view: ActiveView,
    pub settings_tab: SettingsTab,
    pub app_state: Arc<AppState>,
    pub video_source: Option<Video>,
    pub selected_source: Option<String>,
    /// Add/Edit-Source dialog state, grouped (see [`crate::ui::add_source::AddSourceForm`]).
    pub add_source: crate::ui::add_source::AddSourceForm,
    pub session_to_delete: Option<i32>,
    /// Clips-library view state, grouped (see [`crate::ui::clips::ClipsState`]).
    pub clips: crate::ui::clips::ClipsState,
    /// Clips-page mini-player state, grouped (see [`crate::ui::clips::ClipPreviewState`]).
    pub clip_preview: crate::ui::clips::ClipPreviewState,
    pub clip_start: f64,
    pub clip_end: f64,
    pub clip_start_mark: Option<crate::state::ClipMark>,
    pub clip_end_mark: Option<crate::state::ClipMark>,
    pub is_scrubbing: bool,
    /// Progress (0..1) shown on the preview seek bar while dragging it.
    pub scrubbing_progress: f32,
    /// Window-space bounds of the preview seek bar, captured each frame so a
    /// click/drag can be mapped to a playback position.
    pub preview_bar_bounds: Bounds<Pixels>,
    pub toast_manager: Entity<adabraka_ui::overlays::toast::ToastManager>,
    /// Clip-export dialog state, grouped (see [`crate::ui::export::ExportForm`]).
    pub export: crate::ui::export::ExportForm,
    /// Dashboard Sources-list state, grouped (see [`crate::ui::dashboard::SourcesState`]).
    pub sources: crate::ui::dashboard::SourcesState,
    /// Playback audio-mixer state, grouped (see [`crate::ui::dashboard::MixerState`]).
    pub mixer: crate::ui::dashboard::MixerState,
    pub last_notified_position: f64,
    pub is_refreshing_windows: bool,
    pub is_loading_video: bool,
    pub last_volume_update_at: std::time::Instant,
    /// Storage-usage state, grouped (see [`crate::ui::settings::StorageState`]).
    pub storage: crate::ui::settings::StorageState,
    /// Teams view state, grouped (see [`crate::ui::teams::TeamsState`]).
    pub teams: crate::ui::teams::TeamsState,
    pub recording_start_time: Option<std::time::Instant>,
    pub recording_session_id: Option<u64>,
    /// First-run setup-wizard state, grouped (see [`crate::ui::setup_wizard::SetupWizardState`]).
    pub setup: crate::ui::setup_wizard::SetupWizardState,
    /// Which hotkey slot is currently listening for a new binding (None = not editing)
    /// Slots: 0=toggle recording, 1=save clip, 2=toggle mic, 3=push-to-talk,
    /// 4=marker flag, 5=marker kill, 6=marker death, 7=marker highlight
    pub hotkey_listening: Option<usize>,
    pub hotkey_focus_handle: FocusHandle,
    /// Global-settings form state, grouped (see [`crate::ui::settings::SettingsForm`]).
    pub settings: crate::ui::settings::SettingsForm,
    pub update_state: crate::updater::UpdateState,
    pub update_has_receipt: bool,
    _quit_subscription: Option<Subscription>,
}

impl RekaptrWorkspace {
    pub fn new(app_state: Arc<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let toast_manager = cx.new(|cx| adabraka_ui::overlays::toast::ToastManager::new(cx));
        let config = AppConfig::load();

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

        let teams_signed_in = app_state.cloud_auth.is_signed_in();

        let mut workspace = Self {
            active_view: ActiveView::Dashboard,
            settings_tab: SettingsTab::General,
            app_state,
            video_source: None,
            selected_source: None,
            add_source: crate::ui::add_source::AddSourceForm::new(&config, cx),
            session_to_delete: None,
            clips: crate::ui::clips::ClipsState::new(cx),
            clip_preview: crate::ui::clips::ClipPreviewState::new(cx),
            clip_start: -1.0,
            clip_end: -1.0,
            clip_start_mark: None,
            clip_end_mark: None,
            is_scrubbing: false,
            scrubbing_progress: 0.0,
            preview_bar_bounds: Bounds::default(),
            toast_manager,
            export: crate::ui::export::ExportForm::new(cx),
            sources: crate::ui::dashboard::SourcesState::new(cx),
            mixer: crate::ui::dashboard::MixerState::new(),
            last_notified_position: 0.0,
            is_refreshing_windows: false,
            is_loading_video: false,
            last_volume_update_at: std::time::Instant::now(),
            storage: crate::ui::settings::StorageState::new(&config),
            teams: crate::ui::teams::TeamsState::new(teams_signed_in, cx),
            recording_start_time: None,
            recording_session_id: None,
            // Setup wizard
            setup: crate::ui::setup_wizard::SetupWizardState::new(&config),
            hotkey_listening: None,
            hotkey_focus_handle: cx.focus_handle(),
            settings: crate::ui::settings::SettingsForm::new(&config, cx),
            update_state: crate::updater::UpdateState::Idle,
            update_has_receipt: crate::updater::has_install_receipt(),
            _quit_subscription: None,
        };

        // Register graceful shutdown handler
        let quit_sub = cx.on_app_quit(|this, _cx| {
            log::info!("[Shutdown] Graceful shutdown initiated...");

            // 0. Stop mic monitor if active
            if let Some(pipeline) = this.settings.mic_monitor_pipeline.take() {
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
            this.clip_preview.player = None;

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

        // Re-render (and thus re-filter the Sources list) whenever the search
        // box changes — the idle poll loop above doesn't notify when no video is
        // playing, so without this typing wouldn't update the filtered rows.
        cx.observe(&workspace.sources.search_input, |_, _, cx| cx.notify()).detach();

        workspace
    }

    pub fn update_mpv_audio_mix(&mut self) {
        let Some(v) = &self.video_source else { return };

        // Get the actual number of audio tracks present in the current file/segment
        let actual_track_count = v.audio_tracks().len();

        let active_tracks = self.get_current_audio_tracks();
        let any_solo = self.mixer.solo.iter().any(|&s| s);

        // Walk enabled tracks in order. The recorded file has one audio stream
        // per enabled-at-record-time track, in that order, so the mpv aid for the
        // p-th enabled track is `p + 1`. Mute/solo are indexed the same way.
        // Older segments may have fewer streams than configured tracks, so guard
        // against referencing an aid that doesn't exist (it would crash the
        // filter chain).
        let mut aids: Vec<(usize, f64)> = Vec::new(); // (aid 1-based, volume 0..)
        let mut pos = 0usize;
        for t in active_tracks.iter() {
            if !t.enabled {
                continue;
            }
            let p = pos;
            pos += 1;
            if p >= actual_track_count {
                continue;
            }
            let muted = self.mixer.muted.get(p).copied().unwrap_or(false);
            let soloed = self.mixer.solo.get(p).copied().unwrap_or(false);
            let audible = if any_solo { soloed && !muted } else { !muted };
            if !audible {
                continue;
            }
            let vol = self.mixer.volumes.get(p).copied().unwrap_or(100.0) / 100.0;
            aids.push((p + 1, vol));
        }

        // Build the desired filter graph instead of applying it immediately.
        let complex = if aids.is_empty() {
            String::new()
        } else if aids.len() == 1 {
            let (aid, vol) = aids[0];
            format!("[aid{}]volume=volume={}[ao]", aid, vol)
        } else {
            let mut complex = String::new();
            for &(aid, vol) in &aids {
                complex.push_str(&format!("[aid{}]volume=volume={}[a{}];", aid, vol, aid));
            }
            for &(aid, _) in &aids {
                complex.push_str(&format!("[a{}]", aid));
            }
            // Normalize=0 prevents volume dropping when mixing multiple tracks
            complex.push_str(&format!("amix=inputs={}:normalize=0[ao]", aids.len()));
            complex
        };

        // Re-applying an identical `lavfi-complex` forces mpv to tear down and
        // rebuild its audio filter graph — an expensive, blocking operation that
        // the ~20Hz poll loop was triggering constantly. Skip when unchanged.
        if self.mixer.last_mix_sig.as_deref() == Some(complex.as_str()) {
            return;
        }
        let _ = v.read().mpv.set_property("aid", "no");
        let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
        self.mixer.last_mix_sig = Some(complex);
    }

    pub fn update_preview_audio_mix(&self) {
        if let Some(v) = &self.clip_preview.player {
            let enabled_ids: Vec<usize> = self.clip_preview.audio_enabled.iter()
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
        if let Some(v) = &self.clip_preview.player {
            let tracks = v.audio_tracks();
            let count = tracks.len();
            if self.clip_preview.audio_enabled.len() != count {
                self.clip_preview.audio_enabled = vec![true; count];
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

        // Load the user's teams the first time the signed-in Teams tab opens.
        if view == ActiveView::Teams
            && self.teams.signed_in
            && !self.teams.listed
            && !self.teams.busy
        {
            self.reload_teams(cx);
        }
        if view == ActiveView::Teams && self.teams.signed_in {
            self.start_presence_heartbeat(cx);
        }
        // Leaving Teams: tear down any open clip player so its audio/mpv stops.
        if view != ActiveView::Teams {
            self.teams.player = None;
            self.teams.player_title = None;
            self.teams.player_scrubbing = false;
        }

        if view == ActiveView::Clips {
            self.refresh_clips(cx);
        } else {
            // Clear metadata when not in library to save RAM
            self.clips.cached.clear();
            self.clips.table.update(cx, |table, cx| {
                table.set_data(Vec::new(), cx);
            });
        }

        if view == ActiveView::Settings {
            let config = AppConfig::load();
            self.storage.max_buffer_size_gb = config.max_buffer_size_gb;
            self.sync_settings_form_from_config(&config);
            
            if !self.storage.is_calculating {
                self.storage.is_calculating = true;
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
                            this.storage.clips_mb = clips_bytes / (1024 * 1024);
                            this.storage.sessions_mb = sessions_bytes / (1024 * 1024);
                            this.storage.is_calculating = false;
                            cx.notify();
                        });
                    }
                }).detach();
            }
        }
        
        cx.notify();
    }

    pub fn toggle_favorite(&mut self, clip_path: &str, cx: &mut Context<Self>) {
        let is_fav = self.clips.favorites.contains(clip_path);
        if is_fav {
            self.clips.favorites.remove(clip_path);
        } else {
            self.clips.favorites.insert(clip_path.to_string());
        }
        crate::config::AppConfig::set_favorite(clip_path, !is_fav);
        cx.notify();
    }

    pub fn refresh_clips(&mut self, cx: &mut Context<Self>) {
        if self.clips.is_loading { return; }
        self.clips.is_loading = true;

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let clips = cx.background_executor().spawn(async move {
                    crate::utils::fetch_all_clips()
                }).await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.clips.cached = clips.clone();
                    this.clips.is_loading = false;

                    // Keep the table in sync if it's ever shown.
                    this.clips.table.update(cx, |table, cx| {
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
                        let url = format!("http://127.0.0.1:{}/{}/master.m3u8?token={}", crate::get_hls_port(), safe_title, crate::get_hls_token());
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
                                // Fresh mpv instance — force the mix to re-apply.
                                this.mixer.last_mix_sig = None;
                                // Markers are keyed per source in shared state, so
                                // switching sources just renders the new source's set.
                                this.clip_start = -1.0;
                                this.clip_end = -1.0;
                                this.clip_start_mark = None;
                                this.clip_end_mark = None;
                                this.update_mpv_audio_mix();
                            }
                            Err(_) => {
                                this.video_source = None;
                                this.mixer.last_mix_sig = None;
                            }
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

    /// Dispatch a global hotkey / overlay command to the workspace. Shared by the
    /// system-wide hotkey listener and the in-game overlay's action buttons.
    pub fn handle_hotkey_action(
        &mut self,
        action: crate::hotkeys::HotkeyAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            crate::hotkeys::HotkeyAction::ToggleRecording => {
                self.toggle_recording(window, cx);
            }
            crate::hotkeys::HotkeyAction::SaveClip => {
                // A clip configured in the in-game overlay routes through here:
                // run the shared export backend headlessly with its chosen options.
                // The bare hotkey / desktop button opens the desktop dialog instead.
                let overlay_req = self.app_state.overlay_clip_request.lock().take();
                if let Some(req) = overlay_req {
                    self.export_from_overlay(req, window, cx);
                } else {
                    self.save_clip(window, cx);
                }
            }
            crate::hotkeys::HotkeyAction::ToggleMic => {
                match self.toggle_recording_mic_mute() {
                    Some(muted) => {
                        let status = if muted { "muted" } else { "unmuted" };
                        self.show_toast(
                            "Microphone",
                            Some(format!("Mic {}", status)),
                            adabraka_ui::overlays::toast::ToastVariant::Default,
                            window,
                            cx,
                        );
                    }
                    None => {
                        self.show_toast(
                            "Microphone",
                            Some("No active recording mic to mute"),
                            adabraka_ui::overlays::toast::ToastVariant::Warning,
                            window,
                            cx,
                        );
                    }
                }
                cx.notify();
            }
            crate::hotkeys::HotkeyAction::PushToTalk => {
                log::debug!("[Hotkeys] Push-to-talk triggered (toggle mode)");
                let _ = self.toggle_recording_mic_mute();
                cx.notify();
            }
            crate::hotkeys::HotkeyAction::MarkerFlag => {
                self.add_marker_with_kind(crate::state::MarkerKind::Flag, cx);
            }
            crate::hotkeys::HotkeyAction::MarkerKill => {
                self.add_marker_with_kind(crate::state::MarkerKind::Kill, cx);
            }
            crate::hotkeys::HotkeyAction::MarkerDeath => {
                self.add_marker_with_kind(crate::state::MarkerKind::Death, cx);
            }
            crate::hotkeys::HotkeyAction::MarkerHighlight => {
                self.add_marker_with_kind(crate::state::MarkerKind::Highlight, cx);
            }
            crate::hotkeys::HotkeyAction::ToggleOverlay => {
                crate::overlay::send(&self.app_state, crate::overlay::OverlayEvent::ToggleManual);
            }
        }
    }

    pub fn add_marker_with_kind(&mut self, kind: crate::state::MarkerKind, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            let time = v.position().as_secs_f64();
            let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
            if self.app_state.add_marker(&source, time, kind) {
                crate::overlay::send(&self.app_state, crate::overlay::OverlayEvent::Marker(kind));
                cx.notify();
            }
        }
    }

    pub fn remove_marker(&mut self, index: usize, cx: &mut Context<Self>) {
        let source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
        self.app_state.remove_marker(&source, index);
        cx.notify();
    }

    pub fn ensure_track_vol_sliders(&mut self, track_count: usize, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();

        // Master volume bar (created once) — drives mpv's overall `volume`.
        if self.mixer.master_slider.is_none() {
            let vh = view.clone();
            let master = cx.new(|cx| {
                volume_slider::VolumeSlider::new(cx)
                    .compact()
                    .with_value(1.0)
                    .on_change(move |value, _window, cx| {
                        let _ = vh.update(cx, |this, _| {
                            if let Some(v) = &this.video_source {
                                let _ = v.read().mpv.set_property("volume", value as f64 * 100.0);
                            }
                        });
                    })
            });
            self.mixer.master_slider = Some(master);
        }

        if self.mixer.sliders.len() == track_count {
            return;
        }
        self.mixer.muted = vec![false; track_count];
        self.mixer.solo = vec![false; track_count];
        self.mixer.sliders.clear();
        for idx in 0..track_count {
            let vh = view.clone();
            let initial_vol = self.mixer.volumes.get(idx).copied().unwrap_or(100.0);
            let slider = cx.new(|cx| {
                volume_slider::VolumeSlider::new(cx)
                    .compact()
                    .fill_color(track_color(idx))
                    .with_value((initial_vol / 150.0) as f32)
                    .on_change(move |value, _window, cx| {
                        let _ = vh.update(cx, |this, cx| {
                            let volume = (value * 150.0) as f64;
                            if this.mixer.volumes.len() <= idx {
                                this.mixer.volumes.resize(idx + 1, 100.0);
                            }
                            this.mixer.volumes[idx] = volume;
                            let now = std::time::Instant::now();
                            if now.duration_since(this.last_volume_update_at).as_millis() > 50 {
                                this.last_volume_update_at = now;
                                this.update_mpv_audio_mix();
                            }
                            cx.notify();
                        });
                    })
            });
            self.mixer.sliders.push(slider);
        }
    }

    /// Toggle mute for an enabled mixer track (by its enabled-order index) and
    /// re-apply the playback mix.
    pub fn toggle_mixer_mute(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.mixer.muted.len() {
            self.mixer.muted.resize(idx + 1, false);
        }
        self.mixer.muted[idx] = !self.mixer.muted[idx];
        self.mixer.last_mix_sig = None; // force re-apply
        self.update_mpv_audio_mix();
        cx.notify();
    }

    /// Toggle solo for an enabled mixer track (by its enabled-order index) and
    /// re-apply the playback mix.
    pub fn toggle_mixer_solo(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.mixer.solo.len() {
            self.mixer.solo.resize(idx + 1, false);
        }
        self.mixer.solo[idx] = !self.mixer.solo[idx];
        self.mixer.last_mix_sig = None; // force re-apply
        self.update_mpv_audio_mix();
        cx.notify();
    }

    pub fn delete_session(&mut self, session_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.app_state.manual_sessions.get(&session_id).map(|s| s.title.clone());
        if let Some(title) = title {
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
        self.clips.to_delete = None;
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
                        ActiveView::Teams => self.render_teams(window, cx).into_any_element(),
                    })
            );

        if self.setup.open {
            root = root.child(self.render_setup_wizard(window, cx));
        }

        if self.add_source.modal_open {
            root = root.child(self.render_add_source_modal(window, cx));
        }

        if let Some(source) = &self.add_source.advanced_source {
            root = root.child(self.render_advanced_settings_dialog(&source, window, cx));
        }

        if self.export.modal_open {
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

        if let Some(clip) = self.clips.to_delete.clone() {
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
                                                        move |_, _, cx| { let _ = view.update(cx, |this, cx| { this.clips.to_delete = None; cx.notify(); }); }
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
        if let Some((pos, clip)) = self.clips.popover.clone() {
            let clip_path_str = clip.path.to_string_lossy().to_string();
            let is_favorited = self.clips.favorites.contains(&clip_path_str);
            let fav_label = if is_favorited { "Unfavorite" } else { "Favorite" };
            let items = vec![
                PopoverMenuItem::new("favorite", fav_label)
                    .icon(if is_favorited { "star-off" } else { "star" })
                    .on_click({
                        let view = cx.entity().downgrade();
                        let path = clip_path_str.clone();
                        move |_, cx| { let _ = view.update(cx, |this, cx| {
                            this.clips.popover = None;
                            this.toggle_favorite(&path.clone(), cx);
                        }); }
                    }),
                PopoverMenuItem::new("play", "Play Clip")
                    .icon("play")
                    .on_click({
                        let view = cx.entity().downgrade();
                        let clip = clip.clone();
                        move |window, cx| { let _ = view.update(cx, |this, cx| {
                            this.clips.popover = None;
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
                            this.clips.popover = None;
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
                            this.clips.popover = None;
                            this.clips.to_delete = Some(clip.clone());
                            cx.notify();
                        }); }
                    }),
            ];

            root = root.child(
                PopoverMenu::new(pos, items)
                    .on_close({
                        let view = cx.entity().downgrade();
                        move |_, cx| { let _ = view.update(cx, |this, cx| { this.clips.popover = None; cx.notify(); }); }
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
