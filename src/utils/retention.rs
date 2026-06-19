//! Rolling-buffer enforcement: per-game duration retention + a global GB cap,
//! driven by a filesystem watcher (with a polling fallback).

use std::path::PathBuf;

use super::paths::clean_title;
use super::segments::{get_segment_duration, DEFAULT_SEGMENT_DURATION_SECS};

/// One full retention + global-size-cap sweep over the recordings root.
/// Shared by the file-watcher path and the polling fallback so both enforce
/// the same limits.
fn run_buffer_cleanup_scan(root_dir: &std::path::Path) {
    let config = crate::config::AppConfig::load();
    let max_bytes = config.max_buffer_size_gb as u64 * 1024 * 1024 * 1024;
    let mut total_size = 0;
    let mut global_segments = Vec::new();

    if let Ok(entries) = std::fs::read_dir(root_dir) {
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
                    // No watcher: degrade to a polling loop that still enforces
                    // retention and the global GB cap, rather than going dark and
                    // letting the buffer grow without bound.
                    log::error!("[Cleanup] Failed to create file watcher: {}. Falling back to {}s polling.", e, CLEANUP_POLL_INTERVAL_SECS);
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(CLEANUP_POLL_INTERVAL_SECS));
                        run_buffer_cleanup_scan(&root_dir);
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

                        run_buffer_cleanup_scan(&root_dir);
                    }
                    Err(e) => {
                        log::warn!("[Cleanup] Watcher error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    }
                }
            }
        }).ok();
}
