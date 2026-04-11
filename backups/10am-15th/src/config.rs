//! Application configuration backed by SQLite.
//!
//! Config is stored as a single JSON blob in a SQLite row (id=1). This gives us:
//! - **Atomic writes**: SQLite's WAL journaling prevents config corruption on crash,
//!   which happened regularly with plain JSON when the app was killed mid-write during
//!   recording. A game crash + recording stop + config save is a common simultaneous event.
//! - **No file locking headaches**: SQLite handles concurrent access from the UI thread
//!   and background cleanup thread gracefully.
//! - **Migration path**: on first launch after the switch, we auto-migrate any existing
//!   `config.json` into the database and delete the old file.
//!
//! The config includes global video/audio defaults, per-game overrides (the "game
//! registry"), and mic processing settings. Per-game overrides are keyed by the game's
//! display title and can selectively override encoder, bitrate, retention, and audio
//! routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single audio track in the recording. Up to 6 tracks can be configured, each
/// capturing from a different source (system audio, mic, or a specific application).
/// Tracks are muxed as separate streams in the fMP4 output.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioRouting {
    pub name: String,
    pub enabled: bool,
    pub source_type: String, // "System", "Mic", "App"
    pub device_name: String,
    pub volume: f32, // 0.0 to 10.0 (multiplier)
    #[serde(default)]
    pub app_targets: Vec<String>, // list of process names
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VideoSettings {
    pub encoder: String,
    pub rate_control_index: i32,
    pub bitrate_kbps: i32,
    pub cq_level: i32,
    pub resolution: String,
    pub fps: i32,
    pub retention_minutes: i32,
    // Advanced
    pub gop_size: i32,
    pub bframes: i32,
    pub preset: String,
    pub zero_latency: bool,
    // Quality Suite
    pub lookahead: bool,
    pub lookahead_frames: i32,
    pub spatial_aq: bool,
    pub temporal_aq: bool,
    pub artwork_path: Option<String>,
}

/// Per-game overrides. When a game has custom settings, these take precedence over
/// the global defaults. `video_overrides` and `audio_routing` are `Option` so that
/// `None` means "inherit from global" — only fields the user explicitly customized
/// are stored.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameSettings {
    pub title: String,
    pub target_process: Option<String>,
    pub auto_record: bool,
    pub retention_minutes: i32,
    pub video_overrides: Option<VideoSettings>,
    pub audio_routing: Option<Vec<AudioRouting>>,
    pub record_focus_only: bool,
    pub artwork_path: Option<String>,
}

/// Microphone capture and processing chain. These settings feed into the shared
/// `MicProvider` which runs independently of recording sessions so that mic audio
/// is always warmed up and ready — no cold-start delay when recording begins.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MicSettings {
    pub device_name: String,
    pub noise_suppression: bool,
    pub noise_gate_enabled: bool,
    pub noise_gate_threshold: f32, // dB
    pub noise_gate_attack: i32,    // ms
    pub noise_gate_release: i32,   // ms
    pub compressor_enabled: bool,
    pub compressor_threshold: f32, // dB
    pub compressor_ratio: f32,
    pub compressor_attack: i32,  // ms
    pub compressor_release: i32, // ms
    pub limiter_enabled: bool,
    pub limiter_threshold: f32, // dB
    pub gain_db: f32,
    pub force_mono: bool,
}

/// Top-level configuration. Serialized as JSON into a single SQLite row.
/// The `game_registry` maps display titles to per-game overrides.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub global_video: VideoSettings,
    pub selected_adapter_index: i32,
    pub global_audio_tracks: Vec<AudioRouting>,
    pub mic_settings: MicSettings,
    pub game_registry: HashMap<String, GameSettings>,
    #[serde(default = "default_max_buffer_size")]
    pub max_buffer_size_gb: i32,
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
    #[serde(default)]
    pub window_bounds: Option<WindowBounds>,
    #[serde(default = "default_instant_replay_secs")]
    pub instant_replay_secs: i32,
}

fn default_instant_replay_secs() -> i32 { 30 }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn default_max_buffer_size() -> i32 { 50 }

fn default_storage_path() -> String {
    // Default to %USERPROFILE%\Videos\LumaRecordings, falling back to C:\LumaRecordings
    let base = std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Videos"))
        .unwrap_or_else(|_| PathBuf::from("C:\\"));
    base.join("LumaRecordings").to_string_lossy().to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            global_video: VideoSettings {
                encoder: "nvav1enc".to_string(),
                rate_control_index: 0,
                bitrate_kbps: 15000,
                cq_level: 20,
                resolution: "1920x1080".to_string(),
                fps: 60,
                retention_minutes: 10,
                gop_size: 60,
                bframes: 0,
                preset: "p4".to_string(),
                zero_latency: true,
                lookahead: true,
                lookahead_frames: 32,
                spatial_aq: true,
                temporal_aq: true,
                artwork_path: None,
            },
            selected_adapter_index: -1,
            global_audio_tracks: vec![
                AudioRouting {
                    name: "Track 1".to_string(),
                    enabled: true,
                    source_type: "System".to_string(),
                    device_name: "Default".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
                AudioRouting {
                    name: "Track 2".to_string(),
                    enabled: false,
                    source_type: "Mic".to_string(),
                    device_name: "Default".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
                AudioRouting {
                    name: "Track 3".to_string(),
                    enabled: false,
                    source_type: "App".to_string(),
                    device_name: "".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
                AudioRouting {
                    name: "Track 4".to_string(),
                    enabled: false,
                    source_type: "App".to_string(),
                    device_name: "".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
                AudioRouting {
                    name: "Track 5".to_string(),
                    enabled: false,
                    source_type: "App".to_string(),
                    device_name: "".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
                AudioRouting {
                    name: "Track 6".to_string(),
                    enabled: false,
                    source_type: "App".to_string(),
                    device_name: "".to_string(),
                    volume: 1.0,
                    app_targets: Vec::new(),
                },
            ],
            mic_settings: MicSettings {
                device_name: "Default".to_string(),
                noise_suppression: false,
                noise_gate_enabled: false,
                noise_gate_threshold: -30.0,
                noise_gate_attack: 25,
                noise_gate_release: 200,
                compressor_enabled: false,
                compressor_threshold: -18.0,
                compressor_ratio: 4.0,
                compressor_attack: 6,
                compressor_release: 60,
                limiter_enabled: false,
                limiter_threshold: -3.0,
                gain_db: 0.0,
                force_mono: false,
            },
            game_registry: HashMap::new(),
            max_buffer_size_gb: 50,
            storage_path: default_storage_path(),
            window_bounds: None,
            instant_replay_secs: 30,
        }
    }
}

impl AppConfig {
    pub fn get_db_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|dir| dir.join("luma.db")))
            .unwrap_or_else(|| PathBuf::from("luma.db"))
    }

    /// Initializes the config database with WAL journaling for crash safety.
    /// WAL mode allows concurrent readers (UI) and writers (config save) without blocking.
    /// SYNCHRONOUS=NORMAL trades a tiny durability window for significantly less fsync overhead.
    pub fn init_db() -> rusqlite::Result<()> {
        let conn = rusqlite::Connection::open(Self::get_db_path())?;

        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        let _ = conn.execute("PRAGMA synchronous=NORMAL", []);
        let _ = conn.execute("PRAGMA cache_size=-2000", []);
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (id INTEGER PRIMARY KEY, json_data TEXT)",
            [],
        )?;
        Ok(())
    }

    /// Loads config with a three-tier fallback: SQLite -> JSON migration -> defaults.
    /// The JSON migration path exists for users upgrading from the pre-SQLite version.
    pub fn load() -> Self {
        if let Err(e) = Self::init_db() {
            eprintln!(
                "[Config] Database init failed: {}. Falling back to defaults.",
                e
            );
        }

        let path = Self::get_db_path();

        if let Ok(conn) = rusqlite::Connection::open(&path) {
            let res = conn
                .prepare("SELECT json_data FROM config WHERE id = 1")
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, String>(0)));

            if let Ok(json) = res {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&json) {
                    return config;
                }
            }
        }

        // Fallback to JSON migration
        let json_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|dir| dir.join("config.json")))
            .unwrap_or_else(|| PathBuf::from("config.json"));

        if json_path.exists() {
            println!("[Config] Migrating config.json to SQLite...");
            let content = std::fs::read_to_string(&json_path).unwrap_or_default();
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                config.save();
                let _ = std::fs::remove_file(json_path);
                return config;
            }
        }

        let default_config = Self::default();
        default_config.save();
        default_config
    }

    pub fn save(&self) {
        if let Ok(conn) = rusqlite::Connection::open(Self::get_db_path()) {
            let json = match serde_json::to_string(self) {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("[Config] Failed to serialize config: {}", e);
                    return;
                }
            };
            let _ = conn.execute(
                "INSERT OR REPLACE INTO config (id, json_data) VALUES (1, ?1)",
                [&json],
            );
        }
    }
}
