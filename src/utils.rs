//! Recording utilities: segment management, playlist generation, buffer cleanup,
//! and Steam artwork lookup.
//!
//! ## Segment naming convention
//! Segments are named `seg_NNNNNNNNNN_XXXXms.m4s` where:
//! - `NNNNNNNNNN`: zero-padded monotonic index (from `splitmuxsink`'s `start-index`)
//! - `XXXXms`: duration in milliseconds, appended by the segment finalizer after
//!   `splitmuxsink` closes the file. This suffix is the source of truth for duration
//!   and doubles as a "finalized" marker — segments without the `ms` suffix are still
//!   being written and are excluded from playlists.
//!
//! ## Playlist generation
//! Two playlist formats are generated from the same segment list:
//! - **M3U8** (`master.m3u8`): served by the local HLS server for instant in-app playback.
//!   HLS was chosen because GPUI's media player supports it natively and it handles the
//!   live-edge/VOD transition gracefully.
//! - **ffconcat** (`view.ffconcat`): used by FFmpeg during clip export. FFmpeg's concat
//!   demuxer needs this format and cannot consume M3U8 without re-downloading segments.
//!
//! Both playlists sort segments by filesystem modification time rather than by index
//! number. This is intentional: after buffer cleanup deletes old segments, the surviving
//! segments may have non-contiguous indices. Modification time preserves the true
//! chronological order regardless of gaps.
//!
//! ## Buffer cleanup architecture
//! A dedicated background thread watches the storage directory via `notify` (filesystem
//! events). On any file change, it debounces for 5 seconds (to batch concurrent segment
//! writes from multiple audio/video tracks), then enforces two retention limits:
//! 1. **Per-game duration limit**: respects each game's configured retention window
//! 2. **Global storage size limit**: caps total disk usage across all games
//! Oldest segments (by modification time) are deleted first in both passes.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use winreg::enums::*;
use winreg::RegKey;

/// Strips non-alphanumeric characters to create a filesystem-safe directory name.
/// Used as the canonical key for mapping game titles to their storage directories.
pub fn clean_title(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

static STORAGE_ROOT: OnceLock<PathBuf> = OnceLock::new();
static GST_INIT: OnceLock<()> = OnceLock::new();

/// Initializes GStreamer exactly once. This is called lazily by recording and
/// playback engines to avoid the 40ms+ cost during the application's critical
/// startup path.
pub fn init_gstreamer() {
    GST_INIT.get_or_init(|| {
        let start = std::time::Instant::now();
        if let Err(e) = gstreamer::init() {
            log::error!("[Utils] Failed to initialize GStreamer: {:?}", e);
        } else {
            let (major, minor, micro, nano) = gstreamer::version();
            log::info!("[Utils] Lazy GStreamer init took {:?}. Version: {}.{}.{}.{}", 
                start.elapsed(), major, minor, micro, nano);
        }
    });
}

pub fn get_storage_root() -> PathBuf {
    STORAGE_ROOT.get_or_init(|| {
        let config = crate::config::AppConfig::load();
        PathBuf::from(&config.storage_path)
    }).clone()
}

/// Call this after the user changes the storage path in settings to update the cached value.
pub fn set_storage_root(path: PathBuf) {
    // OnceLock can't be reset, so we just update the config and log.
    // The app needs to restart for a new storage path to take full effect.
    let mut config = crate::config::AppConfig::load();
    config.storage_path = path.to_string_lossy().to_string();
    config.save();
}

pub fn get_ffmpeg_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("bin")
        .join("ffmpeg.exe")
}

pub fn get_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_dir_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    }
    Ok(size)
}

pub fn clear_all_buffers() {
    let root = get_storage_root();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name != "Clips" && name != "Cache" && !name.starts_with(".") {
                    // This is a session folder, potentially containing buffers
                    let _ = std::fs::remove_dir_all(path);
                }
            }
        }
    }
}

pub fn fetch_all_clips() -> Vec<crate::state::Clip> {
    let root = get_storage_root();
    let clips_dir = root.join("Clips");
    let mut clips = Vec::new();

    let config = crate::config::AppConfig::load();

    if let Ok(entries) = std::fs::read_dir(&clips_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // Game folder
                let folder_name = entry.file_name().to_string_lossy().to_string();
                let game_title = config.game_registry.iter()
                    .find(|(t, _)| clean_title(t) == folder_name)
                    .map(|(t, _)| t.clone())
                    .unwrap_or_else(|| folder_name.replace('_', " "));

                if let Ok(clip_entries) = std::fs::read_dir(&path) {
                    for clip_entry in clip_entries.filter_map(|e| e.ok()) {
                        let clip_path = clip_entry.path();
                        if clip_path.extension().map_or(false, |ext| ext == "mp4" || ext == "mkv") {
                            if let Ok(meta) = clip_path.metadata() {
                                let timestamp = meta.modified()
                                    .unwrap_or(std::time::SystemTime::now())
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let size_mb = meta.len() / (1024 * 1024);
                                let date = chrono::DateTime::<chrono::Local>::from(meta.modified().unwrap_or(std::time::SystemTime::now()))
                                    .format("%Y-%m-%d %H:%M")
                                    .to_string();

                                let mut thumb_path = clip_path.clone();
                                thumb_path.set_extension("jpg");
                                let thumbnail_path = if thumb_path.exists() {
                                    Some(thumb_path)
                                } else {
                                    None
                                };

                                clips.push(crate::state::Clip {
                                    title: game_title.clone(),
                                    path: clip_path,
                                    timestamp,
                                    size: format!("{} MB", size_mb),
                                    duration: "0:00".to_string(), // Placeholder
                                    date,
                                    thumbnail_path,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    clips
}

pub fn get_all_clips() -> Vec<crate::state::Clip> {
    fetch_all_clips()
}

/// Probes a segment's actual duration via ffprobe. Used as a fallback when the
/// filename doesn't contain the `_XXXXms` suffix (e.g., segments from older versions).
/// Subtracts `start_time` from `duration` to get the true playable length, since fMP4
/// segments may have a non-zero start offset from the `decode-time-offset` in the muxer.
fn get_exact_duration(file_path: &std::path::Path) -> f64 {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration,start_time",
            "-of", "default=noprint_wrappers=1",
            &file_path.to_string_lossy()
        ])
        .output();

    if let Ok(out) = output {
        let out_str = String::from_utf8_lossy(&out.stdout);
        let mut start_time = 0.0;
        let mut duration = 0.0;

        // Parse the key=value lines
        for line in out_str.lines() {
            if let Some(val) = line.strip_prefix("start_time=") {
                start_time = val.parse::<f64>().unwrap_or(0.0);
            } else if let Some(val) = line.strip_prefix("duration=") {
                duration = val.parse::<f64>().unwrap_or(0.0);
            }
        }

        let actual_duration = duration - start_time;

        // Sanity check: If the segment is valid and under our 2-second max + buffer
        if actual_duration > 0.0 && actual_duration <= 3.0 {
            return actual_duration;
        }
    }

    // Fallback to 2.0 if ffprobe fails or the file is corrupted
    2.0
}

/// Extracts segment duration from the filename's `_XXXXms` suffix.
/// Returns `None` for segments still being written (no suffix yet).
pub fn get_segment_duration(path: &Path) -> Option<f64> {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy(),
        None => return None,
    };
    // Look for the _XXXXms pattern in the filename
    if let Some(ms_pos) = name.rfind('_') {
        let part = &name[ms_pos + 1..];
        if let Some(ms_str) = part.strip_suffix("ms.m4s").or_else(|| part.strip_suffix("ms")) {
            if let Ok(ms) = ms_str.parse::<u64>() {
                return Some(ms as f64 / 1000.0);
            }
        }
    }
    None
}

/// Generates an M3U8 playlist and computes the timeline session blocks for a game.
/// Only includes finalized segments (those with the `ms` duration suffix).
/// Returns `None` if the game directory doesn't exist or has no valid segments.
pub fn generate_session_playlist(game_title: &str, _active_session_id: Option<u64>) -> Option<(PathBuf, Vec<crate::state::SessionBlock>)> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    // 1. Scan disk for segments that have been finalized (have the ms suffix)
    let mut segments = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        segments.push((modified, path, duration));
                    }
                }
            }
        }
    }

    if segments.is_empty() { return None; }
    
    // Sort by modification date to maintain the rolling buffer timeline
    segments.sort_by_key(|s| s.0);

    // 2. Calculate total duration from finalized segments only
    let total_duration: f64 = segments.iter().map(|s| s.2).sum();

    // 3. Represent as one continuous block for the timeline
    let session_blocks = vec![crate::state::SessionBlock {
        start_timestamp: 0,
        duration_secs: total_duration,
        timeline_offset_secs: 0.0,
        playlist_path: game_dir.join("master.m3u8"),
    }];

    let master_playlist = generate_master_playlist(game_title);
    Some((master_playlist?, session_blocks))
}

/// Writes the M3U8 playlist file to disk for the local HLS server to serve.
/// Uses `EXT-X-PLAYLIST-TYPE:EVENT` so the HLS player knows more segments may appear.
pub fn generate_master_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    let mut segments = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        segments.push((modified, path, duration));
                    }
                }
            }
        }
    }

    if segments.is_empty() { return None; }
    segments.sort_by_key(|s| s.0);

    let master_playlist_path = game_dir.join("master.m3u8");
    let mut m3u8_content = String::from("#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:EVENT\n");

    for (_, path, duration) in segments {
        if let Some(file_name) = path.file_name() {
            m3u8_content.push_str(&format!("#EXTINF:{:.3},\n", duration));
            m3u8_content.push_str(&format!("{}\n", file_name.to_string_lossy()));
        }
    }
    
    m3u8_content.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&master_playlist_path, m3u8_content);

    Some(master_playlist_path)
}

/// Writes an ffconcat playlist for FFmpeg-based clip export. ffconcat is required
/// because FFmpeg's concat demuxer cannot consume HLS playlists directly — it needs
/// explicit `file`/`duration` directives to concatenate fMP4 segments without re-encoding.
pub fn generate_ffconcat_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    let mut segments = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        segments.push((modified, path, duration));
                    }
                }
            }
        }
    }

    if segments.is_empty() { return None; }
    segments.sort_by_key(|s| s.0);

    let ffconcat_path = game_dir.join("view.ffconcat");
    let mut ffconcat_content = String::from("ffconcat version 1.0\n");

    for (_, path, duration) in segments {
        if let Some(file_name) = path.file_name() {
            ffconcat_content.push_str(&format!("file '{}'\nduration {}\n", file_name.to_string_lossy(), duration));
        }
    }

    if let Ok(_) = std::fs::write(&ffconcat_path, ffconcat_content) {
        Some(ffconcat_path)
    } else {
        None
    }
}


/// Resolves a game title to its Steam hero artwork URL or cached local path.
///
/// Lookup flow:
/// 1. Try hardcoded AppID map for common titles (avoids network roundtrip)
/// 2. Fall back to Steam Store search API with a 3-second timeout
/// 3. Check local cache (`Cache/Artwork/{appid}_hero.jpg`) — skip files <5KB (broken downloads)
/// 4. Return the CDN URL if not cached, letting the caller download asynchronously
///
/// Returns `None` for non-Steam games (e.g., Valorant) that don't have Steam artwork.
pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    let sanitized: String = game_title.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();

    let slug = sanitized.replace(' ', "");
    eprintln!("[Utils] find_steam_artwork for title: '{}' (slug: '{}')", game_title, slug);

    let mut app_id = match slug.as_str() {
        s if s.contains("counterstrike") || s.contains("cs2") || s.contains("csgo") => Some("730".to_string()),
        s if s.contains("dota") => Some("570".to_string()),
        s if s.contains("cyberpunk") => Some("1091500".to_string()),
        s if s.contains("halflife2") => Some("220".to_string()),
        s if s.contains("eldenring") => Some("1245620".to_string()),
        s if s.contains("baldursgate") => Some("1086940".to_string()),
        s if s.contains("arcraiders") => Some("1808500".to_string()),
        s if s.contains("monitor") => Some("1144200".to_string()),
        s if s.contains("thelastofus") => Some("1888500".to_string()),
        s if s.contains("vivaldi") => Some("1144200".to_string()),
        s if s.contains("valorant") => None,
        _ => None,
    };

    // If hardcoded fails, try to look it up on Steam Store API synchronously
    if app_id.is_none() {
        let url = format!("https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US", url::form_urlencoded::byte_serialize(game_title.as_bytes()).collect::<String>());

        if let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .user_agent("Luma/1.0")
            .build() 
        {
            if let Ok(response) = client.get(&url).send() {
                if let Ok(json) = response.json::<serde_json::Value>() {
                    if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
                        if let Some(first_item) = items.first() {
                            if let Some(id) = first_item.get("id").and_then(|id| id.as_i64()) {
                                app_id = Some(id.to_string());
                                eprintln!("[Utils] Found Steam AppID {} for '{}' via search API", id, game_title);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(id) = app_id {
        let cache_dir = get_storage_root().join("Cache").join("Artwork");
        let _ = std::fs::create_dir_all(&cache_dir);
        let local_path = cache_dir.join(format!("{}_hero.jpg", id));

        if local_path.exists() {
            if let Ok(meta) = local_path.metadata() {
                if meta.len() > 5000 { // Ignore files < 5KB (likely broken/404)
                    let path_str = local_path.to_string_lossy().replace('\\', "/");
                    eprintln!("[Utils] Using cached artwork: {}", path_str);
                    return Some(path_str);
                } else {
                    eprintln!("[Utils] Cached artwork for '{}' is too small ({}), ignoring", game_title, meta.len());
                    let _ = std::fs::remove_file(&local_path);
                }
            }
        }

        let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg", id);
        eprintln!("[Utils] Artwork not cached or broken, returning URL: {}", url);
        return Some(url);
    }

    eprintln!("[Utils] No Steam AppID found for '{}'", game_title);
    None
}
use std::sync::mpsc::{channel, Sender};
use parking_lot::Mutex;
use once_cell::sync::Lazy;

static CLEANUP_SENDER: Lazy<Mutex<Option<Sender<()>>>> = Lazy::new(|| Mutex::new(None));

/// Wakes the cleanup thread immediately (e.g., after the user changes retention settings).
/// The thread debounces internally so rapid triggers are coalesced.
pub fn trigger_buffer_cleanup() {
    if let Some(tx) = CLEANUP_SENDER.lock().as_ref() {
        let _ = tx.send(());
    }
}

/// Spawns the background cleanup thread that enforces retention limits.
///
/// The thread is event-driven (not polling): it sleeps on a channel that receives
/// events from both the filesystem watcher and manual `trigger_buffer_cleanup()` calls.
/// On wake, it debounces for 5 seconds to batch simultaneous segment closes, then runs
/// a two-pass cleanup:
/// 1. Per-game pass: delete oldest segments until each game is within its retention window
/// 2. Global pass: if total storage still exceeds `max_buffer_size_gb`, delete oldest
///    segments across all games until under budget
///
/// Runs at below-normal thread priority to avoid competing with the recording pipeline.
pub fn start_buffer_cleanup_thread(root_dir: PathBuf) {
    use notify::{RecursiveMode, Watcher, Config};

    let result = std::thread::Builder::new()
        .name("Luma Cleanup".to_string())
        .spawn(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Threading::*;
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
            }

            let (tx, rx) = channel();
            
            // Register our global sender so the UI can wake us up
            *CLEANUP_SENDER.lock() = Some(tx.clone());

            let mut watcher = match notify::RecommendedWatcher::new(move |res| {
                if let Ok(_) = res {
                    let _ = tx.send(());
                }
            }, Config::default()) {
                Ok(w) => w,
                Err(e) => {
                    log::error!("[Cleanup] Failed to create filesystem watcher: {}", e);
                    return;
                }
            };
            
            let _ = watcher.watch(&root_dir, RecursiveMode::Recursive);

            loop {
                // 1. Wait for either a file event or a manual trigger (completely idle until then)
                match rx.recv() {
                    Ok(_) => {
                        // 2. Debounce: Wait a few seconds to let all simultaneous segment 
                        // closures (A/V tracks) finish so we only scan once.
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        
                        // Drain any other pending events in the channel
                        while let Ok(_) = rx.try_recv() {}

                        let config = crate::config::AppConfig::load();
                        let max_bytes = config.max_buffer_size_gb as u64 * 1024 * 1024 * 1024;
                        let mut total_size = 0;
                        let mut global_segments = Vec::new();

                        if let Ok(entries) = std::fs::read_dir(&root_dir) {
                            for entry in entries.filter_map(|e| e.ok()) {
                                if entry.path().is_dir() {
                                    let game_title = entry.file_name().to_string_lossy().to_string();
                                    if game_title == "Clips" || game_title == "Cache" || game_title.starts_with(".") { continue; }
                                    
                                    // 1. Get retention for this specific game
                                    let retention_mins = if game_title == "monitor" {
                                        config.global_video.retention_minutes as f64
                                    } else {
                                        config.game_registry.iter().find(|(t, _)| clean_title(t) == game_title)
                                            .map(|(_, s)| s.retention_minutes as f64)
                                            .unwrap_or(config.global_video.retention_minutes as f64)
                                    };
                                    let retention_secs = retention_mins * 60.0;

                                    if let Ok(seg_entries) = std::fs::read_dir(entry.path()) {
                                        let mut game_segments = Vec::new();
                                        for seg in seg_entries.filter_map(|e| e.ok()) {
                                            let path = seg.path();
                                            if path.extension().map_or(false, |ex| ex == "m4s") {
                                                let file_name = match path.file_name() {
                                                    Some(n) => n.to_string_lossy(),
                                                    None => continue,
                                                };
                                                if !file_name.starts_with("seg_") { continue; }
                                                
                                                if let Ok(meta) = seg.metadata() {
                                                    let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                                                    game_segments.push((path, modified, meta.len()));
                                                }
                                            }
                                        }

                                        // 2. Sort by modification date (Source of Truth for "Oldest")
                                        game_segments.sort_by(|a, b| a.1.cmp(&b.1));

                                        // 3. ENFORCE PER-GAME RETENTION (Duration-based)
                                        let mut total_game_duration = 0.0;
                                        for (path, _, _) in &game_segments {
                                            total_game_duration += get_segment_duration(path).unwrap_or(6.0);
                                        }
                                        
                                        for (path, _, _) in game_segments.iter() {
                                            if total_game_duration <= retention_secs { break; }
                                            let dur = get_segment_duration(path).unwrap_or(6.0);
                                            let _ = std::fs::remove_file(path);
                                            total_game_duration -= dur;
                                        }

                                        // 4. Track remaining segments for global GB limit
                                        for (path, modified, size) in game_segments {
                                            if path.exists() {
                                                total_size += size;
                                                global_segments.push((path, modified, size));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 5. ENFORCE GLOBAL STORAGE LIMIT (Size-based)
                        if total_size > max_bytes {
                            global_segments.sort_by(|a, b| a.1.cmp(&b.1));
                            let mut current_size = total_size;
                            for (path, _, size) in global_segments {
                                if current_size <= max_bytes { break; }
                                if path.exists() {
                                    current_size -= size;
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Watcher error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    }
                }
            }
        });
    if let Err(e) = result {
        log::error!("[Cleanup] Failed to spawn cleanup thread: {}", e);
    }
}
