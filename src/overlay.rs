//! In-game overlay.
//!
//! A Steam-style overlay the player *summons* with a hotkey (default F8): a
//! separate transparent, always-on-top window that dims the game behind it and
//! presents a large capture/replay preview plus quick actions (save replay,
//! start/stop recording, mute mic, drop markers). Press the hotkey again — or
//! Esc — to dismiss it and resume the game.
//!
//! ## Anti-cheat safety (why it's built this way)
//!
//! This is a plain top-level OS window. It **never** injects a DLL into the game
//! process, hooks the game's render pipeline (DirectX/Vulkan `Present`), reads
//! the game's memory, or synthesizes input into it — those are the techniques
//! anti-cheats (EAC, BattlEye, Vanguard, …) actually detect. Staying a separate
//! composited window keeps us clear of that entire class of detection.
//!
//! The trade-off: a separate window **cannot** draw over true exclusive
//! fullscreen (DXGI flip-exclusive); use borderless / windowed-fullscreen. For
//! the most aggressive kernel anti-cheats the overlay also stays off per-title by
//! default (see [`ANTI_CHEAT_PROCESSES`]); the user can opt in per game.
//!
//! `WDA_EXCLUDEFROMCAPTURE` keeps the overlay visible to the user but out of our
//! own screen capture, so the summoned panel is never baked into the recording.

use crate::config::{AppConfig, AudioRouting};
use crate::hotkeys::HotkeyAction;
use crate::state::{AppState, ClipMark, ExportPhase, MarkerKind};
use crate::video_player::{Video, VideoOptions};
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
use adabraka_ui::prelude::*;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Which screen the overlay's export dialog is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OvExportStage {
    Configure,
    Exporting,
    Done,
}

/// How often the event pump drains events and (while summoned) repaints the
/// elapsed-time clock.
const PUMP_INTERVAL: Duration = Duration::from_millis(120);
/// How long the "Replay saved ✓" confirmation stays lit on the Save button.
const SAVED_TTL: Duration = Duration::from_millis(3000);
/// App logo (same asset as the About page), shown in the overlay header.
const LOGO_BYTES: &[u8] = include_bytes!("../assets/logo.png");

/// Decode the logo once and reuse the `Arc<Image>` across frames (the header
/// re-renders on every pump tick while summoned).
fn logo_image() -> Arc<Image> {
    static LOGO: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    LOGO.get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Png, LOGO_BYTES.to_vec())))
        .clone()
}

/// Config-derived labels precomputed once and reused every frame. The video
/// element re-renders the whole overlay at video framerate, so doing
/// `AppConfig::load()` (a full config clone) + key formatting in `header`/
/// `footer`/`preview` *per frame* was making playback stutter. We recompute these
/// only when the config or recording source actually changes.
#[derive(Clone, Default)]
struct OverlayLabels {
    /// "<resolution> @ <fps>" for the effective (per-game) capture settings.
    res_caption: String,
    retention_min: i32,
    key_toggle: String,
    key_record: String,
    key_save: String,
    key_mic: String,
}

/// Audio tracks configured for `source`, mirroring `setup_export`'s selection so
/// the overlay dialog offers the same per-track toggles.
fn audio_tracks_for(source: &str) -> Vec<AudioRouting> {
    let config = AppConfig::load();
    if source == "monitor" {
        config.global_audio_tracks.clone()
    } else {
        config
            .game_registry
            .get(source)
            .and_then(|g| g.audio_routing.clone())
            .unwrap_or_else(|| config.global_audio_tracks.clone())
    }
}

/// Resolve the labels for `title` (the recorded source, if any). Loads the config
/// exactly once.
fn compute_labels(title: Option<&str>) -> OverlayLabels {
    let config = AppConfig::load();
    let hk = &config.hotkeys;
    // Effective video settings: a game may override the global resolution/fps.
    let mut vs = config.global_video.clone();
    if let Some(t) = title {
        if t != "monitor" {
            if let Some(o) = config.game_registry.get(t).and_then(|gs| gs.video_overrides.clone()) {
                vs = o;
            }
        }
    }
    OverlayLabels {
        res_caption: format!("{} @ {}", vs.resolution, vs.fps),
        retention_min: vs.retention_minutes,
        key_toggle: crate::hotkeys::vk_to_string(hk.toggle_overlay_vk, hk.toggle_overlay_mod),
        key_record: crate::hotkeys::vk_to_string(hk.toggle_recording_vk, hk.toggle_recording_mod),
        key_save: crate::hotkeys::vk_to_string(hk.save_clip_vk, hk.save_clip_mod),
        key_mic: crate::hotkeys::vk_to_string(hk.toggle_mic_vk, hk.toggle_mic_mod),
    }
}
/// When the live preview loads, how far back from the end of the buffer we start
/// playback, so the overlay opens on the most recent gameplay (the segments are
/// ~6s each, so the true live edge lags disk by a few seconds).
const LIVE_EDGE_TAIL_SECS: f64 = 12.0;

// ── Palette (matches mockups/overlay_mockup.rs) ─────────────────────────
const PRIMARY: u32 = 0x8B5CF6FF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const REC: u32 = 0xEF4444FF;
const AMBER: u32 = 0xF59E0BFF;
const GOOD: u32 = 0x22C55EFF;
/// Clip IN / OUT marker tints (match the main timeline's green / pink).
const IN_COLOR: u32 = 0x22C55EFF;
const OUT_COLOR: u32 = 0xF43F8EFF;
const BORDER: u32 = 0x2A2A30FF;
const SURFACE: u32 = 0x111114F2;
const SURFACE_2: u32 = 0x1A1A1FFF;
const PANEL_BORDER: u32 = 0xFFFFFF1F;

/// Process names (lowercase, with or without a trailing `.exe`) of known kernel /
/// aggressive anti-cheats. For titles whose target process matches one of these
/// the overlay defaults **off** unless the user explicitly enables it per-game.
pub const ANTI_CHEAT_PROCESSES: &[&str] = &[
    // Riot Vanguard / Valorant
    "vgc",
    "vgtray",
    "valorant",
    "valorant-win64-shipping",
    // FACEIT anti-cheat
    "faceit",
    "faceitclient",
    // EasyAntiCheat
    "easyanticheat",
    "easyanticheat_eos",
    // BattlEye
    "battleye",
    "beservice",
];

/// Returns true if `process` is a known anti-cheat where the overlay should be
/// off by default. Matching is case-insensitive and ignores a `.exe` suffix.
pub fn process_is_anti_cheat(process: &str) -> bool {
    let p = process.trim().to_ascii_lowercase();
    let p = p.strip_suffix(".exe").unwrap_or(&p);
    ANTI_CHEAT_PROCESSES.iter().any(|known| *known == p)
}

// ---------------------------------------------------------------------------
// Config types (referenced by `crate::config::AppConfig`)
// ---------------------------------------------------------------------------

/// Which monitor corner the overlay anchors to. Retained for config
/// compatibility; the summoned panel is centered, so this is currently unused
/// for layout.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for OverlayCorner {
    fn default() -> Self {
        Self::TopRight
    }
}

/// User-facing overlay configuration. Persisted as part of `AppConfig`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlaySettings {
    /// Master on/off for the overlay.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Retained for config compatibility (panel is centered).
    #[serde(default)]
    pub position: OverlayCorner,
    /// Show the recording status badge in the panel.
    #[serde(default = "default_true")]
    pub show_recording_indicator: bool,
    /// Show the "replay saved" confirmation on the Save button.
    #[serde(default = "default_true")]
    pub show_clip_confirmations: bool,
    /// Show the marker buttons in the panel.
    #[serde(default = "default_true")]
    pub show_markers: bool,
    /// Panel surface opacity (0..1).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Index into `cx.displays()`; `None` = primary monitor.
    #[serde(default)]
    pub monitor: Option<usize>,
}

fn default_true() -> bool {
    true
}
fn default_opacity() -> f32 {
    0.95
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            position: OverlayCorner::default(),
            show_recording_indicator: true,
            show_clip_confirmations: true,
            show_markers: true,
            opacity: default_opacity(),
            monitor: None,
        }
    }
}

/// Resolve whether the overlay is allowed for a given game title, applying
/// precedence: per-game override → anti-cheat allowlist (off) → global enable.
pub fn should_show_for_title(config: &AppConfig, title: &str) -> bool {
    if !config.overlay.enabled {
        return false;
    }
    if let Some(gs) = config.game_registry.get(title) {
        if let Some(explicit) = gs.overlay_enabled {
            return explicit;
        }
        if let Some(proc) = &gs.target_process {
            if process_is_anti_cheat(proc) {
                log::info!(
                    "[Overlay] '{}' uses anti-cheat process '{}'; overlay off by default",
                    title,
                    proc
                );
                return false;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Messages pushed to the overlay from the rest of the app via [`send`].
#[derive(Clone, Debug)]
pub enum OverlayEvent {
    /// A recording started (`active = true`) or stopped (`active = false`).
    /// `title` is the source name, used to resolve anti-cheat / per-game gating.
    RecordingChanged { active: bool, title: Option<String> },
    /// A clip finished exporting.
    ClipSaved { title: String },
    /// A timeline marker was dropped.
    Marker(MarkerKind),
    /// Toggle the summoned overlay on/off (the "toggle overlay" hotkey).
    ToggleManual,
    /// Settings changed in the UI — apply the new config live.
    ConfigChanged(OverlaySettings),
}

/// Fire-and-forget an overlay event. No-op if the overlay isn't running.
pub fn send(app_state: &AppState, event: OverlayEvent) {
    if let Some(tx) = app_state.overlay_tx.lock().as_ref() {
        let _ = tx.send(event);
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Root view of the overlay window — the summoned Steam-style panel.
pub struct OverlayView {
    settings: OverlaySettings,
    app_state: Arc<AppState>,
    focus_handle: FocusHandle,
    /// True while the overlay is summoned (visible & interactive).
    summoned: bool,
    /// Whether a recording is currently live.
    recording: bool,
    /// When the current recording started, for the elapsed clock.
    recording_started: Option<Instant>,
    /// Source title of the live/last recording (for gating + the caption).
    title: Option<String>,
    /// Optimistic mic-mute state (flipped when the user clicks Mute).
    mic_muted: bool,
    /// Until when the "Replay saved ✓" confirmation shows.
    saved_until: Option<Instant>,
    /// Last marker dropped, highlighted in the marker row.
    last_marker: Option<MarkerKind>,
    /// Live libmpv preview of the currently-recording source, created when the
    /// overlay is summoned during a recording and dropped when it's dismissed.
    live_video: Option<Video>,
    /// Whether we've already jumped the live preview to its live edge (mpv only
    /// reports a duration once the playlist is parsed, a frame or two after load).
    live_seek_done: bool,
    /// True while the user is dragging the scrub bar; `scrub_progress` then drives
    /// the displayed position instead of mpv's reported `time-pos`.
    is_scrubbing: bool,
    /// Drag position as a 0..1 fraction of the buffer duration.
    scrub_progress: f32,
    /// Clip IN point: seconds into the preview buffer, or `-1.0` when unset.
    clip_start: f64,
    /// Clip OUT point: seconds into the preview buffer, or `-1.0` when unset.
    clip_end: f64,
    /// Stable segment-relative mark for the IN point (used to export the trim).
    clip_start_mark: Option<ClipMark>,
    /// Stable segment-relative mark for the OUT point.
    clip_end_mark: Option<ClipMark>,
    /// Config-derived labels, cached so per-frame render does no config work.
    labels: OverlayLabels,
    // ── In-overlay export dialog ────────────────────────────────────────
    /// Whether the export dialog is showing (over the preview).
    export_open: bool,
    /// Current dialog screen.
    export_stage: OvExportStage,
    /// Re-encode (true) vs lossless stream copy (false).
    ov_reencode: bool,
    /// Encoder id when re-encoding.
    ov_encoder: String,
    /// Target bitrate (kbps) when re-encoding.
    ov_bitrate: i32,
    /// Output container extension.
    ov_container: String,
    /// Audio tracks for the source, with per-track include flags.
    ov_audio: Vec<AudioRouting>,
    /// Title text input (reuses the adabraka Input component).
    ov_title: Entity<InputState>,
    /// True once we've observed the shared export actually run, so finishing
    /// without a `ClipSaved` can be treated as a failure.
    export_saw_running: bool,
}

impl OverlayView {
    fn new(
        settings: OverlaySettings,
        app_state: Arc<AppState>,
        focus_handle: FocusHandle,
        title_input: Entity<InputState>,
    ) -> Self {
        Self {
            settings,
            app_state,
            focus_handle,
            summoned: false,
            recording: false,
            recording_started: None,
            title: None,
            mic_muted: false,
            saved_until: None,
            last_marker: None,
            live_video: None,
            live_seek_done: false,
            is_scrubbing: false,
            scrub_progress: 0.0,
            clip_start: -1.0,
            clip_end: -1.0,
            clip_start_mark: None,
            clip_end_mark: None,
            labels: compute_labels(None),
            export_open: false,
            export_stage: OvExportStage::Configure,
            ov_reencode: false,
            ov_encoder: "h264_nvenc".to_string(),
            ov_bitrate: 50000,
            ov_container: "mp4".to_string(),
            ov_audio: Vec::new(),
            ov_title: title_input,
            export_saw_running: false,
        }
    }

    /// Set / clear the clip IN point at the preview's current position. Clicking
    /// again when a range is set clears the whole range — mirrors the main
    /// timeline's `set_clip_in`.
    fn set_clip_in(&mut self) {
        let Some(v) = &self.live_video else { return };
        if self.clip_start >= 0.0 {
            self.clear_clip_range();
            return;
        }
        let Some(source) = self.title.clone() else { return };
        let pos = v.position().as_secs_f64();
        let stream = v.current_stream_filename();
        self.clip_start = pos;
        self.clip_end = -1.0;
        self.clip_start_mark = stream
            .as_deref()
            .and_then(|s| crate::utils::mark_from_mpv_state(&source, s, pos));
        self.clip_end_mark = None;
    }

    /// Set / clear the clip OUT point at the preview's current position.
    fn set_clip_out(&mut self) {
        let Some(v) = &self.live_video else { return };
        if self.clip_end >= 0.0 {
            self.clear_clip_range();
            return;
        }
        let Some(source) = self.title.clone() else { return };
        let pos = v.position().as_secs_f64();
        let stream = v.current_stream_filename();
        self.clip_end = pos;
        self.clip_end_mark = stream
            .as_deref()
            .and_then(|s| crate::utils::mark_from_mpv_state(&source, s, pos));
    }

    fn clear_clip_range(&mut self) {
        self.clip_start = -1.0;
        self.clip_end = -1.0;
        self.clip_start_mark = None;
        self.clip_end_mark = None;
    }

    /// Drop a timeline marker at the preview's current position. Writes to the
    /// shared per-source store so the marker also appears on the desktop timeline.
    fn add_marker(&mut self, kind: MarkerKind) {
        let Some(v) = &self.live_video else { return };
        let Some(source) = self.title.clone() else { return };
        let time = v.position().as_secs_f64();
        if self.app_state.add_marker(&source, time, kind) {
            self.last_marker = Some(kind);
        }
    }

    /// Open the in-overlay export dialog for the marked IN→OUT range. Requires
    /// both points. Populates the audio-track list for the source and pauses the
    /// preview while configuring.
    fn open_export_dialog(&mut self, cx: &mut Context<Self>) {
        if self.clip_start_mark.is_none() || self.clip_end_mark.is_none() {
            log::warn!("[Overlay] Export ignored — set IN and OUT points first");
            return;
        }
        let Some(source) = self.title.clone() else { return };

        self.ov_audio = audio_tracks_for(&source);
        self.export_stage = OvExportStage::Configure;
        self.export_saw_running = false;
        self.export_open = true;
        if let Some(v) = &self.live_video {
            v.set_paused(true);
        }
        let _ = cx;
    }

    /// Close the dialog and resume the live preview.
    fn close_export_dialog(&mut self) {
        self.export_open = false;
        if self.export_stage != OvExportStage::Exporting {
            if let Some(v) = &self.live_video {
                v.set_paused(false);
            }
        }
    }

    /// Hand the marked range + chosen options to the workspace, which runs the
    /// shared `perform_export` backend. Switches the dialog to its progress screen.
    fn start_overlay_export(&mut self, cx: &mut Context<Self>) {
        let (Some(in_mark), Some(out_mark)) =
            (self.clip_start_mark.clone(), self.clip_end_mark.clone())
        else {
            return;
        };
        let Some(source) = self.title.clone() else { return };

        *self.app_state.overlay_clip_request.lock() = Some(crate::state::OverlayClipRequest {
            source,
            in_mark,
            out_mark,
            reencode: self.ov_reencode,
            encoder: self.ov_encoder.clone(),
            bitrate: self.ov_bitrate,
            container: self.ov_container.clone(),
            title: self.ov_title.read(cx).content().to_string(),
            audio_enabled: self.ov_audio.iter().map(|t| t.enabled).collect(),
        });
        self.cmd(HotkeyAction::SaveClip);
        self.export_stage = OvExportStage::Exporting;
        self.export_saw_running = false;
    }

    /// Start the live libmpv preview of the currently-recording source. No-op if a
    /// preview already exists or nothing is recording. Builds the on-disk HLS
    /// playlist for the session and points mpv at the local HLS server, mirroring
    /// `RekaptrWorkspace::load_video`.
    fn start_live_preview(&mut self) {
        if self.live_video.is_some() {
            return;
        }
        let Some(title) = self.title.clone() else {
            return;
        };

        // Refresh the on-disk master playlist so it includes every segment
        // flushed so far for this session.
        if crate::utils::generate_session_playlist(&title, None).is_none() {
            log::warn!("[Overlay] No segments yet for '{}'; skipping live preview", title);
            return;
        }
        let safe_title = if title == "monitor" {
            "monitor".to_string()
        } else {
            crate::utils::clean_title(&title)
        };
        let url = format!(
            "http://127.0.0.1:{}/{}/master.m3u8?token={}",
            crate::get_hls_port(),
            safe_title,
            crate::get_hls_token()
        );
        let d3d_device_ptr = self.app_state.d3d11_device.lock().as_ref().map(|h| h.0 .0);
        match Video::new_with_options(
            &url,
            VideoOptions {
                source_name: Some(title.clone()),
                ..Default::default()
            },
            d3d_device_ptr,
        ) {
            Ok(video) => {
                log::info!("[Overlay] Live preview started for '{}'", title);
                self.live_video = Some(video);
                self.live_seek_done = false;
            }
            Err(e) => log::error!("[Overlay] Failed to start live preview: {:?}", e),
        }
    }

    /// Tear down the live preview and release its shared render image on the
    /// overlay window.
    fn stop_live_preview(&mut self, window: &mut Window) {
        if let Some(old) = self.live_video.take() {
            window.drop_image(old.render_image()).ok();
            log::info!("[Overlay] Live preview stopped");
        }
        self.live_seek_done = false;
        self.is_scrubbing = false;
        self.clear_clip_range();
    }

    /// Once mpv knows the buffer duration, jump the preview to its live edge so the
    /// overlay opens on the most recent gameplay rather than the oldest buffered
    /// frame. Runs once per preview.
    fn tick_live_seek(&mut self) {
        if self.live_seek_done {
            return;
        }
        let Some(v) = &self.live_video else {
            return;
        };
        let dur = v.duration().as_secs_f64();
        if dur > 1.0 {
            let target = (dur - LIVE_EDGE_TAIL_SECS).max(0.0);
            let _ = v.seek(Duration::from_secs_f64(target), true);
            self.live_seek_done = true;
        }
    }

    /// Send a workspace action (Save / Record / Mic / Marker) to the dispatch loop.
    fn cmd(&self, action: HotkeyAction) {
        if let Some(tx) = self.app_state.overlay_cmd_tx.lock().as_ref() {
            let _ = tx.send(action);
        }
    }

    /// Apply one event to the view state. Returns true if the summon state changed
    /// (so the pump can sync window visibility/focus).
    fn apply(&mut self, event: OverlayEvent) {
        match event {
            OverlayEvent::RecordingChanged { active, title } => {
                self.recording = active;
                if active {
                    self.recording_started = Some(Instant::now());
                    if title.is_some() {
                        self.title = title;
                        // Resolution/fps caption is per-source — refresh it now.
                        self.labels = compute_labels(self.title.as_deref());
                    }
                } else {
                    self.recording_started = None;
                }
            }
            OverlayEvent::ClipSaved { title } => {
                self.saved_until = Some(Instant::now() + SAVED_TTL);
                if !title.is_empty() {
                    self.title = Some(title);
                }
                // The in-overlay export finished successfully.
                if self.export_open {
                    self.export_stage = OvExportStage::Done;
                    self.export_saw_running = false;
                }
            }
            OverlayEvent::Marker(kind) => {
                self.last_marker = Some(kind);
            }
            OverlayEvent::ToggleManual => {
                if self.summoned {
                    self.summoned = false;
                } else {
                    // Respect master enable + per-game / anti-cheat gating for the
                    // title we're currently recording (if any).
                    let allowed = match self.title.as_deref() {
                        Some(t) => should_show_for_title(&AppConfig::load(), t),
                        None => self.settings.enabled,
                    };
                    self.summoned = allowed;
                }
            }
            OverlayEvent::ConfigChanged(settings) => {
                self.settings = settings;
                if !self.settings.enabled {
                    self.summoned = false;
                }
                // Hotkeys / video settings may have changed.
                self.labels = compute_labels(self.title.as_deref());
            }
        }
    }

    fn desired_visible(&self) -> bool {
        self.settings.enabled && self.summoned
    }

    fn saved(&self) -> bool {
        self.saved_until.map_or(false, |t| t > Instant::now())
    }

    fn elapsed_label(&self) -> String {
        let secs = self
            .recording_started
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        format!("REC {:02}:{:02}", secs / 60, secs % 60)
    }
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .id("overlay-root")
            .key_context("Overlay")
            .track_focus(&self.focus_handle)
            .size_full()
            .text_color(rgba(FG))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    // Esc backs out of the export dialog first, then dismisses.
                    if this.export_open {
                        if this.export_stage != OvExportStage::Exporting {
                            this.close_export_dialog();
                        }
                    } else {
                        this.summoned = false;
                    }
                    cx.notify();
                }
            }));

        if !self.desired_visible() {
            // Nothing drawn while dismissed (window is also hidden by the pump).
            return root;
        }

        let body = if self.export_open {
            self.export_panel(cx).into_any_element()
        } else {
            self.panel(window, cx).into_any_element()
        };

        root.child(
            div()
                .absolute()
                .inset_0()
                .bg(rgba(0x000000C2))
                .flex()
                .items_center()
                .justify_center()
                // Click the margin around the panel to dismiss (but not while a
                // dialog is open — there you must use its buttons).
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    if !this.export_open {
                        this.summoned = false;
                        cx.notify();
                    }
                }))
                .child(body),
        )
    }
}

impl OverlayView {
    fn panel(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(1000.0))
            .max_w(relative(0.95))
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .rounded_2xl()
            .shadow_xl()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Swallow clicks so they don't hit the dismiss backdrop.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.header())
            .child(
                div()
                    .px_6()
                    .py_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.preview(window, cx))
                    .child(self.action_bar(cx)),
            )
            .child(self.footer())
    }

    fn header(&self) -> impl IntoElement {
        let toggle_key = self.labels.key_toggle.clone();
        HStack::new()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(img(logo_image()).size(px(30.0)).rounded_lg())
                    .child(div().text_lg().font_weight(FontWeight::BOLD).child("Rekaptr"))
                    .child(self.recording_badge())
                    .child(self.mic_badge()),
            )
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgba(0xFFFFFF14))
                            .border_1()
                            .border_color(rgba(PANEL_BORDER))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(toggle_key),
                    )
                    .child(div().text_xs().text_color(rgba(FG_MUTED)).child("to resume game")),
            )
    }

    fn recording_badge(&self) -> impl IntoElement {
        let (dot, label, tint) = if self.recording {
            (REC, self.elapsed_label(), REC)
        } else {
            (AMBER, "IDLE".to_string(), AMBER)
        };
        HStack::new()
            .gap_2()
            .items_center()
            .px_2p5()
            .py_1()
            .rounded_full()
            .bg(rgba(0xFFFFFF0F))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .when(self.settings.show_recording_indicator, |d| {
                d.child(div().size(px(8.0)).rounded_full().bg(rgba(dot)))
                    .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgba(tint)).child(label))
            })
    }

    fn mic_badge(&self) -> impl IntoElement {
        let (icon, tint, label) = if self.mic_muted {
            ("volume-x", REC, "Mic muted")
        } else {
            ("mic", GOOD, "Mic live")
        };
        HStack::new()
            .gap_1p5()
            .items_center()
            .px_2p5()
            .py_1()
            .rounded_full()
            .bg(rgba(0xFFFFFF0F))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .child(Icon::new(IconSource::Named(icon.into())).size(px(13.0)).color(rgba(tint).into()))
            .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgba(FG_MUTED)).child(label))
    }

    fn preview(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title.clone().unwrap_or_else(|| "Desktop".to_string());
        // Labels are precomputed (see `compute_labels`) so this hot render path
        // does no config work per frame.
        let res_caption = format!("{} · {}", title, self.labels.res_caption);
        let retention = self.labels.retention_min;

        let has_live = self.live_video.is_some();
        let frame = div()
            .w_full()
            .h(px(440.0))
            .rounded_xl()
            .overflow_hidden()
            .bg(rgba(0x101826FF))
            .border_1()
            .border_color(rgba(BORDER))
            .relative()
            // Live footage of the currently-recording source (bottom layer).
            .when_some(self.live_video.as_ref(), |d, v| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui::black())
                        .child(crate::video_player::video(v.clone())),
                )
            })
            // Mockup wash + bottom gradient, only when there's no live frame.
            .when(!has_live, |d| {
                d.child(div().absolute().inset_0().bg(rgba(0x1E293BFF)).opacity(0.55))
                    .child(div().absolute().bottom_0().left_0().right_0().h(px(120.0)).bg(rgba(0x000000AA)))
            })
            .child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .left(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(0x000000AA))
                    .child(div().size(px(8.0)).rounded_full().bg(rgba(if self.recording { REC } else { AMBER })))
                    .child(
                        div().text_color(rgba(FG)).text_xs().font_weight(FontWeight::BOLD).child(
                            if self.recording { "LIVE".to_string() } else { "BUFFER".to_string() },
                        ),
                    ),
            )
            // Placeholder play glyph — only when there's no live footage.
            .when(!has_live, |d| {
                d.child(
                    div().absolute().inset_0().flex().items_center().justify_center().child(
                        div()
                            .size(px(64.0))
                            .rounded_full()
                            .bg(rgba(0x000000AA))
                            .border_1()
                            .border_color(rgba(PANEL_BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconSource::Named("play".into())).size(px(26.0)).color(rgba(FG).into())),
                    ),
                )
            });

        let caption = HStack::new()
            .items_center()
            .justify_between()
            .px_1()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!("Instant replay · last {} min", retention)))
            .child(div().text_xs().text_color(rgba(FG_MUTED)).child(res_caption));

        div()
            .flex()
            .flex_col()
            .gap_2()
            // Continue a drag even when the cursor leaves the thin scrub bar, and
            // end it on release anywhere over the preview.
            .when(has_live, |d| {
                d.on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    if this.is_scrubbing {
                        this.is_scrubbing = false;
                        cx.notify();
                    }
                }))
            })
            .child(frame)
            .child(caption)
            .when(has_live, |d| d.child(self.scrubber(window, cx)))
    }

    /// Transport + scrub bar for the live preview. Geometry mirrors the centered
    /// panel: the bar spans the content area (panel width minus the `px_6`
    /// padding), so absolute mouse-x maps to a buffer fraction.
    fn scrubber(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (pos, dur) = self
            .live_video
            .as_ref()
            .map(|v| (v.position().as_secs_f64(), v.duration().as_secs_f64().max(1.0)))
            .unwrap_or((0.0, 1.0));
        let display_pos = if self.is_scrubbing {
            self.scrub_progress as f64 * dur
        } else {
            pos
        };
        let progress = (display_pos / dur).clamp(0.0, 1.0) as f32;
        let is_paused = self.live_video.as_ref().map_or(true, |v| v.paused());

        let fmt = |s: f64| {
            let t = s.max(0.0) as u64;
            format!("{:01}:{:02}", t / 60, t % 60)
        };

        // Clip IN/OUT positions as 0..1 fractions of the buffer.
        let clip_in_prog = if self.clip_start >= 0.0 { (self.clip_start / dur).clamp(0.0, 1.0) as f32 } else { -1.0 };
        let clip_out_prog = if self.clip_end >= 0.0 { (self.clip_end / dur).clamp(0.0, 1.0) as f32 } else { -1.0 };
        let in_text = if self.clip_start >= 0.0 { fmt(self.clip_start) } else { "--:--".to_string() };
        let out_text = if self.clip_end >= 0.0 { fmt(self.clip_end) } else { "--:--".to_string() };

        // Markers for this source (shared with the desktop timeline).
        let markers = self
            .title
            .as_deref()
            .map(|t| self.app_state.markers_for(t))
            .unwrap_or_default();

        // Map an absolute mouse-x to a 0..1 fraction across the scrub bar.
        let vw = window.viewport_size().width.0;
        let panel_w = 1000.0_f32.min(vw * 0.95);
        let bar_left = (vw - panel_w) / 2.0 + 24.0; // + content px_6 padding
        let bar_width = (panel_w - 48.0).max(1.0);
        let frac_at = move |x: f32| ((x - bar_left) / bar_width).clamp(0.0, 1.0);

        let track = div()
            .id("ov-scrub")
            .w_full()
            .h(px(16.0))
            .relative()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                this.is_scrubbing = true;
                let p = frac_at(ev.position.x.0);
                this.scrub_progress = p;
                if let Some(v) = &this.live_video {
                    let _ = v.seek(Duration::from_secs_f64(p as f64 * dur), false);
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if this.is_scrubbing {
                    let p = frac_at(ev.position.x.0);
                    this.scrub_progress = p;
                    if let Some(v) = &this.live_video {
                        let _ = v.seek(Duration::from_secs_f64(p as f64 * dur), false);
                    }
                    cx.notify();
                }
            }))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(6.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(rgba(0xFFFFFF26))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(relative(progress))
                            .rounded_full()
                            .bg(rgba(PRIMARY)),
                    ),
            )
            // Selected clip range band + IN tick.
            .when(clip_in_prog >= 0.0, |d| {
                let end = if clip_out_prog >= 0.0 { clip_out_prog } else { 1.0 };
                let w = (end - clip_in_prog).max(0.0);
                d.child(div().absolute().top_0().bottom_0().left(relative(clip_in_prog)).w(relative(w)).bg(rgba(0x8B5CF63D)))
                    .child(div().absolute().top_0().bottom_0().left(relative(clip_in_prog)).w(px(2.0)).bg(rgba(IN_COLOR)))
            })
            // OUT tick.
            .when(clip_out_prog >= 0.0, |d| {
                d.child(div().absolute().top_0().bottom_0().left(relative(clip_out_prog)).w(px(2.0)).bg(rgba(OUT_COLOR)))
            })
            // Timeline markers (kill / death / flag / highlight), color-coded.
            .children(markers.iter().map(|m| {
                let prog = (m.time_secs / dur).clamp(0.0, 1.0) as f32;
                let (h, s, l, a) = m.kind.color_hsla();
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(prog))
                    .w(px(2.0))
                    .bg(hsla(h, s, l, a).opacity(0.9))
            }));

        let icon_btn = |id: &'static str, name: &'static str, size: f32| {
            div()
                .id(id)
                .size(px(size + 12.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgba(0xFFFFFF14)))
                .child(Icon::new(IconSource::Named(name.into())).size(px(size)).color(rgba(FG).into()))
        };

        let transport = HStack::new()
            .gap_2()
            .items_center()
            .child(
                icon_btn("ov-back", "rotate-ccw", 18.0).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(v) = &this.live_video {
                            let t = (v.position().as_secs_f64() - 5.0).max(0.0);
                            let _ = v.seek(Duration::from_secs_f64(t), false);
                            cx.notify();
                        }
                    }),
                ),
            )
            .child(
                div()
                    .id("ov-playpause")
                    .size(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgba(PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        if let Some(v) = &this.live_video {
                            v.set_paused(!v.paused());
                            cx.notify();
                        }
                    }))
                    .child(
                        Icon::new(IconSource::Named(if is_paused { "play" } else { "pause" }.into()))
                            .size(px(18.0))
                            .color(rgba(FG).into()),
                    ),
            )
            .child(
                icon_btn("ov-fwd", "rotate-cw", 18.0).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(v) = &this.live_video {
                            let t = (v.position().as_secs_f64() + 5.0).min(dur);
                            let _ = v.seek(Duration::from_secs_f64(t), false);
                            cx.notify();
                        }
                    }),
                ),
            );

        let live_btn = div()
            .id("ov-live")
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .bg(rgba(0xFFFFFF0F))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .hover(|s| s.bg(rgba(0xFFFFFF1F)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.is_scrubbing = false;
                if let Some(v) = &this.live_video {
                    let t = (v.duration().as_secs_f64() - 1.0).max(0.0);
                    let _ = v.seek(Duration::from_secs_f64(t), false);
                    v.set_paused(false);
                    cx.notify();
                }
            }))
            .child(div().size(px(8.0)).rounded_full().bg(rgba(REC)))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgba(REC)).child("LIVE"));

        // IN / OUT marker buttons — click to set at the current position, click
        // again to clear the range (mirrors the main timeline).
        let mark_btn = |id: &'static str, label: &'static str, value: String, tint: u32, set: bool| {
            let border = if set { tint } else { BORDER };
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .h(px(26.0))
                .rounded_md()
                .cursor_pointer()
                .bg(rgba(SURFACE_2))
                .border_1()
                .border_color(rgba(border))
                .hover(|s| s.bg(rgba(0xFFFFFF14)))
                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgba(tint)).child(label))
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgba(FG_MUTED)).child(value))
        };
        let in_btn = mark_btn("ov-in", "IN", in_text, IN_COLOR, self.clip_start >= 0.0).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.set_clip_in();
                cx.notify();
            }),
        );
        let out_btn = mark_btn("ov-out", "OUT", out_text, OUT_COLOR, self.clip_end >= 0.0).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.set_clip_out();
                cx.notify();
            }),
        );

        VStack::new().gap_1().child(track).child(
            HStack::new()
                .items_center()
                .gap_3()
                .px_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgba(FG_MUTED))
                        .child(format!("{} / {}", fmt(display_pos), fmt(dur))),
                )
                .child(div().flex_1().flex().justify_center().child(transport))
                .child(
                    HStack::new()
                        .gap_2()
                        .items_center()
                        .child(in_btn)
                        .child(out_btn)
                        .child(live_btn),
                ),
        )
    }

    fn action_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = self.settings.show_clip_confirmations && self.saved();
        let has_range = self.clip_start_mark.is_some() && self.clip_end_mark.is_some();
        let save_label = if saved {
            "Replay saved ✓"
        } else if has_range {
            "Save clip"
        } else {
            "Mark IN & OUT to save"
        };

        let left = HStack::new()
            .gap_2()
            .items_center()
            .child(
                Button::new("ov-save", save_label)
                    .icon(IconSource::Named("save".into()))
                    .variant(if saved || !has_range { ButtonVariant::Outline } else { ButtonVariant::Default })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_export_dialog(cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ov-rec", if self.recording { "Stop recording" } else { "Start recording" })
                    .icon(IconSource::Named(if self.recording { "square" } else { "circle-dot" }.into()))
                    .variant(ButtonVariant::Outline)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cmd(HotkeyAction::ToggleRecording);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ov-mic", if self.mic_muted { "Unmute mic" } else { "Mute mic" })
                    .icon(IconSource::Named(if self.mic_muted { "volume-x" } else { "mic" }.into()))
                    .variant(ButtonVariant::Outline)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cmd(HotkeyAction::ToggleMic);
                        this.mic_muted = !this.mic_muted;
                        cx.notify();
                    })),
            );

        let row = HStack::new().w_full().items_center().justify_between().gap_3().child(left);

        if self.settings.show_markers {
            let markers = HStack::new()
                .gap_2()
                .items_center()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgba(FG_MUTED)).child("MARK"))
                .child(self.marker_btn(MarkerKind::Flag, cx))
                .child(self.marker_btn(MarkerKind::Kill, cx))
                .child(self.marker_btn(MarkerKind::Death, cx))
                .child(self.marker_btn(MarkerKind::Highlight, cx));
            row.child(markers)
        } else {
            row
        }
    }

    fn marker_btn(&self, kind: MarkerKind, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.last_marker == Some(kind);
        let (h, s, l, a) = kind.color_hsla();
        let accent = hsla(h, s, l, a);
        div()
            .id(SharedString::from(format!("ov-mark-{}", kind.label())))
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { accent.opacity(0.2) } else { rgba(SURFACE_2).into() })
            .border_1()
            .border_color(if active { accent } else { rgba(BORDER).into() })
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.add_marker(kind);
                cx.notify();
            }))
            .child(
                Icon::new(IconSource::Named(kind.icon_name().into()))
                    .size(px(14.0))
                    .color(if active { accent.into() } else { rgba(FG_MUTED).into() }),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(kind.label()),
            )
    }

    fn footer(&self) -> impl IntoElement {
        let pair = |k: String, action: &'static str| {
            HStack::new()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xFFFFFF14))
                        .border_1()
                        .border_color(rgba(PANEL_BORDER))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(k),
                )
                .child(div().text_xs().text_color(rgba(FG_MUTED)).child(action))
        };
        HStack::new()
            .px_6()
            .py_3()
            .gap_5()
            .items_center()
            .border_t_1()
            .border_color(rgba(BORDER))
            .child(pair(self.labels.key_record.clone(), "Record"))
            .child(pair(self.labels.key_save.clone(), "Save replay"))
            .child(pair(self.labels.key_mic.clone(), "Mute mic"))
            .child(div().flex_1())
            .child(pair("Esc".to_string(), "Resume game"))
    }

    // ── In-overlay export dialog ────────────────────────────────────────
    fn export_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (heading, icon) = match self.export_stage {
            OvExportStage::Configure => ("Export clip", "scissors"),
            OvExportStage::Exporting => ("Exporting…", "download"),
            OvExportStage::Done => ("Clip exported", "check"),
        };
        let header = HStack::new()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded_md()
                            .bg(rgba(0x8B5CF626))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconSource::Named(icon.into())).size(px(16.0)).color(rgba(PRIMARY).into())),
                    )
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(heading)),
            )
            .when(self.export_stage != OvExportStage::Exporting, |d| {
                d.child(
                    div()
                        .id("ov-exp-close")
                        .size(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xFFFFFF14)))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.close_export_dialog();
                            cx.notify();
                        }))
                        .child(Icon::new(IconSource::Named("x".into())).size(px(16.0)).color(rgba(FG_MUTED).into())),
                )
            });

        let body = match self.export_stage {
            OvExportStage::Configure => self.export_configure(cx).into_any_element(),
            OvExportStage::Exporting => self.export_progress().into_any_element(),
            OvExportStage::Done => self.export_done(cx).into_any_element(),
        };

        div()
            .w(px(620.0))
            .max_h(relative(0.92))
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .rounded_2xl()
            .shadow_xl()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(body)
    }

    fn section_label(&self, text: &'static str) -> impl IntoElement {
        div()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(rgba(FG_MUTED))
            .child(text)
    }

    fn export_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = VStack::new()
            .p_6()
            .gap_5()
            // Title
            .child(
                VStack::new()
                    .gap_2()
                    .child(self.section_label("CLIP TITLE"))
                    .child(Input::new(&self.ov_title).placeholder("Clutch 1v4 on Mirage")),
            )
            // Mode
            .child(
                VStack::new()
                    .gap_2()
                    .child(self.section_label("MODE"))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.pill("ov-mode-copy", "Instant copy", !self.ov_reencode, cx.listener(|this, _, _, cx| { this.ov_reencode = false; cx.notify(); })))
                            .child(self.pill("ov-mode-enc", "Re-encode", self.ov_reencode, cx.listener(|this, _, _, cx| { this.ov_reencode = true; cx.notify(); }))),
                    )
                    .child(div().text_xs().text_color(rgba(FG_MUTED)).child(if self.ov_reencode {
                        "Re-encodes the clip. Choose codec, quality and container."
                    } else {
                        "Lossless stream copy. Saves in under a second."
                    })),
            );

        if self.ov_reencode {
            content = content.child(self.export_reencode_panel(cx));
        }

        // Audio tracks
        content = content.child(self.export_audio_panel(cx));

        VStack::new()
            .flex_1()
            .min_h_0()
            .child(div().id("ov-exp-scroll").flex_1().min_h_0().overflow_y_scroll().child(content))
            .child(
                HStack::new()
                    .px_6()
                    .py_4()
                    .gap_3()
                    .items_center()
                    .justify_end()
                    .border_t_1()
                    .border_color(rgba(BORDER))
                    .child(
                        Button::new("ov-exp-cancel", "Cancel")
                            .variant(ButtonVariant::Outline)
                            .on_click(cx.listener(|this, _, _, cx| { this.close_export_dialog(); cx.notify(); })),
                    )
                    .child(
                        Button::new("ov-exp-go", "Export clip")
                            .icon(IconSource::Named("download".into()))
                            .on_click(cx.listener(|this, _, _, cx| { this.start_overlay_export(cx); cx.notify(); })),
                    ),
            )
    }

    /// A selectable pill button used across the dialog. `on_click` is the output
    /// of `cx.listener(...)`.
    fn pill(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        active: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_4()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
            .bg(if active { rgba(0x8B5CF626) } else { rgba(SURFACE_2) })
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF14)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(label.into())
    }

    fn export_reencode_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_5()
            .p_5()
            .rounded_lg()
            .bg(rgba(SURFACE_2))
            .border_1()
            .border_color(rgba(BORDER))
            // Encoder
            .child(
                VStack::new()
                    .gap_2()
                    .child(self.section_label("ENCODER"))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.pill("ov-enc-hevc", "HEVC", self.ov_encoder == "hevc_nvenc", cx.listener(|this, _, _, cx| { this.ov_encoder = "hevc_nvenc".into(); cx.notify(); })))
                            .child(self.pill("ov-enc-av1", "AV1", self.ov_encoder == "av1_nvenc", cx.listener(|this, _, _, cx| { this.ov_encoder = "av1_nvenc".into(); cx.notify(); })))
                            .child(self.pill("ov-enc-h264", "H.264", self.ov_encoder == "h264_nvenc", cx.listener(|this, _, _, cx| { this.ov_encoder = "h264_nvenc".into(); cx.notify(); }))),
                    ),
            )
            // Quality
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        HStack::new()
                            .items_center()
                            .justify_between()
                            .child(self.section_label("QUALITY"))
                            .child(div().text_xs().text_color(rgba(FG_MUTED)).child(format!("{} kbps", self.ov_bitrate))),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.pill("ov-q-low", "Smaller", self.ov_bitrate == 8000, cx.listener(|this, _, _, cx| { this.ov_bitrate = 8000; cx.notify(); })))
                            .child(self.pill("ov-q-bal", "Balanced", self.ov_bitrate == 20000, cx.listener(|this, _, _, cx| { this.ov_bitrate = 20000; cx.notify(); })))
                            .child(self.pill("ov-q-max", "Max quality", self.ov_bitrate == 50000, cx.listener(|this, _, _, cx| { this.ov_bitrate = 50000; cx.notify(); }))),
                    ),
            )
            // Container
            .child(
                VStack::new()
                    .gap_2()
                    .child(self.section_label("CONTAINER"))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.pill("ov-ct-mp4", "MP4", self.ov_container == "mp4", cx.listener(|this, _, _, cx| { this.ov_container = "mp4".into(); cx.notify(); })))
                            .child(self.pill("ov-ct-mov", "MOV", self.ov_container == "mov", cx.listener(|this, _, _, cx| { this.ov_container = "mov".into(); cx.notify(); })))
                            .child(self.pill("ov-ct-mkv", "MKV", self.ov_container == "mkv", cx.listener(|this, _, _, cx| { this.ov_container = "mkv".into(); cx.notify(); }))),
                    ),
            )
    }

    fn export_audio_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut col = VStack::new().gap_2().child(self.section_label("AUDIO TRACKS"));
        if self.ov_audio.is_empty() {
            return col.child(div().text_xs().text_color(rgba(FG_MUTED)).child("No audio tracks configured for this source."));
        }
        let mut row = HStack::new().gap_2().flex_wrap();
        for (idx, track) in self.ov_audio.iter().enumerate() {
            let name = crate::ui::audio_track_display_name(track);
            row = row.child(self.pill(
                SharedString::from(format!("ov-at-{idx}")),
                name,
                track.enabled,
                cx.listener(move |this, _, _, cx| {
                    if let Some(t) = this.ov_audio.get_mut(idx) {
                        t.enabled = !t.enabled;
                    }
                    cx.notify();
                }),
            ));
        }
        col = col.child(row);
        col
    }

    fn export_progress(&self) -> impl IntoElement {
        let pct = (*self.app_state.export.progress.lock() * 100.0).clamp(0.0, 100.0);
        VStack::new()
            .p_8()
            .gap_4()
            .items_center()
            .child(div().text_sm().text_color(rgba(FG_MUTED)).child("Exporting your clip…"))
            .child(
                div()
                    .w_full()
                    .h(px(8.0))
                    .rounded_full()
                    .bg(rgba(0xFFFFFF1F))
                    .child(div().h_full().w(relative((pct / 100.0) as f32)).rounded_full().bg(rgba(PRIMARY))),
            )
            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(format!("{:.0}%", pct)))
    }

    fn export_done(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .p_8()
            .gap_4()
            .items_center()
            .child(
                div()
                    .size(px(48.0))
                    .rounded_full()
                    .bg(rgba(0x22C55E26))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconSource::Named("check".into())).size(px(24.0)).color(rgba(GOOD).into())),
            )
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Clip saved"))
            .child(div().text_xs().text_color(rgba(FG_MUTED)).child("Find it in your Clips library."))
            .child(
                Button::new("ov-exp-done", "Back to replay")
                    .on_click(cx.listener(|this, _, _, cx| { this.close_export_dialog(); cx.notify(); })),
            )
    }
}

// ---------------------------------------------------------------------------
// Window creation + event pump
// ---------------------------------------------------------------------------

/// Overlay window size. Deliberately *not* full-screen: a borderless window that
/// exactly covers a monitor makes DXGI reject the flip-model swapchain resize
/// (E_INVALIDARG), which crashes gpui's renderer. A normal panel-sized window
/// (same size class as the main window) resizes cleanly. The window is
/// transparent, so only the centered panel is visible over the game.
const WINDOW_W: f32 = 1060.0;
const WINDOW_H: f32 = 820.0;

/// Compute the overlay window bounds (a centered panel on the chosen monitor) and
/// its display id.
fn overlay_bounds(cx: &App, settings: &OverlaySettings) -> (Bounds<Pixels>, Option<DisplayId>) {
    let displays = cx.displays();
    let display = settings
        .monitor
        .and_then(|i| displays.get(i).cloned())
        .or_else(|| cx.primary_display());

    let work = display.as_ref().map(|d| d.bounds()).unwrap_or(Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1920.0), px(1080.0)),
    });
    let w = px(WINDOW_W);
    let h = px(WINDOW_H);
    let x = work.origin.x + (work.size.width - w) * 0.5;
    let y = work.origin.y + (work.size.height - h) * 0.5;
    let bounds = Bounds {
        origin: point(x, y),
        size: size(w, h),
    };
    (bounds, display.map(|d| d.id()))
}

/// Apply always-on-top + tool-window (no taskbar button) styles to the overlay
/// window. We do this manually instead of using `WindowKind::Overlay`, because
/// that kind creates a `WS_POPUP` window which breaks the DirectComposition
/// flip-model swapchain on this gpui fork (`ResizeBuffers` → E_INVALIDARG →
/// renderer double-drops its render target → crash). A `WindowKind::Normal`
/// window with these ex-styles is topmost and taskbar-less but keeps a working
/// swapchain. Safe to call while the window is hidden (no resize triggered).
#[cfg(target_os = "windows")]
fn apply_overlay_styles(window: &Window) {
    let Some(raw) = window.raw_window_handle() else { return };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_FRAMECHANGED, WS_EX_APPWINDOW,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };
    let hwnd = HWND(raw as *mut _);
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex & !WS_EX_APPWINDOW.0) | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Best-effort: bring the overlay to the foreground so it receives keyboard input
/// (Esc) when summoned. Operates only on our own HWND. Unlike gpui's
/// `activate_window`, this does not call `SetWindowPlacement`, so it avoids the
/// swapchain-resize crash path. If it fails, mouse interaction still works.
#[cfg(target_os = "windows")]
fn bring_to_foreground(window: &Window) {
    let Some(raw) = window.raw_window_handle() else { return };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    let hwnd = HWND(raw as *mut _);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Create the overlay window and start the event pump. Wires `overlay_tx` into
/// `app_state` so the rest of the app can [`send`] events.
///
/// Call once from the GPUI main-thread `run` closure, **after** the main window
/// has opened (so the other background loops' `cx.windows().first()` keeps
/// resolving to the workspace window).
pub fn spawn(cx: &mut App, app_state: Arc<AppState>) {
    let config = AppConfig::load();
    let settings = config.overlay.clone();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<OverlayEvent>();
    *app_state.overlay_tx.lock() = Some(tx);

    let (bounds, display_id) = overlay_bounds(cx, &settings);

    // NOTE: create with `show: true` so the full-screen placement is applied
    // exactly once, at creation, while the swapchain is fresh. We then hide it and
    // only ever `show`/`hide` afterwards (neither resizes the window). This avoids
    // a gpui crash: a hidden transparent full-screen window's *deferred* placement
    // (applied later via `activate`) triggers a second swapchain resize that
    // double-drops a half-released render target → null-pointer panic.
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: false,
        show: true,
        // Normal kind (not Overlay/WS_POPUP, which breaks the swapchain — see
        // apply_overlay_styles). Topmost/no-taskbar applied manually after creation.
        kind: WindowKind::Normal,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id,
        // IMPORTANT: must be Opaque. `Transparent` makes gpui call
        // SetWindowCompositionAttribute(ACCENT_ENABLE_TRANSPARENTGRADIENT), the
        // legacy DWM accent hack, which conflicts with the DirectComposition
        // flip-model swapchain and makes ResizeBuffers fail with E_INVALIDARG →
        // the renderer then double-drops its render target and the process aborts.
        // We render our own dim/surface layers instead.
        window_background: WindowBackgroundAppearance::Opaque,
        // Interactive when summoned — must receive clicks/keys.
        mouse_passthrough: false,
        ..Default::default()
    };

    let app_state_for_view = app_state.clone();
    let window = match cx.open_window(options, move |window, cx| {
        let _ = window;
        let title_input = cx.new(|cx| InputState::new(cx));
        cx.new(|cx| OverlayView::new(settings, app_state_for_view, cx.focus_handle(), title_input))
    }) {
        Ok(w) => w,
        Err(e) => {
            log::error!("[Overlay] Failed to open overlay window: {e}");
            return;
        }
    };

    // Apply topmost / tool-window styles, then hide (SW_HIDE — no resize) so we
    // start dismissed.
    let _ = window.update(cx, |_, win, _| {
        #[cfg(target_os = "windows")]
        apply_overlay_styles(win);
        win.hide_window();
    });

    log::info!("[Overlay] Window created (hidden until summoned)");

    cx.spawn(move |cx: &mut AsyncApp| {
        let cx = cx.clone();
        let mut shown = false;
        async move {
            loop {
                cx.background_executor().timer(PUMP_INTERVAL).await;

                let result = cx.update(|app| {
                    window
                        .update(app, |view, win, cx| {
                            while let Ok(event) = rx.try_recv() {
                                view.apply(event);
                            }

                            let want = view.desired_visible();
                            if want != shown {
                                shown = want;
                                if want {
                                    // show_window is SW_SHOW (no resize, no
                                    // deferred placement) — safe to call repeatedly.
                                    win.show_window();
                                    #[cfg(target_os = "windows")]
                                    bring_to_foreground(win);
                                    view.focus_handle.focus(win);
                                } else {
                                    win.hide_window();
                                }
                            }

                            // Live preview lifecycle: play the recording while the
                            // overlay is up, and release it once dismissed or the
                            // recording ends.
                            if shown && view.recording {
                                view.start_live_preview();
                                view.tick_live_seek();
                            } else if view.live_video.is_some() {
                                view.stop_live_preview(win);
                            }

                            // Track the shared export's lifecycle so the dialog can
                            // show progress and detect a failed export (finished
                            // without a ClipSaved → back to Configure).
                            if view.export_open && view.export_stage == OvExportStage::Exporting {
                                match *view.app_state.export.phase.lock() {
                                    ExportPhase::Exporting => view.export_saw_running = true,
                                    ExportPhase::Idle => {
                                        if view.export_saw_running {
                                            view.export_stage = OvExportStage::Configure;
                                            view.export_saw_running = false;
                                        }
                                    }
                                }
                            }

                            // Repaint while visible so the clock / progress tick.
                            if shown {
                                cx.notify();
                            }
                        })
                        .ok();
                });

                if result.is_err() {
                    break; // app shutting down
                }
            }
        }
    })
    .detach();
}
