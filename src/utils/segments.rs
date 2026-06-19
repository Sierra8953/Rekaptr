//! Low-level fragmented-MP4 segment primitives shared by the playlist, clip,
//! retention and stats code: filename parsing (`seg_<index>_<session>_<dur>ms`),
//! ffprobe-based duration/end-time probing, and a deduplicating directory scan.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

use super::paths::find_bundled_binary;

/// Assumed segment length when ffprobe is unavailable or fails.
pub(crate) const DEFAULT_SEGMENT_DURATION_SECS: f64 = 6.0;

/// Segment entry after deduplication: (modified_time, path, duration_secs, session_id)
pub(crate) type SegmentEntry = (std::time::SystemTime, std::path::PathBuf, f64, Option<u64>);

fn get_ffprobe_path() -> PathBuf {
    find_bundled_binary("ffprobe.exe")
}

pub(crate) fn parse_segment_index(name: &str) -> Option<u64> {
    let stem = name.strip_prefix("seg_")?;
    let first_part = stem.split('_').next().unwrap_or(stem);
    first_part.parse::<u64>().ok()
}

pub(crate) fn parse_segment_session_id(name: &str) -> Option<u64> {
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

/// Read the absolute decode-time at which a segment ends (last packet PTS +
/// its duration). For fMP4 fragments this equals the global tfdt at the start
/// of the next fragment.
pub(crate) fn ffprobe_segment_end_time(path: &Path) -> Option<f64> {
    let ffprobe = get_ffprobe_path();
    let output = std::process::Command::new(&ffprobe)
        .args(["-v", "error", "-select_streams", "v:0",
               "-show_entries", "packet=pts_time,duration_time",
               "-of", "csv=p=0"])
        .arg(path)
        .creation_flags(0x08000000)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut last_pts: Option<f64> = None;
    let mut last_frame_dur: f64 = 0.0;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let Ok(pts) = parts[0].parse::<f64>() {
                last_pts = Some(pts);
                if let Ok(dur) = parts[1].parse::<f64>() {
                    last_frame_dur = dur;
                }
            }
        }
    }

    let end = last_pts? + last_frame_dur;
    if end > 0.0 { Some(end) } else { None }
}

/// Get actual segment duration from packet-level PTS using ffprobe.
/// format.duration lies for fMP4 segments — only packet analysis gives the true duration.
pub(crate) fn ffprobe_segment_duration(path: &Path) -> Option<f64> {
    let ffprobe = get_ffprobe_path();
    let output = std::process::Command::new(&ffprobe)
        .args(["-v", "error", "-select_streams", "v:0",
               "-show_entries", "packet=pts_time,duration_time",
               "-of", "csv=p=0"])
        .arg(path)
        .creation_flags(0x08000000)
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

pub(crate) fn get_segment_duration(path: &Path) -> Option<f64> {
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

/// Scan a game directory for .m4s segments, deduplicate by index (keep most recent).
/// Returns a BTreeMap sorted by segment index.
pub(crate) fn scan_segments(game_dir: &Path) -> std::collections::BTreeMap<u64, SegmentEntry> {
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
