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

    if let Ok(entries) = std::fs::read_dir(&clips_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // Game folder
                let game_title = entry.file_name().to_string_lossy().to_string().replace('_', " ");
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

pub fn generate_session_playlist(game_title: &str) -> Option<(PathBuf, Vec<crate::state::SessionBlock>)> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    // 1. Scan disk for all segments to find ground-truth sessions and durations
    let mut session_groups: std::collections::BTreeMap<u64, Vec<PathBuf>> = std::collections::BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.starts_with("seg_") {
                    let parts: Vec<&str> = name.split('_').collect();
                    if parts.len() >= 3 {
                        if let Ok(session_id) = parts[1].parse::<u64>() {
                            session_groups.entry(session_id).or_default().push(path);
                        }
                    }
                }
            }
        }
    }

    if session_groups.is_empty() { return None; }

    // 2. Sync scanned data to DB and build session_data list
    let mut session_data = Vec::new();
    if let Ok(db) = crate::db::GameDatabase::open(&game_dir) {
        for (session_id, mut segments) in session_groups {
            let _ = db.register_session(session_id);
            segments.sort();
            
            // Calculate exact duration by summing every segment
            let mut total_duration = 0.0;
            for seg in &segments {
                total_duration += get_exact_duration(seg);
            }
            
            let _ = db.update_duration(session_id, total_duration);
            session_data.push((session_id, total_duration));
        }
    }

    if session_data.is_empty() { return None; }
    session_data.sort_by_key(|s| s.0);

    // 3. Build session blocks for the timeline from the ground-truth data
    let mut session_blocks = Vec::new();
    let mut current_timeline_offset = 0.0;

    for (session_id, duration) in session_data {
        session_blocks.push(crate::state::SessionBlock {
            start_timestamp: session_id,
            duration_secs: duration,
            timeline_offset_secs: current_timeline_offset,
            playlist_path: game_dir.join("master.m3u8"),
        });
        current_timeline_offset += duration;
    }

    if session_blocks.is_empty() { return None; }

    // Generate/Update the master playlist (scans disk to be accurate)
    let master_playlist = generate_master_playlist(game_title);

    Some((master_playlist?, session_blocks))
}

pub fn generate_master_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);

    if !game_dir.exists() { return None; }

    // 1. Find all segments and group them by session_id
    // Filename pattern: seg_{session_id}_{index}.m4s
    let mut session_groups: std::collections::BTreeMap<u64, Vec<PathBuf>> = std::collections::BTreeMap::new();

    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.starts_with("seg_") {
                    let parts: Vec<&str> = name.split('_').collect();
                    if parts.len() >= 3 {
                        if let Ok(session_id) = parts[1].parse::<u64>() {
                            session_groups.entry(session_id).or_default().push(path);
                        }
                    }
                }
            }
        }
    }

    if session_groups.is_empty() { return None; }

    let master_playlist_path = game_dir.join("master.m3u8");
    let mut m3u8_content = String::from("#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-DISCONTINUITY-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:EVENT\n");

    for (_session_idx, (_session_id, mut segments)) in session_groups.into_iter().enumerate() {
        segments.sort();

        // NO DISCONTINUITY needed for maximum smoothness
        
        let seg_count = segments.len();
        for (i, seg) in segments.iter().enumerate() {
            let file_name = seg.file_name().unwrap().to_string_lossy();

            // Assume 2.0s for speed, but precisely measure the final segment
            let mut duration = 2.000;
            if i == seg_count - 1 {
                duration = get_exact_duration(seg);
            }

            m3u8_content.push_str(&format!("#EXTINF:{:.3},\n", duration));
            m3u8_content.push_str(&format!("{}\n", file_name));
        }
    }
    m3u8_content.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&master_playlist_path, m3u8_content);

    Some(master_playlist_path)
}


pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    // Strictly strip everything except basic ASCII alphanumeric for matching
    let sanitized: String = game_title.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    
    let slug = sanitized.replace(' ', "");
    eprintln!("[Utils] find_steam_artwork for title: '{}' (slug: '{}')", game_title, slug);
    
    let app_id = match slug.as_str() {
        s if s.contains("counterstrike") || s.contains("cs2") || s.contains("csgo") => Some("730"),
        s if s.contains("dota") => Some("570"),
        s if s.contains("cyberpunk") => Some("1091500"),
        s if s.contains("halflife2") => Some("220"),
        s if s.contains("eldenring") => Some("1245620"),
        s if s.contains("baldursgate") => Some("1086940"),
        s if s.contains("arcraiders") => Some("1808500"),
        s if s.contains("monitor") => Some("1144200"),
        s if s.contains("thelastofus") => Some("1888500"),
        s if s.contains("vivaldi") => Some("1144200"),
        s if s.contains("valorant") => None,
        _ => None,
    };

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
                        // Check storage limits
                        let config = crate::config::AppConfig::load();
                        let max_bytes = config.max_buffer_size_gb as u64 * 1024 * 1024 * 1024;
                        
                        let mut total_size = 0;
                        let mut segments = Vec::new();

                        if let Ok(entries) = std::fs::read_dir(&root_dir) {
                            for entry in entries.filter_map(|e| e.ok()) {
                                if entry.path().is_dir() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if name == "Clips" || name == "Cache" { continue; }

                                    if let Ok(seg_entries) = std::fs::read_dir(entry.path()) {
                                        for seg in seg_entries.filter_map(|e| e.ok()) {
                                            let path = seg.path();
                                            if path.extension().map_or(false, |ex| ex == "m4s" || ex == "mp4" || ex == "ts") {
                                                let file_name = path.file_name().unwrap().to_string_lossy();
                                                
                                                // PROTECT INIT SEGMENTS: Never delete the 00000 fragments as they are HLS maps
                                                if file_name.ends_with("_00000.m4s") { continue; }

                                                if let Ok(meta) = seg.metadata() {
                                                    total_size += meta.len();
                                                    segments.push((path, meta.modified().unwrap_or(std::time::SystemTime::now())));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if total_size > max_bytes {
                            segments.sort_by(|a, b| a.1.cmp(&b.1));
                            let mut current_size = total_size;
                            for (path, _) in segments {
                                if current_size <= max_bytes { break; }
                                if let Ok(meta) = path.metadata() {
                                    current_size -= meta.len();
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
