use crate::audio::MicProvider;
use crate::config::GameSettings;
use crate::game_detector::WindowInfo;
use crate::video_player::SendHandle;
use dashmap::DashMap;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

#[derive(Clone)]
#[allow(dead_code)]
pub struct GameSession {
    pub id: i32,
    pub title: String,
    pub auto_record: bool,
    pub retention: i32,
    pub bitrate: i32,
    pub cq: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionBlock {
    pub start_timestamp: u64,
    pub duration_secs: f64,
    pub timeline_offset_secs: f64,
    pub playlist_path: std::path::PathBuf,
}

/// The kind of event a timeline marker represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerKind {
    /// Generic bookmark / flag
    Flag,
    /// Kill / elimination (crosshair icon)
    Kill,
    /// Death (skull icon)
    Death,
    /// Highlight / cool moment (star icon)
    Highlight,
}

impl MarkerKind {
    pub const ALL: &'static [MarkerKind] = &[
        MarkerKind::Flag,
        MarkerKind::Kill,
        MarkerKind::Death,
        MarkerKind::Highlight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flag => "Flag",
            Self::Kill => "Kill",
            Self::Death => "Death",
            Self::Highlight => "Highlight",
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Kill => "crosshair",
            Self::Death => "skull",
            Self::Highlight => "star",
        }
    }

    /// Canonical marker color as `(hue 0..1, saturation, lightness, alpha)`.
    /// Shared by the marker toolbar buttons and the timeline so they always
    /// agree. Build a color with `gpui::hsla(h, s, l, a)`.
    pub fn color_hsla(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Flag => (45.0 / 360.0, 0.9, 0.55, 1.0),
            Self::Kill => (0.0, 0.85, 0.55, 1.0),
            Self::Death => (270.0 / 360.0, 0.6, 0.55, 1.0),
            Self::Highlight => (50.0 / 360.0, 1.0, 0.55, 1.0),
        }
    }
}

/// Stable reference to a moment in a recording. Survives buffer eviction
/// and playlist regeneration as long as the underlying segment file is
/// still on disk.
#[derive(Clone, Debug)]
pub struct ClipMark {
    pub session_id: Option<u64>,
    pub segment_index: u64,
    pub offset_in_segment: f64,
}

/// A clip range marked in the in-game overlay plus the export options chosen in
/// the overlay's own dialog. Handed to the workspace so the actual export runs
/// through the shared `perform_export` backend — no duplicated ffmpeg logic.
#[derive(Clone, Debug)]
pub struct OverlayClipRequest {
    pub source: String,
    pub in_mark: ClipMark,
    pub out_mark: ClipMark,
    /// Re-encode (true) vs lossless stream copy (false).
    pub reencode: bool,
    /// Encoder id (e.g. "h264_nvenc"); used only when `reencode`.
    pub encoder: String,
    /// Target bitrate in kbps; used only when `reencode`.
    pub bitrate: i32,
    /// Output container extension ("mp4" / "mov" / "mkv").
    pub container: String,
    /// Clip title (empty → auto-generated filename).
    pub title: String,
    /// Per-track include flags, aligned with the source's audio tracks in order.
    pub audio_enabled: Vec<bool>,
}

#[derive(Clone)]
pub struct TimelineMarker {
    /// Absolute time in the video (seconds from start of playlist)
    pub time_secs: f64,
    /// What kind of event this marker represents
    pub kind: MarkerKind,
}

#[derive(Clone)]
pub struct Clip {
    pub title: String,
    pub path: std::path::PathBuf,
    /// `path` rendered to a lossy UTF-8 string once at construction. Favorite/
    /// selection sets are keyed off this, and the clips view compares against it
    /// every render — precomputing it avoids re-allocating per clip per frame.
    pub path_str: String,
    pub thumbnail_path: Option<std::path::PathBuf>,
    pub date: String,
    pub duration: String,
    pub size: String,
    pub timestamp: u64,
}

/// Recording lifecycle state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingPhase {
    /// No recording active
    Idle,
    /// Pipeline is being constructed
    Starting,
    /// Actively recording
    Recording,
    /// EOS sent, waiting for pipeline teardown
    Stopping,
}

impl RecordingPhase {
    pub fn is_recording(self) -> bool {
        matches!(self, Self::Recording)
    }

    pub fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

/// Export lifecycle state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPhase {
    Idle,
    Exporting,
}

/// Which screen the export dialog is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportStage {
    Configure,
    Exporting,
    Done,
}

/// Live recording performance metrics, updated atomically from the pipeline monitor.
pub struct RecordingStats {
    /// Encoded frames per second (measured over last segment)
    pub fps: Mutex<f64>,
    /// Number of dropped frames since recording started
    pub dropped_frames: AtomicU64,
    /// Current encoder bitrate in kbps (measured from last segment)
    pub bitrate_kbps: Mutex<f64>,
    /// Disk write rate in MB/s (measured from last segment)
    pub disk_write_mbps: Mutex<f64>,
    /// Total segments written this session
    pub segments_written: AtomicU64,
    /// Size of last segment in bytes
    pub last_segment_bytes: AtomicU64,
}

/// State related to the active recording pipeline.
pub struct RecordingState {
    pub pipeline: Mutex<Option<gst::Pipeline>>,
    pub phase: Arc<Mutex<RecordingPhase>>,
    pub current_recording_duration: Mutex<f64>,
    pub current_session_blocks: Mutex<Vec<SessionBlock>>,
    pub rec_stats: RecordingStats,
    /// Mic-provider subscriber keys inserted for the active recording session.
    /// Cleared on stop so the mic thread doesn't keep pushing to dead AppSrcs.
    pub mic_subscriber_keys: Mutex<Vec<u64>>,
    /// True while the background teardown thread is still finalizing files for the
    /// previous session. Blocks a new recording from starting until splitmuxsink has
    /// flushed and fixup_eos_segments has run.
    pub teardown_in_progress: std::sync::atomic::AtomicBool,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            pipeline: Mutex::new(None),
            phase: Arc::new(Mutex::new(RecordingPhase::Idle)),
            current_recording_duration: Mutex::new(0.0),
            current_session_blocks: Mutex::new(Vec::new()),
            rec_stats: RecordingStats {
                fps: Mutex::new(0.0),
                dropped_frames: AtomicU64::new(0),
                bitrate_kbps: Mutex::new(0.0),
                disk_write_mbps: Mutex::new(0.0),
                segments_written: AtomicU64::new(0),
                last_segment_bytes: AtomicU64::new(0),
            },
            mic_subscriber_keys: Mutex::new(Vec::new()),
            teardown_in_progress: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Reset recording stats when a new recording starts.
    pub fn reset_stats(&self) {
        *self.rec_stats.fps.lock() = 0.0;
        self.rec_stats.dropped_frames.store(0, std::sync::atomic::Ordering::Relaxed);
        *self.rec_stats.bitrate_kbps.lock() = 0.0;
        *self.rec_stats.disk_write_mbps.lock() = 0.0;
        self.rec_stats.segments_written.store(0, std::sync::atomic::Ordering::Relaxed);
        self.rec_stats.last_segment_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

/// State related to clip export.
pub struct ExportState {
    pub phase: Arc<Mutex<ExportPhase>>,
    pub progress: Arc<Mutex<f32>>,
}

impl ExportState {
    pub fn new() -> Self {
        Self {
            phase: Arc::new(Mutex::new(ExportPhase::Idle)),
            progress: Arc::new(Mutex::new(0.0)),
        }
    }
}

/// Maximum entries per cache before eviction kicks in
const CACHE_MAX_ENTRIES: usize = 100;

pub enum TrayCommand {
    SetStopEnabled(bool),
    SetRecording(bool),
}

pub struct AppState {
    pub recording: RecordingState,
    pub export: ExportState,
    pub manual_sessions: Arc<DashMap<i32, GameSession>>,
    pub available_windows: Mutex<Vec<WindowInfo>>,
    pub game_registry: Arc<DashMap<String, GameSettings>>,
    pub mic_provider: Arc<Mutex<Option<Arc<MicProvider>>>>,
    pub artwork_cache: Arc<DashMap<String, Option<String>>>,
    /// Square game-icon cache for the sources list (local Steam librarycache
    /// path, or `None` while resolving / when unavailable).
    pub icon_cache: Arc<DashMap<String, Option<String>>>,
    /// Game portrait-cover cache (library_600x900 — clips-page folder posters)
    pub cover_cache: Arc<DashMap<String, Option<String>>>,
    /// Per-source on-disk stats (size / last activity / buffered duration) for
    /// the sources-list columns, computed on a background thread.
    pub source_stats: Arc<DashMap<String, crate::utils::SourceStats>>,
    pub d3d11_device: Arc<Mutex<Option<SendHandle>>>,
    pub virtual_audio_routers: Mutex<Vec<crate::virtual_audio_router::VirtualAudioRouter>>,
    /// Cached list of audio output devices (for system/loopback capture): (id, friendly_name)
    pub audio_output_devices: Mutex<Vec<(String, String)>>,
    /// Cached list of audio input devices (microphones): (id, friendly_name)
    pub audio_input_devices: Mutex<Vec<(String, String)>>,
    pub tray_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<TrayCommand>>>,
    /// Channel to the in-game overlay event pump (`src/overlay.rs`). `None` until
    /// the overlay window is created at startup.
    pub overlay_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<crate::overlay::OverlayEvent>>>,
    /// Channel the in-game overlay's buttons use to drive workspace actions
    /// (Save / Record / Mic / Markers), routed through the hotkey dispatch.
    pub overlay_cmd_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<crate::hotkeys::HotkeyAction>>>,
    /// Cloud account/session for the Teams feature (Clerk OAuth). The one
    /// networked surface; everything else is local. See `src/cloud`.
    pub cloud_auth: Arc<crate::cloud::CloudAuth>,
    /// Timeline markers keyed by source name, shared between the desktop timeline
    /// and the in-game overlay so markers added in either surface appear in both.
    /// Time is absolute seconds from the start of that source's playlist.
    pub timeline_markers: Arc<DashMap<String, Vec<TimelineMarker>>>,
    /// A clip range marked in the overlay, pending pickup by the workspace's
    /// `SaveClip` handler so the overlay opens the same export dialog as the desktop.
    pub overlay_clip_request: Mutex<Option<OverlayClipRequest>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            recording: RecordingState::new(),
            export: ExportState::new(),
            manual_sessions: Arc::new(DashMap::new()),
            available_windows: Mutex::new(Vec::new()),
            game_registry: Arc::new(DashMap::new()),
            mic_provider: Arc::new(Mutex::new(None)),
            artwork_cache: Arc::new(DashMap::new()),
            icon_cache: Arc::new(DashMap::new()),
            cover_cache: Arc::new(DashMap::new()),
            source_stats: Arc::new(DashMap::new()),
            d3d11_device: Arc::new(Mutex::new(None)),
            virtual_audio_routers: Mutex::new(Vec::new()),
            audio_output_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
            audio_input_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
            tray_tx: Mutex::new(None),
            overlay_tx: Mutex::new(None),
            overlay_cmd_tx: Mutex::new(None),
            cloud_auth: Arc::new(crate::cloud::CloudAuth::new()),
            timeline_markers: Arc::new(DashMap::new()),
            overlay_clip_request: Mutex::new(None),
        }
    }

    /// Add a timeline marker for `source` at `time_secs`, de-duplicating markers
    /// within 0.5s. Returns true if a marker was added. Shared by the desktop
    /// timeline and the overlay.
    pub fn add_marker(&self, source: &str, time_secs: f64, kind: MarkerKind) -> bool {
        let mut list = self.timeline_markers.entry(source.to_string()).or_default();
        if list.iter().any(|m| (m.time_secs - time_secs).abs() < 0.5) {
            return false;
        }
        list.push(TimelineMarker { time_secs, kind });
        list.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
        true
    }

    /// Remove the marker at `index` (into the sorted list) for `source`.
    pub fn remove_marker(&self, source: &str, index: usize) {
        if let Some(mut list) = self.timeline_markers.get_mut(source) {
            if index < list.len() {
                list.remove(index);
            }
        }
    }

    /// Snapshot of the markers for `source`, sorted by time.
    pub fn markers_for(&self, source: &str) -> Vec<TimelineMarker> {
        self.timeline_markers
            .get(source)
            .map(|l| l.clone())
            .unwrap_or_default()
    }

    /// Evict oldest entries from caches when they exceed the size limit.
    /// Call this periodically (e.g., when games change or on a timer).
    pub fn evict_caches(&self) {
        if self.artwork_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.artwork_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.artwork_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.artwork_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} artwork cache entries", excess);
        }
        if self.cover_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.cover_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.cover_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.cover_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} cover cache entries", excess);
        }
    }
}
