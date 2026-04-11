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

#[derive(Clone)]
pub struct SessionInfo {
    pub game_title: String,
    pub path: std::path::PathBuf,
    pub date: String,
    pub timestamp: u64,
    pub segment_count: usize,
    pub total_duration_secs: f64,
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
}

#[derive(Clone, Debug)]
pub struct TimelineMarker {
    /// Absolute time in the video (seconds from start of playlist)
    pub time_secs: f64,
    /// What kind of event this marker represents
    pub kind: MarkerKind,
    /// Optional user label
    pub label: Option<String>,
}

#[derive(Clone)]
pub struct Clip {
    pub title: String,
    pub path: std::path::PathBuf,
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
}

pub struct AppState {
    pub recording: RecordingState,
    pub export: ExportState,
    pub manual_sessions: Arc<DashMap<i32, GameSession>>,
    pub available_windows: Mutex<Vec<WindowInfo>>,
    pub game_registry: Arc<DashMap<String, GameSettings>>,
    pub mic_provider: Arc<Mutex<Option<Arc<MicProvider>>>>,
    pub artwork_cache: Arc<DashMap<String, Option<String>>>,
    /// Portrait artwork cache for clips page (library_600x900.jpg)
    pub portrait_cache: Arc<DashMap<String, Option<String>>>,
    /// Game logo cache (logo.png — transparent)
    pub logo_cache: Arc<DashMap<String, Option<String>>>,
    pub d3d11_device: Arc<Mutex<Option<SendHandle>>>,
    pub virtual_audio_routers: Mutex<Vec<crate::virtual_audio_router::VirtualAudioRouter>>,
    /// Cached list of audio output devices (for system/loopback capture): (id, friendly_name)
    pub audio_output_devices: Mutex<Vec<(String, String)>>,
    /// Cached list of audio input devices (microphones): (id, friendly_name)
    pub audio_input_devices: Mutex<Vec<(String, String)>>,
    pub tray_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<TrayCommand>>>,
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
            portrait_cache: Arc::new(DashMap::new()),
            logo_cache: Arc::new(DashMap::new()),
            d3d11_device: Arc::new(Mutex::new(None)),
            virtual_audio_routers: Mutex::new(Vec::new()),
            audio_output_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
            audio_input_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
            tray_tx: Mutex::new(None),
        }
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
        if self.portrait_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.portrait_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.portrait_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.portrait_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} portrait cache entries", excess);
        }
    }
}
