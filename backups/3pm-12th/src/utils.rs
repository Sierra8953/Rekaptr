use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

pub fn clean_title(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

pub fn get_storage_root() -> PathBuf {
    PathBuf::from("E:\\LumaRecordings")
}

pub fn get_ffmpeg_path() -> PathBuf {
    std::env::current_dir().unwrap().join("bin").join("ffmpeg.exe")
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
                let mut game_title = config.game_registry.iter()
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
                                    .unwrap()
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

fn get_segment_duration(path: &Path) -> Option<f64> {
    let name = path.file_name().unwrap().to_string_lossy();
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
        let file_name = path.file_name().unwrap().to_string_lossy();
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
        let file_name = path.file_name().unwrap().to_string_lossy();
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
            let mut watcher = notify::RecommendedWatcher::new(tx, Config::default()).unwrap();
            let _ = watcher.watch(&root_dir, RecursiveMode::Recursive);

            loop {
                match rx.recv() {
                    Ok(_) => {
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
                                                let file_name = path.file_name().unwrap().to_string_lossy();
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
                    Err(e) => eprintln!("Watcher error: {:?}", e),
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }).unwrap();
}
