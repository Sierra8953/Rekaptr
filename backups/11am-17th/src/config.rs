use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub global_video: VideoSettings,
    pub selected_adapter_index: i32,
    pub global_audio_tracks: Vec<AudioRouting>,
    pub mic_settings: MicSettings,
    pub game_registry: HashMap<String, GameSettings>,
    #[serde(default = "default_max_buffer_size")]
    pub max_buffer_size_gb: i32,
}

fn default_max_buffer_size() -> i32 { 50 }

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
        }
    }
}

impl AppConfig {
    pub fn get_db_path() -> PathBuf {
        std::env::current_exe()
            .map(|p| p.parent().unwrap().join("luma.db"))
            .unwrap_or(PathBuf::from("luma.db"))
    }

    pub fn init_db() -> rusqlite::Result<()> {
        let conn = rusqlite::Connection::open(Self::get_db_path())?;
        
        // Use execute for PRAGMAs that we don't need the return value from
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        let _ = conn.execute("PRAGMA synchronous=NORMAL", []);
        let _ = conn.execute("PRAGMA cache_size=-2000", []);
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (id INTEGER PRIMARY KEY, json_data TEXT)",
            [],
        )?;
        Ok(())
    }

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
            .map(|p| p.parent().unwrap().join("config.json"))
            .unwrap_or(PathBuf::from("config.json"));

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
            let json = serde_json::to_string(self).unwrap();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO config (id, json_data) VALUES (1, ?1)",
                [&json],
            );
        }
    }
}
