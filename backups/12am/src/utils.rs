use crate::config::AppConfig;
use regex::Regex;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use wasapi::{DeviceCollection, Direction};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

pub fn get_audio_devices() -> Vec<(String, String)> {
    let mut devices = vec![("Default".to_string(), "Default".to_string())];
    println!("[Audio] Scanning for Capture Devices...");
    if let Ok(collection) = DeviceCollection::new(&Direction::Capture) {
        if let Ok(count) = collection.get_nbr_devices() {
            for i in 0..count {
                if let Ok(device) = collection.get_device_at_index(i) {
                    if let (Ok(name), Ok(id)) = (
                        device.get_friendlyname(),
                        device.get_id(),
                    ) {
                        println!("[Audio] Found Device: '{}' (ID: '{}')", name, id);
                        devices.push((name, id));
                    }
                }
            }
        }
    }
    println!("[Audio] Scan Complete. Found {} devices.", devices.len());
    devices
}

pub fn get_gpu_list() -> Vec<String> {
    let mut gpus = Vec::new();
    unsafe {
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            let mut i = 0;
            while let Ok(adapter) = factory.EnumAdapters1(i) {
                if let Ok(desc) = adapter.GetDesc1() {
                    let name = String::from_utf16_lossy(&desc.Description);
                    let name = name.trim_matches(char::from(0)).to_string();
                    gpus.push(name);
                }
                i += 1;
            }
        }
    }
    if gpus.is_empty() {
        gpus.push("Default Adapter".to_string());
    }
    gpus
}

pub fn get_storage_root() -> PathBuf {
    let path = PathBuf::from("E:\\LumaRecordings");
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    path
}

pub fn get_ffmpeg_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        let local_ffmpeg = exe_path.parent().unwrap().join("ffmpeg.exe");
        let root_ffmpeg = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ffmpeg.exe");
        let bin_ffmpeg = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("bin")
            .join("ffmpeg.exe");
        let bin_adj_ffmpeg = exe_path.parent().unwrap().join("bin").join("ffmpeg.exe");

        if local_ffmpeg.exists() {
            local_ffmpeg
        } else if bin_adj_ffmpeg.exists() {
            bin_adj_ffmpeg
        } else if bin_ffmpeg.exists() {
            bin_ffmpeg
        } else if root_ffmpeg.exists() {
            root_ffmpeg
        } else {
            PathBuf::from("ffmpeg")
        }
    } else {
        PathBuf::from("ffmpeg")
    }
}

pub fn format_time(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{}:{:02}:{:02}", h, m, s)
}

pub fn clean_title(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

pub fn get_session_segments(game_title: &str) -> Vec<PathBuf> {
    let safe_title = clean_title(game_title);
    let storage_root = get_storage_root();
    let game_dir = storage_root.join(&safe_title);
    
    eprintln!("[Utils] Scanning for segments in: {:?}", game_dir);
    
    if !game_dir.exists() {
        eprintln!("[Utils] Directory does not exist: {:?}", game_dir);
        return Vec::new();
    }

    let mut sessions: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with("session_")
            {
                sessions.push(entry.path());
            }
        }
    }
    sessions.sort_by_key(|p| p.file_name().unwrap().to_string_lossy().to_string());
    
    eprintln!("[Utils] Found {} recording sessions", sessions.len());
    
    let mut all_segments: Vec<PathBuf> = Vec::new();
    for session_path in sessions {
        let mut segments: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&session_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".mkv") && !name.starts_with("temp_") {
                    segments.push(entry.path());
                }
            }
        }
        segments.sort_by_key(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            format!("1_{}", name)
        });
        all_segments.extend(segments);
    }
    
    eprintln!("[Utils] Gathered total of {} segments", all_segments.len());
    all_segments
}

pub fn generate_session_playlist(game_title: &str) -> Option<PathBuf> {
    let safe_title = clean_title(game_title);
    let game_dir = get_storage_root().join(&safe_title);
    let playlist_path = game_dir.join("view.m3u8");
    let edl_path = game_dir.join("view.edl");
    
    eprintln!("[Utils] Generating playlist for: {} (Dir: {:?})", game_title, game_dir);
    
    if !game_dir.exists() {
        let _ = std::fs::create_dir_all(&game_dir);
    }

    let mut sessions: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with("session_")
            {
                sessions.push(entry.path());
            }
        }
    }
    sessions.sort_by_key(|p| p.file_name().unwrap().to_string_lossy().to_string());
    
    eprintln!("[Utils] Found {} session folders", sessions.len());
    
    let mut all_segments: Vec<PathBuf> = Vec::new();
    for session_path in sessions {
        let mut segments: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&session_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                // Exclude temp_ segments as they are for engine warmup and get deleted
                if name.ends_with(".mkv") && !name.starts_with("temp_") {
                    segments.push(entry.path());
                }
            }
        }
        segments.sort_by_key(|p| p.file_name().unwrap().to_string_lossy().to_string());
        all_segments.extend(segments);
    }

    eprintln!("[Utils] Total segments gathered: {}", all_segments.len());

    if all_segments.is_empty() {
        eprintln!("[Utils] No segments found, skipping playlist generation");
        return None;
    }

    use std::io::Write;

    // 1. Generate M3U8 in memory
    let mut m3u8_content = String::with_capacity(all_segments.len() * 100);
    m3u8_content.push_str("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n");
    
    // 2. Generate FFCONCAT in memory
    let mut concat_content = String::with_capacity(all_segments.len() * 100);
    concat_content.push_str("ffconcat version 1.0\n");

    // 3. Generate EDL in memory (for MPV virtual timeline)
    let mut edl_content = String::with_capacity(all_segments.len() * 100);
    edl_content.push_str("# mpv EDL v0\n");

    let seg_count = all_segments.len();
    for (i, seg) in all_segments.iter().enumerate() {
        let seg_str = seg.to_string_lossy().replace('\\', "/");
        
        // EDL includes all segments for a continuous timeline
        edl_content.push_str(&format!("{},length=2.0\n", seg_str));

        // Skip the very last segment for streaming playlists if we are likely still recording
        if i == seg_count - 1 && seg_count > 1 {
            continue;
        }
        
        if let Ok(rel_path) = seg.strip_prefix(&game_dir) {
            let rel_str = rel_path.to_string_lossy().replace('\\', "/");
            
            // M3U8
            m3u8_content.push_str(&format!("#EXTINF:2.0,\n{}\n", rel_str));
            
            // FFCONCAT
            concat_content.push_str(&format!("file '{}'\nduration 2.0\n", rel_str));
        }
    }
    m3u8_content.push_str("#EXT-X-ENDLIST\n");

    // Atomic-ish writes
    if let Ok(mut file) = std::fs::File::create(&playlist_path) {
        let _ = file.write_all(m3u8_content.as_bytes());
    }
    
    let concat_path = game_dir.join("view.ffconcat");
    if let Ok(mut file) = std::fs::File::create(&concat_path) {
        let _ = file.write_all(concat_content.as_bytes());
    }

    if let Ok(mut file) = std::fs::File::create(&edl_path) {
        let _ = file.write_all(edl_content.as_bytes());
    }

    eprintln!("[Utils] Wrote playlists (M3U8, FFCONCAT, EDL) to: {:?}", game_dir);

    if edl_path.exists() {
        return Some(edl_path);
    }
    None
}

pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    let clean_target = clean_title(game_title);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam_path: PathBuf = match hkcu.open_subkey("Software\\Valve\\Steam") {
        Ok(key) => {
            let s: String = key.get_value("SteamPath").unwrap_or_default();
            PathBuf::from(s.replace("/", "\\"))
        }
        Err(_) => return None,
    };

    let library_vdf = steam_path.join("steamapps").join("libraryfolders.vdf");
    if !library_vdf.exists() {
        return None;
    }

    let content = std::fs::read_to_string(library_vdf).unwrap_or_default();
    let re = Regex::new(r#""path"\s+"(.+?)""#).unwrap();

    let mut paths = vec![steam_path.clone()];
    for cap in re.captures_iter(&content) {
        paths.push(PathBuf::from(cap[1].replace("\\\\", "\\")));
    }

    let mut app_id = None;
    for path in paths {
        let apps_dir = path.join("steamapps");
        if let Ok(entries) = std::fs::read_dir(apps_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("appmanifest_") && name.ends_with(".acf") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        let name_re = Regex::new(r#""name"\s+"(.+?)""#).unwrap();
                        if let Some(cap) = name_re.captures(&content) {
                            if clean_title(&cap[1]) == clean_target {
                                let id_re = Regex::new(r#""appid"\s+"(\d+)""#).unwrap();
                                if let Some(id_cap) = id_re.captures(&content) {
                                    app_id = Some(id_cap[1].to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        if app_id.is_some() {
            break;
        }
    }

    if let Some(id) = app_id {
        let cache_dir = steam_path.join("appcache").join("librarycache");
        let luma_cache = get_storage_root().join("Cache").join("Artwork");
        if !luma_cache.exists() {
            let _ = std::fs::create_dir_all(&luma_cache);
        }

        let legacy_candidates = [
            format!("{}_header.jpg", id),
            format!("{}_library_600x900.jpg", id),
        ];

        for filename in legacy_candidates {
            let source = cache_dir.join(&filename);
            if source.exists() {
                let dest = luma_cache.join(&filename);
                if std::fs::copy(&source, &dest).is_ok() {
                    return Some(dest.to_string_lossy().replace('\\', "/"));
                }
            }
        }

        let app_dir = cache_dir.join(&id);
        if app_dir.exists() && app_dir.is_dir() {
            let mut found_path = None;
            let mut dirs = vec![app_dir];
            while let Some(dir) = dirs.pop() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            dirs.push(path);
                        } else if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                            if fname == "library_hero_blur.jpg" {
                                found_path = Some(path);
                                break;
                            }
                            if (fname == "library_hero.jpg"
                                || fname == "library_header.jpg"
                                || fname == "header.jpg")
                                && found_path.is_none()
                            {
                                found_path = Some(path.clone());
                            }
                            if fname == "library_600x900.jpg" && found_path.is_none() {
                                found_path = Some(path.clone());
                            }
                        }
                    }
                }
                if found_path
                    .as_ref()
                    .map(|p| p.file_name().unwrap() == "library_hero_blur.jpg")
                    .unwrap_or(false)
                {
                    break;
                }
            }

            if let Some(source) = found_path {
                let dest = luma_cache.join(format!("{}_header.jpg", id));
                if std::fs::copy(&source, &dest).is_ok() {
                    return Some(dest.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
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
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST);
            }

            let (tx, rx) = channel();

            // Setup watcher
            let mut watcher = notify::RecommendedWatcher::new(tx, Config::default()).expect("Failed to create watcher");
            let _ = watcher.watch(&root_dir, RecursiveMode::Recursive);

            eprintln!("[Cleanup] Reactive watcher started for: {:?}", root_dir);

            // Initial cleanup pass
            perform_full_cleanup(&root_dir);

            let mut last_cleanup = std::time::Instant::now() - Duration::from_secs(60);

            // Event loop
            for res in rx {
                match res {
                    Ok(event) => {
                        // We only care about file creations/writes for .mkv files
                        let should_trigger = event.paths.iter().any(|p| {
                            p.extension().map_or(false, |ext| ext == "mkv")
                        });

                        if should_trigger {
                            // Throttle cleanup to once every 5 seconds per event stream
                            if last_cleanup.elapsed() > Duration::from_secs(5) {
                                perform_full_cleanup(&root_dir);
                                last_cleanup = std::time::Instant::now();
                            }
                        }
                    }
                    Err(e) => eprintln!("[Cleanup] Watcher error: {:?}", e),
                }
            }
        })
        .expect("Failed to spawn cleanup thread");
}

fn perform_full_cleanup(root_dir: &PathBuf) {
    let cfg = AppConfig::load();
    if let Ok(entries) = std::fs::read_dir(root_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let folder_name = entry.file_name().to_string_lossy().to_string();
            if folder_name == "Clips"
                || folder_name == "Cache"
                || folder_name.starts_with(".")
            {
                continue;
            }

            let folder_name_lower = folder_name.to_lowercase();
            let mut retention_mins = cfg.global_video.retention_minutes;
            let mut match_found = false;

            if folder_name_lower == "desktop" || folder_name_lower == "monitor" {
                retention_mins = cfg.global_video.retention_minutes;
                match_found = true;
            } else {
                for (title, settings) in &cfg.game_registry {
                    if clean_title(title) == folder_name_lower {
                        retention_mins = settings.retention_minutes;
                        match_found = true;
                        break;
                    }
                }
            }

            if !match_found {
                retention_mins = 60;
            }
            if retention_mins <= 0 {
                retention_mins = 10;
            }

            let max_segments = (retention_mins * 60) / 2;
            let mut all_segments: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

            if let Ok(sessions_result) = std::fs::read_dir(&path) {
                for sess in sessions_result.filter_map(|e| e.ok()) {
                    if !sess.path().is_dir() {
                        continue;
                    }
                    if let Ok(segs) = std::fs::read_dir(sess.path()) {
                        for s in segs.filter_map(|e| e.ok()) {
                            if s.path().extension().map_or(false, |ext| ext == "mkv") {
                                let mt = std::fs::metadata(s.path())
                                    .and_then(|m| m.modified())
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                all_segments.push((s.path(), mt));
                            }
                        }
                    }
                }
            }

            if all_segments.len() > max_segments as usize {
                all_segments.sort_by_key(|k| k.1);
                let to_delete = all_segments.len() - max_segments as usize;
                let mut deleted_any = false;
                for i in 0..to_delete {
                    if std::fs::remove_file(&all_segments[i].0).is_ok() {
                        deleted_any = true;
                    }
                }

                // Regenerate playlist after cleanup if we deleted anything
                if deleted_any {
                    generate_session_playlist(&folder_name);
                }
            }
        }
    }
}

pub fn fetch_all_clips() -> Vec<crate::state::Clip> {
    let mut clips = Vec::new();
    let clips_root = get_storage_root().join("Clips");
    
    if !clips_root.exists() {
        return clips;
    }

    if let Ok(game_dirs) = std::fs::read_dir(clips_root) {
        for game_entry in game_dirs.flatten() {
            if let Ok(file_type) = game_entry.file_type() {
                if file_type.is_dir() {
                    let game_title = game_entry.file_name().to_string_lossy().to_string();
                    if let Ok(files) = std::fs::read_dir(game_entry.path()) {
                        for file_entry in files.flatten() {
                            let path = file_entry.path();
                            if path.extension().map_or(false, |ext| ext == "mkv") {
                                if let Ok(metadata) = file_entry.metadata() {
                                    let size_mb = metadata.len() / (1024 * 1024);
                                    let modified = metadata.modified().unwrap_or(std::time::SystemTime::now());
                                    let timestamp = modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                                    
                                    // Format date
                                    let datetime: chrono::DateTime<chrono::Local> = modified.into();
                                    let date_str = datetime.format("%Y-%m-%d %H:%M").to_string();

                                    clips.push(crate::state::Clip {
                                        title: game_title.clone(),
                                        path,
                                        date: date_str,
                                        size: format!("{} MB", size_mb),
                                        timestamp,
                                    });
                                }
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
