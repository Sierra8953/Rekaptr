use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const STARTUP_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_REG_VALUE: &str = "Rekaptr";

/// Assumed segment length when ffprobe is unavailable or fails.
const DEFAULT_SEGMENT_DURATION_SECS: f64 = 6.0;

pub fn set_startup_with_windows(enable: bool) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(STARTUP_REG_KEY, KEY_SET_VALUE) {
        Ok(key) => {
            if enable {
                let exe_path = std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("rekaptr.exe"));
                let value = format!("\"{}\"", exe_path.to_string_lossy());
                if let Err(e) = key.set_value(STARTUP_REG_VALUE, &value) {
                    log::error!("[Startup] Failed to set registry value: {}", e);
                } else {
                    log::info!("[Startup] Registered startup with Windows");
                }
            } else {
                let _ = key.delete_value(STARTUP_REG_VALUE);
                log::info!("[Startup] Removed startup with Windows");
            }
        }
        Err(e) => log::error!("[Startup] Failed to open registry key: {}", e),
    }
}

pub fn is_startup_with_windows() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(STARTUP_REG_KEY) {
        key.get_value::<String, _>(STARTUP_REG_VALUE).is_ok()
    } else {
        false
    }
}

pub fn clean_title(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

pub fn get_storage_root() -> PathBuf {
    let config = crate::config::AppConfig::load();
    PathBuf::from(&config.storage_path)
}

pub fn get_ffmpeg_path() -> PathBuf {
    // Try alongside the executable first
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("bin").join("ffmpeg.exe");
            if path.exists() { return path; }
        }
    }
    // Try current directory
    if let Ok(cwd) = std::env::current_dir() {
        let path = cwd.join("bin").join("ffmpeg.exe");
        if path.exists() { return path; }
    }
    // Fallback to PATH
    PathBuf::from("ffmpeg")
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

#[allow(dead_code)]
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
                                if !thumb_path.exists() {
                                    generate_thumbnail(&clip_path, &thumb_path);
                                }
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

#[allow(dead_code)]
pub fn get_all_clips() -> Vec<crate::state::Clip> {
    fetch_all_clips()
}

#[allow(dead_code)]
pub fn fetch_all_sessions() -> Vec<crate::state::SessionInfo> {
    let root = get_storage_root();
    let config = crate::config::AppConfig::load();
    let mut sessions = Vec::new();

    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        if folder_name == "Clips" || folder_name == "Cache" || folder_name.starts_with('.') {
            continue;
        }

        // Count segments and compute total duration
        let mut segment_count = 0usize;
        let mut total_duration = 0.0f64;
        let mut latest_modified = std::time::SystemTime::UNIX_EPOCH;

        if let Ok(seg_entries) = std::fs::read_dir(&path) {
            for seg in seg_entries.filter_map(|e| e.ok()) {
                let seg_path = seg.path();
                if seg_path.extension().map_or(false, |ext| ext == "m4s") {
                    segment_count += 1;
                    total_duration += get_segment_duration(&seg_path).unwrap_or(DEFAULT_SEGMENT_DURATION_SECS);
                    if let Ok(meta) = seg.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        if modified > latest_modified {
                            latest_modified = modified;
                        }
                    }
                }
            }
        }

        if segment_count == 0 { continue; }

        let game_title = config.game_registry.iter()
            .find(|(t, _)| clean_title(t) == folder_name)
            .map(|(t, _)| t.clone())
            .unwrap_or_else(|| folder_name.clone());

        let timestamp = latest_modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let date = chrono::DateTime::<chrono::Local>::from(latest_modified)
            .format("%Y-%m-%d %H:%M")
            .to_string();

        sessions.push(crate::state::SessionInfo {
            game_title,
            path,
            date,
            timestamp,
            segment_count,
            total_duration_secs: total_duration,
        });
    }

    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    sessions
}

/// Generate a thumbnail for a video file by extracting a frame near the start.
/// Creates a `.noThumb` marker file if the video is corrupt to avoid retrying.
fn generate_thumbnail(video_path: &Path, thumb_path: &Path) {
    // Skip if we already know this file is truly corrupt
    let marker = thumb_path.with_extension("noThumb");
    if marker.exists() { return; }

    let ffmpeg = get_ffmpeg_path();

    // Try CUDA hardware decoding first (required for NVENC AV1), then software fallback.
    // -hwaccel auto doesn't reliably select CUDA, so we try it explicitly first.
    let strategies: &[&[&str]] = &[
        &["-hwaccel", "cuda"],
        &[], // software decode fallback
    ];

    for (i, hwaccel_args) in strategies.iter().enumerate() {
        let mut cmd = std::process::Command::new(&ffmpeg);
        cmd.arg("-y");
        for arg in *hwaccel_args {
            cmd.arg(arg);
        }
        cmd.arg("-i").arg(video_path)
            .arg("-ss").arg("1")
            .arg("-vframes").arg("1")
            .arg("-update").arg("1")
            .arg("-q:v").arg("2")
            .arg(thumb_path)
            .creation_flags(0x08000000);

        match cmd.output() {
            Ok(o) if o.status.success() => {
                if i > 0 {
                    log::info!("[Utils] Thumbnail generated with strategy {} for {}", i, video_path.display());
                }
                return; // success
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                // Only mark as truly corrupt for container-level errors
                if stderr.contains("moov atom not found") {
                    log::warn!("[Utils] Clip is corrupt, skipping thumbnail: {}", video_path.display());
                    let _ = std::fs::write(&marker, b"");
                    return;
                }
                // For decode errors (like AV1 "No sequence header"), try next strategy
                if i < strategies.len() - 1 {
                    log::debug!("[Utils] Thumbnail strategy {} failed for {}, trying next", i, video_path.display());
                    continue;
                }
                log::warn!("[Utils] All thumbnail strategies failed for {}: {}", video_path.display(), stderr.chars().take(200).collect::<String>());
            }
            Err(e) => {
                log::warn!("[Utils] Failed to run ffmpeg for thumbnail: {}", e);
                return; // ffmpeg binary issue, no point retrying
            }
        }
    }
}

fn get_ffprobe_path() -> PathBuf {
    // Same search strategy as get_ffmpeg_path
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("bin").join("ffprobe.exe");
            if path.exists() { return path; }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let path = cwd.join("bin").join("ffprobe.exe");
        if path.exists() { return path; }
    }
    PathBuf::from("ffprobe")
}

#[allow(dead_code)]
fn get_exact_duration(file_path: &std::path::Path) -> f64 {
    // Check duration cache first to avoid N+1 ffprobe spawns
    static DURATION_CACHE: std::sync::OnceLock<parking_lot::Mutex<std::collections::HashMap<std::path::PathBuf, f64>>> = std::sync::OnceLock::new();
    let cache = DURATION_CACHE.get_or_init(|| parking_lot::Mutex::new(std::collections::HashMap::new()));

    if let Some(&cached) = cache.lock().get(file_path) {
        return cached;
    }

    let ffprobe = get_ffprobe_path();
    let output = std::process::Command::new(&ffprobe)
        .args([
            "-v", "error",
            "-show_entries", "format=duration,start_time",
            "-of", "default=noprint_wrappers=1",
            &file_path.to_string_lossy()
        ])
        .output();

    let result = if let Ok(out) = output {
        let out_str = String::from_utf8_lossy(&out.stdout);
        let mut start_time = 0.0;
        let mut duration = 0.0;

        for line in out_str.lines() {
            if let Some(val) = line.strip_prefix("start_time=") {
                start_time = val.parse::<f64>().unwrap_or(0.0);
            } else if let Some(val) = line.strip_prefix("duration=") {
                duration = val.parse::<f64>().unwrap_or(0.0);
            }
        }

        let actual_duration = duration - start_time;

        if actual_duration > 0.0 && actual_duration <= 3.0 {
            actual_duration
        } else {
            2.0
        }
    } else {
        log::warn!("[Utils] ffprobe not found at {:?}, falling back to default duration", ffprobe);
        2.0
    };

    cache.lock().insert(file_path.to_path_buf(), result);
    result
}

fn parse_segment_index(name: &str) -> Option<u64> {
    let stem = name.strip_prefix("seg_")?;
    let first_part = stem.split('_').next().unwrap_or(stem);
    first_part.parse::<u64>().ok()
}

fn parse_segment_session_id(name: &str) -> Option<u64> {
    let stem = name.strip_prefix("seg_")?;
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() >= 2 {
        let sid_part = parts[1];
        let sid_str = sid_part.split('.').next().unwrap_or(sid_part);
        sid_str.parse::<u64>().ok()
    } else {
        None
    }
}

/// Sum all segment durations from filenames to compute total accumulated time.
/// This is the source of truth for decode-time-offset — matches the test pipeline's approach.
/// Falls back to ffprobe for unrenamed segments to ensure accuracy during active/crashed sessions.
pub fn compute_total_duration(game_dir: &Path) -> f64 {
    let mut total = 0.0;
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "m4s") {
                if let Some(dur) = get_segment_duration(&path) {
                    total += dur;
                } else if let Some(dur) = ffprobe_segment_duration(&path) {
                    // Fallback to ffprobe for unrenamed segments (active or crashed session)
                    total += dur;
                }
            }
        }
    }
    total
}

/// Get actual segment duration from packet-level PTS using ffprobe.
/// format.duration lies for fMP4 segments — only packet analysis gives the true duration.
fn ffprobe_segment_duration(path: &Path) -> Option<f64> {
    let ffprobe = get_ffprobe_path();
    let output = std::process::Command::new(&ffprobe)
        .args(["-v", "error", "-select_streams", "v:0",
               "-show_entries", "packet=pts_time,duration_time",
               "-of", "csv=p=0"])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut first_pts: Option<f64> = None;
    let mut last_pts: Option<f64> = None;
    let mut last_frame_dur: f64 = 0.0;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let Ok(pts) = parts[0].parse::<f64>() {
                if first_pts.is_none() {
                    first_pts = Some(pts);
                }
                last_pts = Some(pts);
                if let Ok(dur) = parts[1].parse::<f64>() {
                    last_frame_dur = dur;
                }
            }
        }
    }

    let start = first_pts?;
    let end = last_pts? + last_frame_dur;
    let dur = end - start;
    if dur > 0.0 { Some(dur) } else { None }
}

/// Post-pipeline fixup for EOS segments.
/// After pipeline is fully stopped, find unrenamed segments and rename them
/// using ffprobe, then correct segments where bus gave wrong duration.
pub fn fixup_eos_segments(game_dir: &Path) {
    // Step 1: Rename segments that never got their duration suffix
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        let mut unrenamed: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |e| e == "m4s") && {
                    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    name.starts_with("seg_") && !name.contains("ms")
                }
            })
            .collect();

        unrenamed.sort();

        for path in unrenamed {
            let stem = match path.file_stem() {
                Some(s) => s.to_string_lossy().to_string(),
                None => continue,
            };
            let display_name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            if let Some(dur_secs) = ffprobe_segment_duration(&path) {
                let dur_ms = (dur_secs * 1000.0).round() as u64;
                let mut new_path = path.clone();
                new_path.set_file_name(format!("{}_{}ms.m4s", stem, dur_ms));
                let new_display = new_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                if let Err(e) = std::fs::rename(&path, &new_path) {
                    log::warn!("[SegmentFixup] Failed to rename {}: {}", display_name, e);
                } else {
                    log::info!("[SegmentFixup] Renamed {} -> {} (ffprobe: {:.3}s)",
                        display_name, new_display, dur_secs);
                }
            } else {
                log::warn!("[SegmentFixup] ffprobe failed for {}", display_name);
            }
        }
    }

    // Step 2: Correct segments where bus gave wrong duration (EOS truncation)
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        let mut segments: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |e| e == "m4s") && {
                    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    name.starts_with("seg_") && name.contains("ms")
                }
            })
            .collect();

        segments.sort();

        for path in segments {
            let stem = match path.file_stem() {
                Some(s) => s.to_string_lossy().to_string(),
                None => continue,
            };
            let display_name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let filename_dur = get_segment_duration(&path);
            let actual_dur = ffprobe_segment_duration(&path);

            if let (Some(f_dur), Some(a_dur)) = (filename_dur, actual_dur) {
                let delta_ms = (f_dur - a_dur).abs() * 1000.0;
                if delta_ms > 100.0 {
                    let base = stem.rsplitn(2, '_').last().unwrap_or(&stem);
                    let correct_ms = (a_dur * 1000.0).round() as u64;
                    let mut new_path = path.clone();
                    new_path.set_file_name(format!("{}_{}ms.m4s", base, correct_ms));
                    let new_display = new_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    if let Err(e) = std::fs::rename(&path, &new_path) {
                        log::warn!("[SegmentFixup] Duration fix failed for {}: {}", display_name, e);
                    } else {
                        log::info!("[SegmentFixup] Duration fix: {} -> {} (was {:.0}ms, actual {:.0}ms)",
                            display_name, new_display, f_dur * 1000.0, a_dur * 1000.0);
                    }
                }
            }
        }
    }
}

fn get_segment_duration(path: &Path) -> Option<f64> {
    let name = path.file_name()?.to_string_lossy();
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

/// Segment entry after deduplication: (modified_time, path, duration_secs, session_id)
type SegmentEntry = (std::time::SystemTime, std::path::PathBuf, f64, Option<u64>);

/// Scan a game directory for .m4s segments, deduplicate by index (keep most recent).
/// Returns a BTreeMap sorted by segment index.
fn scan_segments(game_dir: &Path) -> std::collections::BTreeMap<u64, SegmentEntry> {
    let mut segment_map: std::collections::BTreeMap<u64, SegmentEntry> = std::collections::BTreeMap::new();
    let entries = match std::fs::read_dir(game_dir) {
        Ok(e) => e,
        Err(_) => return segment_map,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.extension().map_or(false, |ext| ext == "m4s") { continue; }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let duration = match get_segment_duration(&path).or_else(|| ffprobe_segment_duration(&path)) {
            Some(d) => d,
            None => continue,
        };
        let modified = entry.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now());
        let Some(idx) = parse_segment_index(&name) else { continue };
        let sid = parse_segment_session_id(&name);
        match segment_map.entry(idx) {
            std::collections::btree_map::Entry::Vacant(e) => { e.insert((modified, path, duration, sid)); }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                if modified > e.get().0 {
                    e.insert((modified, path, duration, sid));
                }
            }
        }
    }
    segment_map
}

fn game_dir_for(game_title: &str) -> PathBuf {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    get_storage_root().join(safe_title)
}

pub fn generate_session_playlist(game_title: &str, _active_session_id: Option<u64>) -> Option<(PathBuf, Vec<crate::state::SessionBlock>)> {
    let game_dir = game_dir_for(game_title);
    if !game_dir.exists() { return None; }

    let segment_map = scan_segments(&game_dir);
    if segment_map.is_empty() { return None; }

    let total_duration: f64 = segment_map.values().map(|s| s.2).sum();
    let session_blocks = vec![crate::state::SessionBlock {
        start_timestamp: 0,
        duration_secs: total_duration,
        timeline_offset_secs: 0.0,
        playlist_path: game_dir.join("master.m3u8"),
    }];

    let master_playlist = generate_master_playlist(game_title);
    Some((master_playlist?, session_blocks))
}

pub fn generate_master_playlist(game_title: &str) -> Option<PathBuf> {
    let game_dir = game_dir_for(game_title);
    if !game_dir.exists() { return None; }

    let segment_map = scan_segments(&game_dir);
    if segment_map.is_empty() { return None; }

    let master_playlist_path = game_dir.join("master.m3u8");
    let mut m3u8 = String::from("#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:EVENT\n");

    let mut last_session_id: Option<u64> = None;
    for (_, (_modified, path, duration, session_id)) in &segment_map {
        let file_name = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };
        if last_session_id.is_some() && *session_id != last_session_id {
            m3u8.push_str("#EXT-X-DISCONTINUITY\n");
        }
        last_session_id = *session_id;
        m3u8.push_str(&format!("#EXTINF:{:.3},\n{}\n", duration, file_name));
    }
    m3u8.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&master_playlist_path, m3u8);

    Some(master_playlist_path)
}

#[allow(dead_code)]
pub fn generate_ffconcat_playlist(game_title: &str) -> Option<PathBuf> {
    let game_dir = game_dir_for(game_title);
    if !game_dir.exists() { return None; }

    let segment_map = scan_segments(&game_dir);
    if segment_map.is_empty() { return None; }

    let ffconcat_path = game_dir.join("view.ffconcat");
    let mut content = String::from("ffconcat version 1.0\n");
    for (_, (_, path, duration, _)) in &segment_map {
        let file_name = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };
        content.push_str(&format!("file '{}'\nduration {}\n", file_name, duration));
    }

    std::fs::write(&ffconcat_path, content).ok()?;
    Some(ffconcat_path)
}


fn resolve_steam_app_id(game_title: &str) -> Option<String> {
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        url::form_urlencoded::byte_serialize(game_title.as_bytes()).collect::<String>()
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .user_agent("Rekaptr/1.0")
        .build()
        .ok()?;

    let json = client.get(&url).send().ok()?.json::<serde_json::Value>().ok()?;
    let id = json.get("items")?.as_array()?.first()?.get("id")?.as_i64()?;
    log::info!("[Utils] Found Steam AppID {} for '{}' via search API", id, game_title);
    Some(id.to_string())
}

fn resolve_steam_artwork(game_title: &str, cdn_filename: &str, cache_suffix: &str) -> Option<String> {
    let app_id = resolve_steam_app_id(game_title)?;

    let cache_dir = get_storage_root().join("Cache").join("Artwork");
    let _ = std::fs::create_dir_all(&cache_dir);

    // Check for cached file with any common image extension
    for ext in &["webp", "png", "jpg"] {
        let local_path = cache_dir.join(format!("{}_{}.{}", app_id, cache_suffix, ext));
        if local_path.exists() {
            if let Ok(meta) = local_path.metadata() {
                if meta.len() > 5000 {
                    let path_str = local_path.to_string_lossy().replace('\\', "/");
                    return Some(path_str);
                } else {
                    let _ = std::fs::remove_file(&local_path);
                }
            }
        }
    }

    let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/{}", app_id, cdn_filename);
    Some(url)
}

/// Returns a landscape hero image URL/path (used on dashboard).
pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    resolve_steam_artwork(game_title, "library_hero.jpg", "hero")
}

/// Returns a portrait library capsule URL/path (used on clips page game cards).
pub fn find_steam_artwork_portrait(game_title: &str) -> Option<String> {
    resolve_steam_artwork(game_title, "library_600x900.jpg", "portrait")
}

/// Returns a game logo (transparent PNG) URL/path.
pub fn find_steam_logo(game_title: &str) -> Option<String> {
    resolve_steam_artwork(game_title, "logo.png", "logo")
}
pub fn start_buffer_cleanup_thread(root_dir: PathBuf) {
    use notify::{RecursiveMode, Watcher, Config};
    use std::sync::mpsc::channel;

    const CLEANUP_POLL_INTERVAL_SECS: u64 = 30;

    std::thread::Builder::new()
        .name("Rekaptr Cleanup".to_string())
        .spawn(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Threading::*;
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
            }

            let (tx, rx) = channel();
            let mut watcher = match notify::RecommendedWatcher::new(tx, Config::default()) {
                Ok(w) => w,
                Err(e) => {
                    log::error!("[Cleanup] Failed to create file watcher: {}. Falling back to polling.", e);
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(CLEANUP_POLL_INTERVAL_SECS));
                    }
                }
            };
            let _ = watcher.watch(&root_dir, RecursiveMode::Recursive);

            loop {
                // 1. Wait for the first event (completely idle until then)
                match rx.recv() {
                    Ok(_) => {
                        // Debounce: let simultaneous segment closures finish before scanning.
                        const DEBOUNCE_SECS: u64 = 5;
                        std::thread::sleep(std::time::Duration::from_secs(DEBOUNCE_SECS));
                        
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
            Some(f) => f.to_string_lossy(),
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
                                            total_game_duration += get_segment_duration(path).unwrap_or(DEFAULT_SEGMENT_DURATION_SECS);
                                        }
                                        
                                        for (path, _, _) in game_segments.iter() {
                                            if total_game_duration <= retention_secs { break; }
                                            let dur = get_segment_duration(path).unwrap_or(DEFAULT_SEGMENT_DURATION_SECS);
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
                        log::warn!("[Cleanup] Watcher error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    }
                }
            }
        }).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_clean_title_basic() {
        assert_eq!(clean_title("Counter-Strike 2"), "counterstrike2");
        assert_eq!(clean_title("Elden Ring"), "eldenring");
        assert_eq!(clean_title("DOOM (2016)"), "doom2016");
    }

    #[test]
    fn test_clean_title_special_chars() {
        assert_eq!(clean_title(""), "");
        assert_eq!(clean_title("!!!"), "");
        assert_eq!(clean_title("A"), "a");
        assert_eq!(clean_title("Test 123"), "test123");
    }

    #[test]
    fn test_parse_segment_index_valid() {
        assert_eq!(parse_segment_index("seg_0_6000ms.m4s"), Some(0));
        assert_eq!(parse_segment_index("seg_42_2000ms.m4s"), Some(42));
        assert_eq!(parse_segment_index("seg_100_1500ms.m4s"), Some(100));
    }

    #[test]
    fn test_parse_segment_index_no_duration() {
        // Segments that haven't been renamed yet (no _XXXms suffix)
        // Format is seg_INDEX_SESSIONID.m4s
        assert_eq!(parse_segment_index("seg_5_1234567890.m4s"), Some(5));
        assert_eq!(parse_segment_index("seg_0_1234567890.m4s"), Some(0));
    }

    #[test]
    fn test_parse_segment_index_invalid() {
        assert_eq!(parse_segment_index("not_a_segment.m4s"), None);
        assert_eq!(parse_segment_index("video.mp4"), None);
        assert_eq!(parse_segment_index(""), None);
        assert_eq!(parse_segment_index("seg_abc_100ms.m4s"), None);
    }

    #[test]
    fn test_get_segment_duration_from_filename() {
        let path = Path::new("seg_0_6000ms.m4s");
        assert_eq!(get_segment_duration(path), Some(6.0));

        let path = Path::new("seg_5_1500ms.m4s");
        assert_eq!(get_segment_duration(path), Some(1.5));

        let path = Path::new("seg_10_500ms.m4s");
        assert_eq!(get_segment_duration(path), Some(0.5));
    }

    #[test]
    fn test_get_segment_duration_no_duration_in_name() {
        let path = Path::new("seg_0.m4s");
        assert_eq!(get_segment_duration(path), None);

        let path = Path::new("video.mp4");
        assert_eq!(get_segment_duration(path), None);
    }

    #[test]
    fn test_compute_total_duration_empty_dir() {
        let dir = std::env::temp_dir().join("rekaptr_test_empty_dur");
        let _ = fs::create_dir_all(&dir);
        assert_eq!(compute_total_duration(&dir), 0.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_total_duration_with_segments() {
        let dir = std::env::temp_dir().join("rekaptr_test_total_dur");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        // Create fake segment files with duration in filename
        fs::write(dir.join("seg_0_6000ms.m4s"), b"fake").unwrap();
        fs::write(dir.join("seg_1_6000ms.m4s"), b"fake").unwrap();
        fs::write(dir.join("seg_2_3000ms.m4s"), b"fake").unwrap();

        let total = compute_total_duration(&dir);
        assert!((total - 15.0).abs() < 0.001, "Expected 15.0, got {}", total);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_total_duration_nonexistent_dir() {
        let dir = Path::new("C:\\nonexistent_rekaptr_test_dir_12345");
        assert_eq!(compute_total_duration(dir), 0.0);
    }
}
