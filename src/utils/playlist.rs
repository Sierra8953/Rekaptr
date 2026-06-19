//! Cross-session timeline math: HLS master/session playlist generation, the
//! decode-time-offset probe (`compute_total_duration`), EOS segment fixup, and
//! turning mpv playback positions into `ClipMark`s / ffmpeg concat lists.

use std::path::{Path, PathBuf};

use super::paths::game_dir_for;
use super::segments::{
    ffprobe_segment_duration, ffprobe_segment_end_time, get_segment_duration, parse_segment_index,
    parse_segment_session_id, scan_segments, SegmentEntry,
};

/// Returns the end of the on-disk timeline in seconds — the value the next
/// session's splitmuxsink should use as decode-time-offset.
///
/// We probe tfdt directly rather than summing filename durations: storage-cap
/// rotation prunes older sessions, but the segments still on disk carry tfdt
/// values that include the pruned offset, so summing what remains under-counts
/// and the next session's tfdt collides with the retained range — mpv's
/// HLS+fMP4 demuxer then refuses to play across the discontinuity.
///
/// We must also take the *max* tfdt across all sessions, not the latest
/// session's tfdt: a previous bug let some sessions be written with offsets
/// lower than earlier sessions still on disk. The next session's offset has
/// to clear every existing tfdt to keep the timeline monotonic.
///
/// Robustness: ffprobe can be unavailable (e.g. ffprobe.exe missing from the
/// install's `bin/`), in which case every probe returns `None` and the tfdt
/// scan yields 0. Returning 0 here is catastrophic — the next session records
/// with `decode-time-offset=0`, its tfdt resets to ~0, overlaps the existing
/// range, and playback snaps the playhead back to the start at the seam. So we
/// also sum the filename `_XXXXms` durations (no external process, can't fail)
/// and return the larger of the two: the probed max wins when ffprobe works
/// (correct after pruning), and the filename sum is the floor that keeps the
/// timeline moving forward when ffprobe is missing.
pub fn compute_total_duration(game_dir: &Path) -> f64 {
    let Ok(entries) = std::fs::read_dir(game_dir) else {
        return 0.0;
    };

    let mut by_session: std::collections::HashMap<u64, Vec<(u64, std::path::PathBuf)>> =
        std::collections::HashMap::new();
    let mut filename_duration_sum: f64 = 0.0;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.extension().map_or(false, |e| e == "m4s") {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if let Some(dur) = get_segment_duration(&path) {
            filename_duration_sum += dur;
        }
        let Some(idx) = parse_segment_index(&name) else { continue; };
        let Some(sid) = parse_segment_session_id(&name) else { continue; };
        by_session.entry(sid).or_default().push((idx, path));
    }

    // For each session, probe its highest-indexed segment for the end tfdt;
    // walk back if it's a truncated orphan from a crashed pipeline. Take the
    // max across sessions so the next session clears every existing tfdt.
    let mut max_end: f64 = 0.0;
    for (_, mut segs) in by_session {
        segs.sort_by_key(|(idx, _)| *idx);
        for (_, path) in segs.iter().rev() {
            if let Some(end) = ffprobe_segment_end_time(path) {
                if end > max_end {
                    max_end = end;
                }
                break;
            }
        }
    }
    if max_end <= 0.0 && filename_duration_sum > 0.0 {
        log::warn!(
            "[Utils] tfdt probe found no end time (ffprobe unavailable?); \
             falling back to filename-duration sum {:.3}s for decode-time-offset",
            filename_duration_sum
        );
    }
    max_end.max(filename_duration_sum)
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

    // Deliberately NO #EXT-X-DISCONTINUITY between sessions. The cross-session
    // design keeps ONE continuous timeline: each new session's splitmuxsink is
    // started with decode-time-offset = the end of the on-disk timeline (see
    // compute_total_duration), so segment tfdts climb monotonically across the
    // session seam. A discontinuity tag makes mpv re-anchor its timeline at the
    // seam — which yanks the playhead back — so the only combination that plays
    // smoothly is "continuous timeline + flat playlist". Continuity is the
    // record-side job (the offset); the playlist must stay a plain segment list.
    for (_, (_modified, path, duration, _session_id)) in &segment_map {
        let file_name = match path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => continue,
        };
        m3u8.push_str(&format!("#EXTINF:{:.3},\n{}\n", duration, file_name));
    }
    m3u8.push_str("#EXT-X-ENDLIST\n");
    let _ = std::fs::write(&master_playlist_path, m3u8);

    Some(master_playlist_path)
}

/// Parse a master playlist into `(filename, duration_secs)` entries in order,
/// reading `#EXTINF:<dur>,` followed by the segment filename. Lines starting
/// with `#` other than EXTINF are skipped, which is fine for our generated
/// EVENT playlists.
fn parse_master_playlist_entries(path: &Path) -> Option<Vec<(String, f64)>> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut out = Vec::new();
    let mut pending_duration: Option<f64> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let dur_str = rest.split(',').next().unwrap_or("");
            pending_duration = dur_str.parse::<f64>().ok();
        } else if line.starts_with('#') {
            continue;
        } else if let Some(dur) = pending_duration.take() {
            out.push((line.to_string(), dur));
        }
    }
    Some(out)
}

/// Build a `ClipMark` from mpv's playback state. mpv reports `time-pos` in
/// the cumulative-EXTINF frame of whatever master playlist it loaded, so we
/// read the *same* playlist file, walk it summing each entry's `EXTINF`
/// duration, and find the entry whose [start, end) range contains
/// `mpv_time_pos`. The segment's `(session_id, segment_index)` parsed from
/// that entry's filename, plus `offset_in_segment = mpv_time_pos - entry_start`,
/// is the mark.
///
/// Anchoring against the exact playlist file mpv is reading guarantees the
/// mark identifies the same segment mpv was decoding, even if disk-scan
/// ordering diverges from playlist ordering.
pub fn mark_from_mpv_state(
    game_title: &str,
    _stream_filename: &str,
    mpv_time_pos: f64,
) -> Option<crate::state::ClipMark> {
    let game_dir = game_dir_for(game_title);
    let playlist_path = game_dir.join("master.m3u8");
    let entries = parse_master_playlist_entries(&playlist_path)?;
    if entries.is_empty() { return None; }

    let mut cumulative = 0.0_f64;
    for (filename, dur) in &entries {
        let next = cumulative + dur;
        if mpv_time_pos < next || (mpv_time_pos - next).abs() < 1e-6 {
            let idx = parse_segment_index(filename)?;
            let sid = parse_segment_session_id(filename);
            let offset = (mpv_time_pos - cumulative).clamp(0.0, *dur);
            return Some(crate::state::ClipMark {
                session_id: sid,
                segment_index: idx,
                offset_in_segment: offset,
            });
        }
        cumulative = next;
    }
    // Past the end: snap to the last entry's tail.
    let (last_name, last_dur) = entries.last()?;
    let idx = parse_segment_index(last_name)?;
    let sid = parse_segment_session_id(last_name);
    Some(crate::state::ClipMark {
        session_id: sid,
        segment_index: idx,
        offset_in_segment: *last_dur,
    })
}

/// Build an ffmpeg concat-demuxer list covering the range between two
/// `ClipMark`s. The disk scan is the source of truth; `master.m3u8` is
/// never read.
///
/// Returns `(concat_file_path, in_offset, out_offset)` where the offsets
/// are measured from the start of the first included segment in the
/// assembled stream — pass them as `-ss` / `-to` *after* `-i`.
pub fn build_clip_concat_list_from_marks(
    game_title: &str,
    in_mark: &crate::state::ClipMark,
    out_mark: &crate::state::ClipMark,
) -> Option<(PathBuf, f64, f64)> {
    let game_dir = game_dir_for(game_title);
    if !game_dir.exists() { return None; }
    let segments = scan_segments(&game_dir);
    if segments.is_empty() { return None; }

    let entries: Vec<(&u64, &SegmentEntry)> = segments.iter().collect();
    let in_i = entries.iter().position(|(idx, e)| {
        **idx == in_mark.segment_index && e.3 == in_mark.session_id
    })?;
    let out_i = entries.iter().position(|(idx, e)| {
        **idx == out_mark.segment_index && e.3 == out_mark.session_id
    })?;
    if out_i < in_i { return None; }

    let in_offset = in_mark.offset_in_segment.max(0.0);
    let mut prefix = 0.0_f64;
    for (_, e) in &entries[in_i..out_i] {
        prefix += e.2;
    }
    let out_offset = (prefix + out_mark.offset_in_segment).max(in_offset + 0.001);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let path = std::env::temp_dir().join(format!("rekaptr_clip_{}.txt", ts));

    let mut content = String::from("ffconcat version 1.0\n");
    for (_, e) in &entries[in_i..=out_i] {
        let abs = e.1.to_string_lossy().replace('\\', "/");
        let escaped = abs.replace('\'', "'\\''");
        content.push_str(&format!("file '{}'\n", escaped));
        content.push_str(&format!("duration {:.3}\n", e.2));
    }
    std::fs::write(&path, content).ok()?;
    Some((path, in_offset, out_offset))
}

/// Clip length in seconds between two marks, without writing a concat file.
/// Cheap enough to call when opening the export dialog.
pub fn clip_duration_from_marks(
    game_title: &str,
    in_mark: &crate::state::ClipMark,
    out_mark: &crate::state::ClipMark,
) -> Option<f64> {
    let game_dir = game_dir_for(game_title);
    if !game_dir.exists() { return None; }
    let segments = scan_segments(&game_dir);
    if segments.is_empty() { return None; }

    let entries: Vec<(&u64, &SegmentEntry)> = segments.iter().collect();
    let in_i = entries.iter().position(|(idx, e)| {
        **idx == in_mark.segment_index && e.3 == in_mark.session_id
    })?;
    let out_i = entries.iter().position(|(idx, e)| {
        **idx == out_mark.segment_index && e.3 == out_mark.session_id
    })?;
    if out_i < in_i { return None; }

    let in_offset = in_mark.offset_in_segment.max(0.0);
    let mut prefix = 0.0_f64;
    for (_, e) in &entries[in_i..out_i] {
        prefix += e.2;
    }
    let out_offset = (prefix + out_mark.offset_in_segment).max(in_offset + 0.001);
    Some(out_offset - in_offset)
}
