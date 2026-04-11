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
            .unwrap()
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

pub struct ClipState {
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
}

#[derive(Clone)]
pub struct Clip {
    pub title: String,
    pub path: std::path::PathBuf,
    pub date: String,
    pub size: String,
    pub timestamp: u64,
}

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
    pub mic_provider: Arc<Mutex<Option<Arc<MicProvider>>>>,
    pub current_session_id: AtomicI32,
    pub pending_seek: Mutex<Option<f64>>,
    pub was_playing_before_scrub: AtomicBool,
    pub playlist_cache: Arc<DashMap<String, std::path::PathBuf>>,
    pub process_cache: Arc<DashMap<String, u32>>,
    pub toast_manager: ToastManager,
    pub d3d11_device: Arc<Mutex<Option<SendHandle>>>,
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
            mic_provider: Arc::new(Mutex::new(None)),
            current_session_id: AtomicI32::new(3),
            pending_seek: Mutex::new(None),
            was_playing_before_scrub: AtomicBool::new(false),
            playlist_cache: Arc::new(DashMap::new()),
            process_cache: Arc::new(DashMap::new()),
            toast_manager: ToastManager::new(),
            d3d11_device: Arc::new(Mutex::new(None)),
        }
    }
}
