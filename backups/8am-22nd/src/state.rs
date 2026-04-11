use crate::audio::MicProvider;
use crate::config::GameSettings;
use crate::game_detector::WindowInfo;
use crate::video_player::SendHandle;
use dashmap::DashMap;
use gstreamer as gst;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64};
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

#[derive(Clone)]
pub struct SessionInfo {
    pub game_title: String,
    pub path: std::path::PathBuf,
    pub date: String,
    pub timestamp: u64,
    pub segment_count: usize,
    pub total_duration_secs: f64,
}

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

/// Maximum entries per cache before eviction kicks in
const CACHE_MAX_ENTRIES: usize = 100;

pub struct AppState {
    pub pipeline: Mutex<Option<gst::Pipeline>>,
    pub mic_monitor_pipeline: Mutex<Option<gst::Pipeline>>,
    pub clip_state: Mutex<ClipState>,
    pub manual_sessions: Arc<DashMap<i32, GameSession>>,
    pub manual_window_handles: Arc<DashMap<i32, u64>>,
    pub available_windows: Mutex<Vec<WindowInfo>>,
    pub audio_device_map: Arc<DashMap<String, String>>,
    pub game_registry: Arc<DashMap<String, GameSettings>>,
    pub last_seek_time: Mutex<std::time::Instant>,
    pub is_recording: Arc<AtomicBool>,
    pub export_running: Arc<AtomicBool>,
    pub export_progress: Arc<Mutex<f32>>,
    pub mic_provider: Arc<Mutex<Option<Arc<MicProvider>>>>,
    pub current_session_id: AtomicI32,
    pub pending_seek: Mutex<Option<f64>>,
    pub was_playing_before_scrub: AtomicBool,
    pub playlist_cache: Arc<DashMap<String, std::path::PathBuf>>,
    pub process_cache: Arc<DashMap<String, u32>>,
    pub artwork_cache: Arc<DashMap<String, Option<String>>>,
    pub current_recording_duration: Mutex<f64>,
    pub toast_manager: ToastManager,
    pub d3d11_device: Arc<Mutex<Option<SendHandle>>>,
    pub current_session_blocks: Mutex<Vec<SessionBlock>>,
    pub virtual_audio_routers: Mutex<Vec<crate::virtual_audio_router::VirtualAudioRouter>>,
    /// Cached list of audio output devices (for system/loopback capture): (id, friendly_name)
    pub audio_output_devices: Mutex<Vec<(String, String)>>,
    /// Cached list of audio input devices (microphones): (id, friendly_name)
    pub audio_input_devices: Mutex<Vec<(String, String)>>,
    // Recording performance stats (updated from GStreamer bus monitor)
    pub rec_stats: RecordingStats,
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

impl AppState {
    /// Evict oldest entries from caches when they exceed the size limit.
    /// Call this periodically (e.g., when games change or on a timer).
    pub fn evict_caches(&self) {
        if self.artwork_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.artwork_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.artwork_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.artwork_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} artwork cache entries", excess);
        }
        if self.playlist_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.playlist_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.playlist_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.playlist_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} playlist cache entries", excess);
        }
        if self.process_cache.len() > CACHE_MAX_ENTRIES {
            let excess = self.process_cache.len() - CACHE_MAX_ENTRIES;
            let keys: Vec<String> = self.process_cache.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys { self.process_cache.remove(&key); }
            log::debug!("[Cache] Evicted {} process cache entries", excess);
        }
    }
}

impl AppState {
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
            audio_output_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
            audio_input_devices: Mutex::new(vec![("Default".to_string(), "Default".to_string())]),
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
    pub fn reset_rec_stats(&self) {
        *self.rec_stats.fps.lock() = 0.0;
        self.rec_stats.dropped_frames.store(0, std::sync::atomic::Ordering::Relaxed);
        *self.rec_stats.bitrate_kbps.lock() = 0.0;
        *self.rec_stats.disk_write_mbps.lock() = 0.0;
        self.rec_stats.segments_written.store(0, std::sync::atomic::Ordering::Relaxed);
        self.rec_stats.last_segment_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
