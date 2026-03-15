//! Central application state shared across the UI thread, recording engine, and
//! background workers.
//!
//! `AppState` is wrapped in `Arc` and handed to every subsystem. Fields that need
//! concurrent mutation use `DashMap` (lock-free concurrent hashmap) rather than
//! `Mutex<HashMap>` because the UI thread reads these maps on every frame for
//! rendering, and blocking the UI on a recording thread's lock causes visible jank.
//! `parking_lot::Mutex` is used for fields where contention is low or where we need
//! exclusive access semantics (e.g., the GStreamer pipeline handle).

use crate::audio::MicProvider;
use crate::config::GameSettings;
use crate::game_detector::WindowInfo;
use crate::video_player::SendHandle;
use dashmap::DashMap;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::Arc;

pub struct Toast {
    pub id: i32,
    pub message: String,
    pub kind: String,
    pub time: i32,
}

#[derive(Clone)]
pub struct GameSession {
    pub id: i32,
    pub title: String,
    pub auto_record: bool,
    pub retention: i32,
    pub bitrate: i32,
    pub cq: i32,
}

/// Represents a contiguous recording session on the playback timeline.
/// Multiple `SessionBlock`s are stitched together when a game has been recorded
/// across multiple start/stop cycles. `timeline_offset_secs` positions this block
/// on the unified timeline so the player can seek across session boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionBlock {
    pub start_timestamp: u64,
    pub duration_secs: f64,
    pub timeline_offset_secs: f64,
    pub playlist_path: std::path::PathBuf,
}

#[derive(Clone)]
pub struct ToastManager {
    pub toasts: Arc<Mutex<Vec<Toast>>>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn show(&self, message: &str, kind: &str) {
        let mut t = self.toasts.lock();
        let id = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 100000) as i32;
        
        t.push(Toast {
            id,
            message: message.to_string(),
            kind: kind.to_string(),
            time: 5,
        });
    }
}

/// Tracks the user's in/out mark points for clip export. Both are `Option` because
/// the user may set them independently (mark in, then scrub, then mark out).
pub struct ClipState {
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
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

/// Shared application state. All fields are behind `Arc`, atomics, or `Mutex`/`DashMap`
/// for safe concurrent access from the GPUI render loop, GStreamer callbacks, and
/// background threads.
pub struct AppState {
    /// The active GStreamer recording pipeline, if any.
    pub pipeline: Mutex<Option<gst::Pipeline>>,
    /// Separate pipeline for mic monitoring (VU meter) independent of recording.
    pub mic_monitor_pipeline: Mutex<Option<gst::Pipeline>>,
    /// User-defined clip in/out points for export.
    pub clip_state: Mutex<ClipState>,
    /// Manual recording sessions keyed by session ID. DashMap because the UI reads
    /// these for rendering while the auto-record loop may mutate them concurrently.
    pub manual_sessions: Arc<DashMap<i32, GameSession>>,
    /// HWND handles for manual recording targets, keyed by session ID.
    pub manual_window_handles: Arc<DashMap<i32, u64>>,
    pub available_windows: Mutex<Vec<WindowInfo>>,
    /// Maps friendly device names to WASAPI device IDs for audio routing.
    pub audio_device_map: Arc<DashMap<String, String>>,
    /// In-memory mirror of the config's game registry for fast, lock-free lookups.
    pub game_registry: Arc<DashMap<String, GameSettings>>,
    /// Debounce guard: prevents seek storms when the user drags the scrubber.
    pub last_seek_time: Mutex<std::time::Instant>,
    pub is_recording: Arc<AtomicBool>,
    /// Guards against stop→restart race: true while the pipeline is tearing down.
    pub pipeline_stopping: Arc<AtomicBool>,
    pub export_running: Arc<AtomicBool>,
    pub export_progress: Arc<Mutex<f32>>,
    /// Shared mic capture instance — lives across recording sessions to avoid
    /// device open/close latency on each recording start.
    pub mic_provider: Arc<Mutex<Option<Arc<MicProvider>>>>,
    pub current_session_id: AtomicI32,
    pub pending_seek: Mutex<Option<f64>>,
    pub was_playing_before_scrub: AtomicBool,
    /// Caches: avoid redundant disk/network I/O on every UI frame.
    pub playlist_cache: Arc<DashMap<String, std::path::PathBuf>>,
    pub process_cache: Arc<DashMap<String, u32>>,
    pub artwork_cache: Arc<DashMap<String, Option<String>>>,
    pub current_recording_duration: Mutex<f64>,
    pub toast_manager: ToastManager,
    /// D3D11 device handle shared with the video player for zero-copy frame rendering.
    pub d3d11_device: Arc<Mutex<Option<SendHandle>>>,
    /// Ordering list of recording sessions for the current game's timeline view.
    pub current_session_blocks: Mutex<Vec<SessionBlock>>,
    pub virtual_audio_routers: Mutex<Vec<crate::virtual_audio_router::VirtualAudioRouter>>,
    /// The source name currently being recorded (e.g., "monitor" or a game title).
    /// Used by the tray thread for instant replay export.
    pub recording_source: Mutex<Option<String>>,
}

impl AppState {
    /// Inserts into the artwork cache with an eviction policy.
    /// To keep it simple and performant without extra crates, we just clear
    /// the cache if it grows beyond 500 entries (approx 100KB of strings).
    pub fn insert_artwork(&self, title: String, path: Option<String>) {
        if self.artwork_cache.len() > 500 {
            log::info!("[State] Evicting artwork cache (size exceeded)");
            self.artwork_cache.clear();
        }
        self.artwork_cache.insert(title, path);
    }

    /// Inserts into the process cache with an eviction policy.
    pub fn insert_process(&self, title: String, pid: u32) {
        if self.process_cache.len() > 500 {
            log::info!("[State] Evicting process cache (size exceeded)");
            self.process_cache.clear();
        }
        self.process_cache.insert(title, pid);
    }

    pub fn new() -> Self {
        Self {
            pipeline: Mutex::new(None),
            mic_monitor_pipeline: Mutex::new(None),
            clip_state: Mutex::new(ClipState {
                start_time: None,
                end_time: None,
            }),
            manual_sessions: Arc::new(DashMap::new()),
            manual_window_handles: Arc::new(DashMap::new()),
            available_windows: Mutex::new(Vec::new()),
            audio_device_map: Arc::new(DashMap::new()),
            game_registry: Arc::new(DashMap::new()),
            last_seek_time: Mutex::new(std::time::Instant::now()),
            is_recording: Arc::new(AtomicBool::new(false)),
            pipeline_stopping: Arc::new(AtomicBool::new(false)),
            export_running: Arc::new(AtomicBool::new(false)),
            export_progress: Arc::new(Mutex::new(0.0)),
            mic_provider: Arc::new(Mutex::new(None)),
            current_session_id: AtomicI32::new(3),
            pending_seek: Mutex::new(None),
            was_playing_before_scrub: AtomicBool::new(false),
            playlist_cache: Arc::new(DashMap::new()),
            process_cache: Arc::new(DashMap::new()),
            artwork_cache: Arc::new(DashMap::new()),
            current_recording_duration: Mutex::new(0.0),
            toast_manager: ToastManager::new(),
            d3d11_device: Arc::new(Mutex::new(None)),
            current_session_blocks: Mutex::new(Vec::new()),
            virtual_audio_routers: Mutex::new(Vec::new()),
            recording_source: Mutex::new(None),
        }
    }
}
