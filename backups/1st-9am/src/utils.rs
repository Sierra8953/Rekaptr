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

pub fn generate_session_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    let root = get_storage_root();
    let game_dir = root.join(&safe_title);
    
    if !game_dir.exists() {
        return None;
    }

    let mut segments = Vec::new();
    
    // Recursive search for segment_*.mkv
    fn find_segments(dir: &Path, segments: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    find_segments(&path, segments);
                } else if path.extension().map_or(false, |ext| ext == "mkv") {
                    let name = path.file_name().unwrap().to_string_lossy();
                    if name.starts_with("segment_") {
                        segments.push(path);
                    }
                }
            }
        }
    }

    find_segments(&game_dir, &mut segments);

    if segments.is_empty() {
        eprintln!("[Utils] No segments found in {:?}", game_dir);
        return None;
    }

    // Sort by path to ensure session_TIMESTAMP and segment_INDEX are in order
    segments.sort();

    // 1. Write M3U8 (for some players)
    let m3u8_path = game_dir.join("view.m3u8");
    let mut m3u8_content = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-MEDIA-SEQUENCE:0\n");
    for seg in &segments {
        m3u8_content.push_str("#EXTINF:10.0,\n"); // Approximate
        let rel_path = seg.strip_prefix(&game_dir).unwrap_or(seg).to_string_lossy().replace('\\', "/");
        m3u8_content.push_str(&rel_path);
        m3u8_content.push_str("\n");
    }
    m3u8_content.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&m3u8_path, m3u8_content);

    // 2. Write FFCONCAT (for FFmpeg exporting)
    let ffconcat_path = game_dir.join("view.ffconcat");
    let mut ffconcat_content = String::from("ffconcat version 1.0\n");
    for seg in &segments {
        ffconcat_content.push_str("file '");
        let rel_path = seg.strip_prefix(&game_dir).unwrap_or(seg).to_string_lossy().replace('\\', "/");
        ffconcat_content.push_str(&rel_path);
        ffconcat_content.push_str("'\n");
    }
    let _ = std::fs::write(&ffconcat_path, ffconcat_content);

    // 3. Write EDL (for MPV seeking)
    let edl_path = game_dir.join("view.edl");
    let mut edl_content = String::from("# mpv EDL v0\n");
    for seg in &segments {
        let abs_path = seg.to_string_lossy().replace('\\', "/");
        edl_content.push_str(&format!("{},length=2.0\n", abs_path)); 
    }
    let _ = std::fs::write(&edl_path, edl_content);

    eprintln!("[Utils] Wrote playlists (M3U8, FFCONCAT, EDL) to: {:?}", game_dir);

    if edl_path.exists() {
        return Some(edl_path);
    }
    None
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
                                            if seg.path().extension().map_or(false, |ex| ex == "mkv") {
                                                if let Ok(meta) = seg.metadata() {
                                                    total_size += meta.len();
                                                    segments.push((seg.path(), meta.modified().unwrap()));
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
