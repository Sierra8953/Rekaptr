use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// In-memory cache so `AppConfig::load()` doesn't hit SQLite on every render frame.
/// Render-thread callers (`get_current_audio_tracks`, settings panels) were opening
/// a new connection ~60x/sec before this — painful on HDDs or with AV scanning.
static CONFIG_CACHE: std::sync::OnceLock<parking_lot::RwLock<Option<AppConfig>>> =
    std::sync::OnceLock::new();

fn config_cache() -> &'static parking_lot::RwLock<Option<AppConfig>> {
    CONFIG_CACHE.get_or_init(|| parking_lot::RwLock::new(None))
}

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
    /// Per-game in-game overlay override: `None` inherits the global setting
    /// (with the anti-cheat allowlist applied), `Some(true/false)` forces it.
    #[serde(default)]
    pub overlay_enabled: Option<bool>,
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
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
    #[serde(default)]
    pub first_run_completed: bool,
    #[serde(default)]
    pub startup_with_windows: bool,
    #[serde(default = "default_hotkeys")]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub auto_delete_clips_days: Option<i32>,
    #[serde(default = "default_export_format")]
    pub default_export_format: String,
    /// In-game overlay configuration. See `crate::overlay`.
    #[serde(default)]
    pub overlay: crate::overlay::OverlaySettings,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HotkeyConfig {
    /// Virtual key code for toggle recording (default: F9 = 0x78)
    pub toggle_recording_vk: u32,
    /// Modifier flags for toggle recording (MOD_CONTROL=2, MOD_ALT=1, MOD_SHIFT=4)
    pub toggle_recording_mod: u32,
    /// Virtual key code for save clip (default: F10 = 0x79)
    pub save_clip_vk: u32,
    pub save_clip_mod: u32,
    /// Virtual key code for toggle mic mute (default: F11 = 0x7A)
    pub toggle_mic_vk: u32,
    pub toggle_mic_mod: u32,
    /// Push-to-talk key
    #[serde(default)]
    pub push_to_talk_vk: u32,
    #[serde(default)]
    pub push_to_talk_mod: u32,
    /// Marker hotkeys
    #[serde(default)]
    pub marker_flag_vk: u32,
    #[serde(default)]
    pub marker_flag_mod: u32,
    #[serde(default)]
    pub marker_kill_vk: u32,
    #[serde(default)]
    pub marker_kill_mod: u32,
    #[serde(default)]
    pub marker_death_vk: u32,
    #[serde(default)]
    pub marker_death_mod: u32,
    #[serde(default)]
    pub marker_highlight_vk: u32,
    #[serde(default)]
    pub marker_highlight_mod: u32,
    /// Toggle the in-game overlay (default F8).
    #[serde(default = "default_overlay_vk")]
    pub toggle_overlay_vk: u32,
    #[serde(default)]
    pub toggle_overlay_mod: u32,
}

fn default_overlay_vk() -> u32 {
    0x77 // F8
}

impl HotkeyConfig {
    pub fn defaults() -> Self {
        default_hotkeys()
    }
}

fn default_hotkeys() -> HotkeyConfig {
    HotkeyConfig {
        toggle_recording_vk: 0x78, // F9
        toggle_recording_mod: 0,   // No modifier
        save_clip_vk: 0x79,        // F10
        save_clip_mod: 0,
        toggle_mic_vk: 0x7A,       // F11
        toggle_mic_mod: 0,
        push_to_talk_vk: 0,
        push_to_talk_mod: 0,
        marker_flag_vk: 0,
        marker_flag_mod: 0,
        marker_kill_vk: 0,
        marker_kill_mod: 0,
        marker_death_vk: 0,
        marker_death_mod: 0,
        marker_highlight_vk: 0,
        marker_highlight_mod: 0,
        toggle_overlay_vk: default_overlay_vk(),
        toggle_overlay_mod: 0,
    }
}

fn default_max_buffer_size() -> i32 { 50 }
fn default_export_format() -> String { "mp4".to_string() }

fn default_storage_path() -> String {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(local_app_data).join("Rekaptr").join("Recordings");
        return path.to_string_lossy().to_string();
    }
    PathBuf::from("C:\\RekaptrRecordings").to_string_lossy().to_string()
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
            first_run_completed: false,
            startup_with_windows: false,
            hotkeys: default_hotkeys(),
            minimize_to_tray: false,
            auto_delete_clips_days: None,
            default_export_format: default_export_format(),
            overlay: crate::overlay::OverlaySettings::default(),
        }
    }
}

impl AppConfig {
    pub fn is_first_run(&self) -> bool {
        !self.first_run_completed
    }

    /// Ensure the configured encoder is actually available on this system.
    /// If not, fall back to the best available encoder and persist the change.
    pub fn validate_and_fix_encoder(&mut self) {
        use gstreamer as gst;
        let _ = gst::init();

        let gst_element = match self.global_video.encoder.as_str() {
            "nvav1enc" => "nvd3d11av1enc",
            "nvh264enc" => "nvd3d11h264enc",
            "nvh265enc" => "nvd3d11h265enc",
            other => other,
        };

        if gst::ElementFactory::find(gst_element).is_some() {
            return; // configured encoder is available
        }

        log::warn!(
            "[Config] Configured encoder '{}' (element '{}') not found, auto-selecting fallback",
            self.global_video.encoder, gst_element
        );

        // Priority: NVENC H.264 > NVENC H.265 > NVENC AV1 > x264
        let fallbacks = [
            ("nvd3d11h264enc", "nvh264enc"),
            ("nvd3d11h265enc", "nvh265enc"),
            ("nvd3d11av1enc", "nvav1enc"),
            ("x264enc", "x264enc"),
        ];

        for (element, config_id) in &fallbacks {
            if gst::ElementFactory::find(element).is_some() {
                log::info!("[Config] Auto-selected encoder: {} ({})", config_id, element);
                self.global_video.encoder = config_id.to_string();
                self.save();
                return;
            }
        }

        log::error!("[Config] No supported encoders found on this system!");
    }

    pub fn get_db_path() -> PathBuf {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let dir = PathBuf::from(local_app_data).join("Rekaptr");
            let _ = std::fs::create_dir_all(&dir);
            return dir.join("rekaptr.db");
        }
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("rekaptr.db")))
            .unwrap_or_else(|| PathBuf::from("rekaptr.db"))
    }

    /// One-time migration: if a DB exists next to the EXE (legacy location used
    /// before %LOCALAPPDATA%\Rekaptr) and no DB exists at the new location, copy
    /// it over. The legacy file is left in place as a backup.
    fn migrate_legacy_exe_dir_db() {
        let Some(exe_dir_db) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("rekaptr.db")))
        else {
            return;
        };
        let new_path = Self::get_db_path();
        if exe_dir_db == new_path || !exe_dir_db.exists() || new_path.exists() {
            return;
        }
        log::info!(
            "[Config] Migrating DB from legacy location {} -> {}",
            exe_dir_db.display(),
            new_path.display()
        );
        if let Err(e) = std::fs::copy(&exe_dir_db, &new_path) {
            log::warn!("[Config] Legacy DB copy failed: {}", e);
        }
    }

    pub fn init_db() -> rusqlite::Result<()> {
        Self::migrate_legacy_exe_dir_db();
        let conn = rusqlite::Connection::open(Self::get_db_path())?;
        
        // Use execute for PRAGMAs that we don't need the return value from
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        let _ = conn.execute("PRAGMA synchronous=NORMAL", []);
        let _ = conn.execute("PRAGMA cache_size=-2000", []);
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (id INTEGER PRIMARY KEY, json_data TEXT)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS favorites (clip_path TEXT PRIMARY KEY)",
            [],
        )?;
        Ok(())
    }

    pub fn load() -> Self {
        if let Some(cached) = config_cache().read().as_ref() {
            return cached.clone();
        }
        let loaded = Self::load_from_disk();
        *config_cache().write() = Some(loaded.clone());
        loaded
    }

    fn load_from_disk() -> Self {
        if let Err(e) = Self::init_db() {
            log::warn!(
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
            .and_then(|p| p.parent().map(|d| d.join("config.json")))
            .unwrap_or_else(|| PathBuf::from("config.json"));

        if json_path.exists() {
            log::info!("[Config] Migrating config.json to SQLite...");
            let content = std::fs::read_to_string(&json_path).unwrap_or_default();
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                config.write_to_disk();
                let _ = std::fs::remove_file(json_path);
                return config;
            }
        }

        let default_config = Self::default();
        default_config.write_to_disk();
        default_config
    }

    pub fn save(&self) {
        self.write_to_disk();
        *config_cache().write() = Some(self.clone());
    }

    fn write_to_disk(&self) {
        if let Ok(conn) = rusqlite::Connection::open(Self::get_db_path()) {
            let json = match serde_json::to_string(self) {
                Ok(j) => j,
                Err(e) => {
                    log::error!("[Config] Failed to serialize config: {}", e);
                    return;
                }
            };
            let _ = conn.execute(
                "INSERT OR REPLACE INTO config (id, json_data) VALUES (1, ?1)",
                [&json],
            );
        }
    }

    pub fn load_favorites() -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        if let Ok(conn) = rusqlite::Connection::open(Self::get_db_path()) {
            if let Ok(mut stmt) = conn.prepare("SELECT clip_path FROM favorites") {
                let rows = stmt.query_map([], |row| row.get::<_, String>(0));
                if let Ok(rows) = rows {
                    for path in rows.flatten() {
                        set.insert(path);
                    }
                }
            }
        }
        set
    }

    pub fn set_favorite(clip_path: &str, favorite: bool) {
        if let Ok(conn) = rusqlite::Connection::open(Self::get_db_path()) {
            if favorite {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO favorites (clip_path) VALUES (?1)",
                    [clip_path],
                );
            } else {
                let _ = conn.execute(
                    "DELETE FROM favorites WHERE clip_path = ?1",
                    [clip_path],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let loaded: AppConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(loaded.global_video.encoder, config.global_video.encoder);
        assert_eq!(loaded.global_video.fps, config.global_video.fps);
        assert_eq!(loaded.global_video.resolution, config.global_video.resolution);
        assert_eq!(loaded.global_video.bitrate_kbps, config.global_video.bitrate_kbps);
        assert_eq!(loaded.max_buffer_size_gb, config.max_buffer_size_gb);
        assert_eq!(loaded.storage_path, config.storage_path);
        assert_eq!(loaded.first_run_completed, config.first_run_completed);
        assert_eq!(loaded.hotkeys.toggle_recording_vk, config.hotkeys.toggle_recording_vk);
        assert_eq!(loaded.global_audio_tracks.len(), 6);
    }

    #[test]
    fn test_config_deserialize_with_missing_fields() {
        // Simulate a config from an older version with missing new fields
        let minimal_json = r#"{
            "global_video": {
                "encoder": "nvav1enc",
                "rate_control_index": 0,
                "bitrate_kbps": 15000,
                "cq_level": 20,
                "resolution": "1920x1080",
                "fps": 60,
                "retention_minutes": 10,
                "gop_size": 60,
                "bframes": 0,
                "preset": "p4",
                "zero_latency": true,
                "lookahead": true,
                "lookahead_frames": 32,
                "spatial_aq": true,
                "temporal_aq": true,
                "artwork_path": null
            },
            "selected_adapter_index": -1,
            "global_audio_tracks": [],
            "mic_settings": {
                "device_name": "Default",
                "noise_suppression": false,
                "noise_gate_enabled": false,
                "noise_gate_threshold": -40.0,
                "noise_gate_attack": 5,
                "noise_gate_release": 50,
                "compressor_enabled": false,
                "compressor_threshold": -20.0,
                "compressor_ratio": 4.0,
                "compressor_attack": 10,
                "compressor_release": 100,
                "limiter_enabled": false,
                "limiter_threshold": -1.0,
                "gain_db": 0.0,
                "force_mono": false
            },
            "game_registry": {}
        }"#;

        let config: AppConfig = serde_json::from_str(minimal_json).expect("deserialize minimal config");
        // Fields with #[serde(default)] should get their defaults
        assert_eq!(config.first_run_completed, false);
        assert_eq!(config.startup_with_windows, false);
        assert_eq!(config.max_buffer_size_gb, 50);
        assert_eq!(config.hotkeys.toggle_recording_vk, 0x78); // F9
    }

    #[test]
    fn test_default_config_values() {
        let config = AppConfig::default();
        assert_eq!(config.global_video.encoder, "nvav1enc");
        assert_eq!(config.global_video.fps, 60);
        assert_eq!(config.global_video.resolution, "1920x1080");
        assert_eq!(config.max_buffer_size_gb, 50);
        assert!(!config.first_run_completed);
        assert!(!config.startup_with_windows);
        assert_eq!(config.global_audio_tracks.len(), 6);
    }
}
