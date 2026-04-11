use crate::config::AppConfig;
use regex::Regex;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use wasapi::{DeviceCollection, Direction};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use winreg::enums::*;
use winreg::RegKey;

pub fn get_audio_devices() -> Vec<(String, String)> {
    let mut devices = vec![("Default".to_string(), "Default".to_string())];
    if let Ok(collection) = DeviceCollection::new(&Direction::Capture) {
        let count = collection.get_nbr_devices().unwrap_or(0);
        for i in 0..count {
            if let Ok(device) = collection.get_device_at_index(i) {
                let name: String = device.get_friendlyname().unwrap_or_else(|_| "Unknown".to_string());
                let id: String = device.get_id().unwrap_or_else(|_| "Unknown".to_string());
                devices.push((name, id));
            }
        }
    }
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
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    let root = path.join("Recordings");
    if !root.exists() {
        let _ = std::fs::create_dir_all(&root);
    }
    root
}

pub fn get_ffmpeg_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    let bin_ffmpeg = path.join("bin").join("ffmpeg.exe");
    if bin_ffmpeg.exists() {
        bin_ffmpeg
    } else {
        PathBuf::from("ffmpeg.exe")
    }
}

pub fn format_time(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{}:{:02}:{:02}", h, m, s)
}

pub fn clean_title(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
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
    } else {
        size = path.metadata()?.len();
    }
    Ok(size)
}

fn get_exact_duration(path: &Path) -> f64 {
    let ffmpeg_path = get_ffmpeg_path();
    let output = std::process::Command::new(ffmpeg_path)
        .arg("-i").arg(path)
        .output();
    
    if let Ok(output) = output {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(pos) = stderr.find("Duration: ") {
            let dur_str = &stderr[pos + 10..pos + 21];
            let parts: Vec<&str> = dur_str.split(':').collect();
            if parts.len() == 3 {
                let h: f64 = parts[0].parse().unwrap_or(0.0);
                let m: f64 = parts[1].parse().unwrap_or(0.0);
                let s: f64 = parts[2].parse().unwrap_or(0.0);
                return h * 3600.0 + m * 60.0 + s;
            }
        }
    }
    2.0
}

pub fn fetch_all_clips() -> Vec<crate::state::Clip> {
    let mut clips = Vec::new();
    let storage_root = get_storage_root();
    let clips_dir = storage_root.join("Clips");
    
    if let Ok(game_entries) = std::fs::read_dir(clips_dir) {
        for game_entry in game_entries.filter_map(|e| e.ok()) {
            if game_entry.path().is_dir() {
                if let Ok(clip_entries) = std::fs::read_dir(game_entry.path()) {
                    for clip_entry in clip_entries.filter_map(|e| e.ok()) {
                        let path = clip_entry.path();
                        if path.extension().map_or(false, |ext| ext == "mp4") {
                            let title = path.file_stem().unwrap().to_string_lossy().to_string();
                            let metadata = path.metadata().unwrap();
                            let created_at = metadata.created().unwrap();
                            let timestamp = created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                            
                            let size_mb = metadata.len() / (1024 * 1024);
                            
                            let mut thumbnail_path = path.clone();
                            thumbnail_path.set_extension("jpg");
                            let thumb = if thumbnail_path.exists() {
                                Some(thumbnail_path)
                            } else {
                                None
                            };

                            clips.push(crate::state::Clip {
                                title,
                                path: path.clone(),
                                thumbnail_path: thumb,
                                date: format!("{:?}", created_at),
                                duration: "0:00".to_string(),
                                size: format!("{} MB", size_mb),
                                timestamp,
                            });
                        }
                    }
                }
            }
        }
    }
    
    clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    clips
}

pub fn clear_all_buffers() {
    let root = get_storage_root();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name != "Clips" && name != "Cache" && !name.starts_with(".") {
                    let _ = std::fs::remove_dir_all(path);
                }
            }
        }
    }
}

pub fn generate_session_playlist(game_title: &str) -> Option<(PathBuf, Vec<crate::state::SessionBlock>)> {
    let safe_title = clean_title(game_title);
    let game_dir = get_storage_root().join(&safe_title);
    
    if !game_dir.exists() { return None; }

    let mut session_blocks = Vec::new();
    let mut current_timeline_offset = 0.0;

    let mut sessions: Vec<_> = std::fs::read_dir(&game_dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy().starts_with("session_"))
        .collect();
    
    sessions.sort_by_key(|e| e.file_name());

    for entry in sessions {
        let session_dir = entry.path();
        let playlist_path = session_dir.join("session.m3u8");
        
        let mut segments: Vec<_> = std::fs::read_dir(&session_dir).ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "m4s"))
            .collect();
        
        segments.sort_by_key(|e| e.file_name());
        
        if segments.is_empty() { continue; }

        let mut m3u8_content = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n");
        let mut session_duration = 0.0;

        let seg_count = segments.len();
        for (i, seg_entry) in segments.iter().enumerate() {
            let seg = seg_entry.path();
            let file_name = seg.file_name().unwrap().to_string_lossy();
            
            if file_name == "segment_00000.m4s" {
                m3u8_content.push_str("#EXT-X-MAP:URI=\"segment_00000.m4s\"\n");
                continue;
            }

            let mut duration = 2.0;
            if i == seg_count - 1 {
                duration = get_exact_duration(&seg);
            }

            m3u8_content.push_str(&format!("#EXTINF:{:.3},\n", duration));
            m3u8_content.push_str(&file_name);
            m3u8_content.push_str("\n");

            session_duration += duration;
        }
        m3u8_content.push_str("#EXT-X-ENDLIST\n");
        let _ = std::fs::write(&playlist_path, m3u8_content);

        if session_duration > 0.0 {
            let dir_name = session_dir.file_name().unwrap().to_string_lossy();
            let start_timestamp = dir_name.strip_prefix("session_").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

            session_blocks.push(crate::state::SessionBlock {
                start_timestamp,
                duration_secs: session_duration,
                timeline_offset_secs: current_timeline_offset,
                playlist_path: playlist_path.clone(),
            });
            current_timeline_offset += session_duration;
        }
    }

    if session_blocks.is_empty() { return None; }

    // 4. Generate master view.edl for the video player
    let master_edl_path = game_dir.join("view.edl");
    let mut edl_content = String::from("# mpv EDL v0\n");
    for block in &session_blocks {
        let session_dir = block.playlist_path.parent().unwrap();
        let dir_name = session_dir.file_name().unwrap().to_string_lossy();
        edl_content.push_str(&format!("{}/session.m3u8\n", dir_name));
    }
    let _ = std::fs::write(&master_edl_path, edl_content);

    // 5. Generate master view.ffconcat for seamless exports
    let ffconcat_path = game_dir.join("view.ffconcat");
    let mut ffconcat_content = String::from("ffconcat version 1.0\n");
    for block in &session_blocks {
        let session_dir = block.playlist_path.parent().unwrap();
        let dir_name = session_dir.file_name().unwrap().to_string_lossy();
        let mut segments: Vec<_> = std::fs::read_dir(session_dir).ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "m4s"))
            .collect();
        segments.sort_by_key(|e| e.file_name());

        let seg_count = segments.len();
        for (i, seg_entry) in segments.iter().enumerate() {
            let seg = seg_entry.path();
            let file_name = seg.file_name().unwrap().to_string_lossy();
            if file_name == "segment_00000.m4s" { continue; }

            let mut duration = 2.0;
            if i == seg_count - 1 {
                duration = get_exact_duration(&seg);
            }
            ffconcat_content.push_str(&format!("file '{}/{}'\nduration {:.3}\n", dir_name, file_name, duration));
        }
    }
    let _ = std::fs::write(&ffconcat_path, ffconcat_content);

    Some((master_edl_path, session_blocks))
}

pub fn find_steam_artwork(game_title: &str) -> Option<String> {
    let sanitized: String = game_title.chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    
    let slug = sanitized.replace(' ', "");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam_path: PathBuf = match hkcu.open_subkey("Software\\Valve\\Steam") {
        Ok(key) => {
            let s: String = key.get_value("SteamPath").unwrap_or_default();
            PathBuf::from(s.replace("/", "\\"))
        }
        Err(_) => return None,
    };

    let artwork_dir = steam_path.join("appcache").join("librarycache");
    if let Ok(entries) = std::fs::read_dir(&artwork_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(&slug) && name.ends_with("_header.jpg") {
                return Some(entry.path().to_string_lossy().to_string());
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
                        let mut segments = Vec::new();

                        if let Ok(entries) = std::fs::read_dir(&root_dir) {
                            for entry in entries.filter_map(|e| e.ok()) {
                                if entry.path().is_dir() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if name == "Clips" || name == "Cache" || name.starts_with(".") { continue; }

                                    if let Ok(seg_entries) = std::fs::read_dir(entry.path()) {
                                        for sess in seg_entries.filter_map(|e| e.ok()) {
                                            if sess.path().is_dir() {
                                                if let Ok(segs) = std::fs::read_dir(sess.path()) {
                                                    for s in segs.filter_map(|e| e.ok()) {
                                                        if s.path().extension().map_or(false, |ext| ext == "m4s") {
                                                            if let Ok(meta) = s.metadata() {
                                                                total_size += meta.len();
                                                                segments.push((s.path(), meta.modified().unwrap()));
                                                            }
                                                        }
                                                    }
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
