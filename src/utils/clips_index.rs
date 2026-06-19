//! Discovery of on-disk content for the UI: list recorded source titles, build
//! the saved-clips library (with thumbnails), and roll up per-source stats.

use std::os::windows::process::CommandExt;
use std::path::Path;

use super::paths::{clean_title, game_dir_for, get_ffmpeg_path, get_storage_root};
use super::segments::scan_segments;

pub fn scan_session_titles() -> Vec<String> {
    let root = get_storage_root();
    let config = crate::config::AppConfig::load();
    let mut titles = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return titles,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        if folder_name == "Clips" || folder_name == "Cache" || folder_name.starts_with('.') {
            continue;
        }
        let has_segments = std::fs::read_dir(&path)
            .map(|rd| rd.filter_map(|e| e.ok()).any(|e| e.path().extension().map_or(false, |ext| ext == "m4s")))
            .unwrap_or(false);
        if !has_segments { continue; }
        let game_title = config.game_registry.iter()
            .find(|(t, _)| clean_title(t) == folder_name)
            .map(|(t, _)| t.clone())
            .unwrap_or(folder_name);
        titles.push(game_title);
    }
    titles
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
                                    path_str: clip_path.to_string_lossy().into_owned(),
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

/// Cheap-to-format rollup of a source's on-disk recording state, for the
/// sources-list columns.
#[derive(Clone, Copy, Default)]
pub struct SourceStats {
    pub disk_bytes: u64,
    pub last_modified: Option<std::time::SystemTime>,
    /// Total seconds currently on disk (the rolling buffer, not lifetime).
    pub total_secs: f64,
    /// Number of saved clips for this source (files under `Clips/<title>`).
    pub clip_count: usize,
}

/// Walk a source's recording directory once for size + most-recent mtime, and
/// sum its segment durations. Blocking — call off the UI thread.
pub fn source_stats(title: &str) -> SourceStats {
    let dir = game_dir_for(title);
    let mut disk_bytes = 0u64;
    let mut last_modified: Option<std::time::SystemTime> = None;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    disk_bytes += m.len();
                    // "Last recorded" must come from actual recording segments
                    // (.m4s) only. master.m3u8 is rewritten every time the source
                    // is reloaded, so including it would report the reload time
                    // rather than when the game was last recorded.
                    let is_segment = entry
                        .path()
                        .extension()
                        .map_or(false, |ext| ext == "m4s");
                    if is_segment {
                        if let Ok(mt) = m.modified() {
                            last_modified = Some(last_modified.map_or(mt, |l| l.max(mt)));
                        }
                    }
                }
            }
        }
    }
    let total_secs = if dir.exists() {
        scan_segments(&dir).values().map(|s| s.2).sum()
    } else {
        0.0
    };

    // Saved clips for this source live under `Clips/<safe_title>` (see
    // export.rs). Count the playable clip files there — this is independent of
    // the clips-library cache, which is cleared while off the Clips view.
    let safe_title = if title == "monitor" { "monitor".to_string() } else { clean_title(title) };
    let clips_dir = get_storage_root().join("Clips").join(&safe_title);
    let clip_count = std::fs::read_dir(&clips_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    // Match every container the export dialog offers (mp4/mov/mkv),
                    // case-insensitively, so the count matches what's on disk.
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map_or(false, |ext| {
                            matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mov" | "mkv")
                        })
                })
                .count()
        })
        .unwrap_or(0);

    SourceStats { disk_bytes, last_modified, total_secs, clip_count }
}
