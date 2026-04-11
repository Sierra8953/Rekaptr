use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const STARTUP_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_REG_VALUE: &str = "Luma";

pub fn set_startup_with_windows(enable: bool) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(STARTUP_REG_KEY, KEY_SET_VALUE) {
        Ok(key) => {
            if enable {
                let exe_path = std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("luma.exe"));
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
                    total_duration += get_segment_duration(&seg_path).unwrap_or(6.0);
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
    let first_part = first_part.strip_suffix(".m4s").unwrap_or(first_part);
    first_part.parse::<u64>().ok()
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

pub fn generate_session_playlist(game_title: &str, _active_session_id: Option<u64>) -> Option<(PathBuf, Vec<crate::state::SessionBlock>)> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    // 1. Scan disk for segments, deduplicate by index (keep most recent per index)
    let mut segment_map: std::collections::BTreeMap<u64, (std::time::SystemTime, std::path::PathBuf, f64)> = std::collections::BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if let Some(idx) = parse_segment_index(&name) {
                            match segment_map.entry(idx) {
                                std::collections::btree_map::Entry::Vacant(e) => { e.insert((modified, path, duration)); }
                                std::collections::btree_map::Entry::Occupied(mut e) => {
                                    if modified > e.get().0 {
                                        e.insert((modified, path, duration));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if segment_map.is_empty() { return None; }

    // 2. Calculate total duration from deduplicated segments
    let total_duration: f64 = segment_map.values().map(|s| s.2).sum();

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

pub fn generate_master_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    // Collect segments keyed by index, deduplicating by keeping the most recently modified file per index
    let mut segment_map: std::collections::BTreeMap<u64, (std::time::SystemTime, std::path::PathBuf, f64)> = std::collections::BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if let Some(idx) = parse_segment_index(&name) {
                            match segment_map.entry(idx) {
                                std::collections::btree_map::Entry::Vacant(e) => { e.insert((modified, path, duration)); }
                                std::collections::btree_map::Entry::Occupied(mut e) => {
                                    if modified > e.get().0 {
                                        e.insert((modified, path, duration));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if segment_map.is_empty() { return None; }

    // BTreeMap is already sorted by segment index
    let master_playlist_path = game_dir.join("master.m3u8");
    let mut m3u8_content = String::from("#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:EVENT\n");

    for (_, (_, path, duration)) in &segment_map {
        let file_name = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };
        m3u8_content.push_str(&format!("#EXTINF:{:.3},\n", duration));
        m3u8_content.push_str(&format!("{}\n", file_name));
    }
    
    m3u8_content.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&master_playlist_path, m3u8_content);

    Some(master_playlist_path)
}

pub fn generate_ffconcat_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    let mut segment_map: std::collections::BTreeMap<u64, (std::time::SystemTime, std::path::PathBuf, f64)> = std::collections::BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                if let Some(duration) = get_segment_duration(&path) {
                    if let Ok(meta) = entry.metadata() {
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::now());
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if let Some(idx) = parse_segment_index(&name) {
                            match segment_map.entry(idx) {
                                std::collections::btree_map::Entry::Vacant(e) => { e.insert((modified, path, duration)); }
                                std::collections::btree_map::Entry::Occupied(mut e) => {
                                    if modified > e.get().0 {
                                        e.insert((modified, path, duration));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if segment_map.is_empty() { return None; }

    let ffconcat_path = game_dir.join("view.ffconcat");
    let mut ffconcat_content = String::from("ffconcat version 1.0\n");

    for (_, (_, path, duration)) in &segment_map {
        let file_name = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };
        ffconcat_content.push_str(&format!("file '{}'\nduration {}\n", file_name, duration));
    }

    if let Ok(_) = std::fs::write(&ffconcat_path, ffconcat_content) {
        Some(ffconcat_path)
    } else {
        None
    }
}


pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    // Basic hardcoded fallbacks for very common generic names or misspellings
    let sanitized: String = game_title.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();

    let slug = sanitized.replace(' ', "");
    log::debug!("[Utils] find_steam_artwork for title: '{}' (slug: '{}')", game_title, slug);

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
                                log::info!("[Utils] Found Steam AppID {} for '{}' via search API", id, game_title);
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
                    log::debug!("[Utils] Using cached artwork: {}", path_str);
                    return Some(path_str);
                } else {
                    log::warn!("[Utils] Cached artwork for '{}' is too small ({}), ignoring", game_title, meta.len());
                    let _ = std::fs::remove_file(&local_path);
                }
            }
        }

        let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg", id);
        log::debug!("[Utils] Artwork not cached or broken, returning URL: {}", url);
        return Some(url);
    }

    log::debug!("[Utils] No Steam AppID found for '{}'", game_title);
    None
}
pub fn start_buffer_cleanup_thread(root_dir: PathBuf) {
    use notify::{RecursiveMode, Watcher, Config};
    use std::sync::mpsc::channel;

    std::thread::Builder::new()
        .name("Luma Cleanup".to_string())
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
                    // Fall back to polling every 30 seconds
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(30));
                        // Re-run cleanup logic would go here, but for now just wait
                    }
                }
            };
            let _ = watcher.watch(&root_dir, RecursiveMode::Recursive);

            loop {
                // 1. Wait for the first event (completely idle until then)
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
                        log::warn!("[Cleanup] Watcher error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    }
                }
            }
        }).ok();
}
