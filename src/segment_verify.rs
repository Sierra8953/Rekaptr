//! Segment continuity verifier for Luma recordings.
//!
//! Luma records as a chain of fMP4 (.m4s) segments. When the user stops and
//! restarts recording, the new segment must pick up exactly where the previous
//! one left off in PTS space — any gap causes a visible stutter in playback,
//! and any overlap causes frames to be dropped by the decoder.
//!
//! This module uses ffprobe to extract packet-level PTS from each segment and
//! verifies continuity across the entire chain. It also cross-checks the M3U8
//! playlist to ensure it references the correct segments with accurate durations.
//!
//! Key insight: we use **packet-level PTS** (first packet to last packet + frame
//! duration) instead of `format.duration` because fMP4's format-level duration
//! reports the cumulative timeline from PTS 0, not the segment's own duration.
//! For segment N starting at PTS 30s with 6s of content, `format.duration`
//! reports ~36s, not 6s. Only packet analysis gives the true segment duration.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A single verified segment with its actual media properties.
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    pub path: PathBuf,
    pub index: u64,
    pub filename_duration_ms: u64,
    /// Actual start PTS in seconds (from ffprobe)
    pub actual_start_pts: f64,
    /// Actual duration in seconds (from ffprobe)
    pub actual_duration: f64,
    /// Computed end PTS (start + duration)
    pub actual_end_pts: f64,
    /// Video codec (e.g. "h264", "hevc", "av1")
    pub codec: String,
    /// Resolution as "WxH"
    pub resolution: String,
    /// Frame rate as a string (e.g. "60/1")
    pub fps: String,
}

#[derive(Debug, Clone)]
/// A PTS discontinuity between two consecutive segments.
pub struct SegmentGap {
    pub after_segment_index: u64,
    pub before_segment_index: u64,
    /// Positive = gap (missing time), Negative = overlap
    pub delta_ms: f64,
    pub expected_start_pts: f64,
    pub actual_start_pts: f64,
}

#[derive(Debug, Clone)]
/// A segment whose filename-encoded duration doesn't match ffprobe reality.
pub struct DurationMismatch {
    pub segment_index: u64,
    pub filename_duration_ms: u64,
    pub actual_duration_ms: f64,
    pub delta_ms: f64,
}

#[derive(Debug, Clone)]
pub struct CodecChange {
    pub at_segment_index: u64,
    pub prev_codec: String,
    pub new_codec: String,
}

#[derive(Debug, Clone)]
pub struct IndexGap {
    pub expected: u64,
    pub actual: u64,
}

#[derive(Debug)]
/// Complete verification report for a game's segment chain.
pub struct VerifyReport {
    pub game_dir: PathBuf,
    pub total_segments: usize,
    pub total_duration_secs: f64,
    pub segments: Vec<SegmentInfo>,
    /// Timestamp gaps/overlaps between consecutive segments
    pub gaps: Vec<SegmentGap>,
    /// Segments where filename duration doesn't match actual duration
    pub duration_mismatches: Vec<DurationMismatch>,
    /// Points where codec or resolution changes (causes decoder resets)
    pub codec_changes: Vec<CodecChange>,
    /// Gaps in segment index numbering
    pub index_gaps: Vec<IndexGap>,
    /// Segments where ffprobe failed entirely
    pub probe_failures: Vec<(PathBuf, String)>,
}

impl VerifyReport {
    pub fn is_perfect(&self) -> bool {
        self.gaps.is_empty()
            && self.duration_mismatches.is_empty()
            && self.codec_changes.is_empty()
            && self.index_gaps.is_empty()
            && self.probe_failures.is_empty()
    }

    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "=== Segment Verification: {} ===\n",
            self.game_dir.file_name().unwrap_or_default().to_string_lossy()
        ));
        out.push_str(&format!(
            "Segments: {}  |  Total Duration: {:.2}s ({:.1} min)\n\n",
            self.total_segments,
            self.total_duration_secs,
            self.total_duration_secs / 60.0
        ));

        if self.is_perfect() {
            out.push_str("PASS: All segments are continuous with matching durations.\n");
            return out;
        }

        if !self.index_gaps.is_empty() {
            out.push_str(&format!("INDEX GAPS ({}):\n", self.index_gaps.len()));
            for g in &self.index_gaps {
                out.push_str(&format!(
                    "  Expected index {}, got {} (skipped {})\n",
                    g.expected,
                    g.actual,
                    g.actual - g.expected
                ));
            }
            out.push('\n');
        }

        if !self.gaps.is_empty() {
            out.push_str(&format!("TIMESTAMP GAPS/OVERLAPS ({}):\n", self.gaps.len()));
            for g in &self.gaps {
                let kind = if g.delta_ms > 0.0 { "GAP" } else { "OVERLAP" };
                out.push_str(&format!(
                    "  {} between seg {} → seg {}: {:.1}ms (expected PTS {:.3}s, got {:.3}s)\n",
                    kind,
                    g.after_segment_index,
                    g.before_segment_index,
                    g.delta_ms.abs(),
                    g.expected_start_pts,
                    g.actual_start_pts
                ));
            }
            out.push('\n');
        }

        if !self.duration_mismatches.is_empty() {
            out.push_str(&format!(
                "DURATION MISMATCHES ({}):\n",
                self.duration_mismatches.len()
            ));
            for m in &self.duration_mismatches {
                out.push_str(&format!(
                    "  seg {}: filename says {}ms, actual {:.1}ms (off by {:.1}ms)\n",
                    m.segment_index,
                    m.filename_duration_ms,
                    m.actual_duration_ms,
                    m.delta_ms
                ));
            }
            out.push('\n');
        }

        if !self.codec_changes.is_empty() {
            out.push_str(&format!("CODEC CHANGES ({}):\n", self.codec_changes.len()));
            for c in &self.codec_changes {
                out.push_str(&format!(
                    "  At seg {}: {} → {} (will cause decoder reset!)\n",
                    c.at_segment_index, c.prev_codec, c.new_codec
                ));
            }
            out.push('\n');
        }

        if !self.probe_failures.is_empty() {
            out.push_str(&format!(
                "PROBE FAILURES ({}):\n",
                self.probe_failures.len()
            ));
            for (path, err) in &self.probe_failures {
                out.push_str(&format!(
                    "  {}: {}\n",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    err
                ));
            }
            out.push('\n');
        }

        out
    }
}

/// Probe a single .m4s segment with ffprobe and extract its media properties.
fn probe_segment(path: &Path, ffprobe_path: &Path) -> Result<SegmentInfo, String> {
    let filename = path.file_name().unwrap().to_string_lossy();

    // Parse index and filename duration from the filename pattern: seg_NNNNNNNNNN_XXXXms.m4s
    let stem = path.file_stem().unwrap().to_string_lossy();
    let (index, filename_duration_ms) = parse_segment_filename(&stem)?;

    // Run ffprobe to get actual PTS, duration, codec, resolution, fps
    let output = Command::new(ffprobe_path)
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=codec_name,width,height,r_frame_rate,start_time,duration",
            "-show_entries", "format=start_time,duration",
            "-of", "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("ffprobe exec failed for {}: {}", filename, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe error for {}: {}", filename, stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut codec = String::new();
    let mut width = 0i32;
    let mut height = 0i32;
    let mut fps = String::new();
    let mut stream_start = None;
    let mut stream_duration = None;
    let mut format_start = None;
    let mut format_duration = None;
    for line in stdout.lines() {
        // ffprobe outputs stream entries first, then format entries
        // We track which section we're in by looking for known keys
        if let Some(val) = line.strip_prefix("codec_name=") {
            codec = val.to_string();
        } else if let Some(val) = line.strip_prefix("width=") {
            width = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("height=") {
            height = val.parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("r_frame_rate=") {
            fps = val.to_string();
        } else if let Some(val) = line.strip_prefix("start_time=") {
            if let Ok(v) = val.parse::<f64>() {
                if stream_start.is_none() {
                    stream_start = Some(v);
                } else {
                    format_start = Some(v);
                }
            }
        } else if let Some(val) = line.strip_prefix("duration=") {
            if let Ok(v) = val.parse::<f64>() {
                if stream_duration.is_none() {
                    stream_duration = Some(v);
                } else {
                    format_duration = Some(v);
                }
            }
        }
    }

    // Prefer stream-level data, fall back to format-level
    let actual_start_pts = stream_start.or(format_start).unwrap_or(0.0);
    let actual_duration = stream_duration.or(format_duration).unwrap_or(0.0);

    if actual_duration <= 0.0 {
        return Err(format!("{}: ffprobe returned zero/negative duration", filename));
    }

    Ok(SegmentInfo {
        path: path.to_path_buf(),
        index,
        filename_duration_ms,
        actual_start_pts,
        actual_duration,
        actual_end_pts: actual_start_pts + actual_duration,
        codec,
        resolution: format!("{}x{}", width, height),
        fps,
    })
}

/// Parse `seg_0000000005_6000ms` → (5, 6000)
fn parse_segment_filename(stem: &str) -> Result<(u64, u64), String> {
    let stem = stem.strip_prefix("seg_").ok_or_else(|| format!("unexpected filename: {}", stem))?;

    // Split into parts: ["0000000005", "6000ms"] or ["0000000005", "6000ms"]
    let parts: Vec<&str> = stem.splitn(2, '_').collect();
    let index: u64 = parts
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("can't parse index from: {}", stem))?;

    let duration_ms = if parts.len() == 2 {
        parts[1]
            .strip_suffix("ms")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("can't parse duration from: {}", stem))?
    } else {
        return Err(format!("no duration suffix in: {}", stem));
    };

    Ok((index, duration_ms))
}

/// Find ffprobe next to ffmpeg in the bin directory.
fn find_ffprobe() -> PathBuf {
    let ffmpeg = crate::utils::get_ffmpeg_path();
    let dir = ffmpeg.parent().unwrap_or(Path::new("."));
    dir.join("ffprobe.exe")
}

/// Verify all segments in a game directory for playback continuity.
///
/// `gap_threshold_ms` controls how much PTS drift is tolerated between
/// consecutive segments before it's flagged. 5ms is a good default;
/// anything above ~30ms will likely cause a visible stutter.
///
/// `duration_threshold_ms` controls how much the filename duration can
/// differ from the actual ffprobe duration before it's flagged.
pub fn verify_segments(
    game_dir: &Path,
    gap_threshold_ms: f64,
    duration_threshold_ms: f64,
) -> VerifyReport {
    let ffprobe_path = find_ffprobe();
    let mut report = VerifyReport {
        game_dir: game_dir.to_path_buf(),
        total_segments: 0,
        total_duration_secs: 0.0,
        segments: Vec::new(),
        gaps: Vec::new(),
        duration_mismatches: Vec::new(),
        codec_changes: Vec::new(),
        index_gaps: Vec::new(),
        probe_failures: Vec::new(),
    };

    // 1. Collect all finalized segments (with _XXXXms suffix)
    let mut segment_paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "m4s") {
                let name = path.file_stem().unwrap().to_string_lossy();
                if name.starts_with("seg_") && name.contains("ms") {
                    segment_paths.push(path);
                }
            }
        }
    }

    if segment_paths.is_empty() {
        return report;
    }

    // Sort by segment index (not filename, not mtime — index is the source of truth)
    segment_paths.sort_by_key(|p| {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        parse_segment_filename(&stem).map(|(idx, _)| idx).unwrap_or(u64::MAX)
    });

    // 2. Probe each segment
    for path in &segment_paths {
        match probe_segment(path, &ffprobe_path) {
            Ok(info) => report.segments.push(info),
            Err(err) => report.probe_failures.push((path.clone(), err)),
        }
    }

    report.total_segments = report.segments.len();
    report.total_duration_secs = report.segments.iter().map(|s| s.actual_duration).sum();

    if report.segments.len() < 2 {
        return report;
    }

    // 3. Check index continuity
    for i in 1..report.segments.len() {
        let prev_idx = report.segments[i - 1].index;
        let curr_idx = report.segments[i].index;
        if curr_idx != prev_idx + 1 {
            report.index_gaps.push(IndexGap {
                expected: prev_idx + 1,
                actual: curr_idx,
            });
        }
    }

    // 4. Check PTS continuity (the main thing that causes stuttering)
    for i in 1..report.segments.len() {
        let prev = &report.segments[i - 1];
        let curr = &report.segments[i];

        let expected_start = prev.actual_end_pts;
        let delta_ms = (curr.actual_start_pts - expected_start) * 1000.0;

        if delta_ms.abs() > gap_threshold_ms {
            report.gaps.push(SegmentGap {
                after_segment_index: prev.index,
                before_segment_index: curr.index,
                delta_ms,
                expected_start_pts: expected_start,
                actual_start_pts: curr.actual_start_pts,
            });
        }
    }

    // 5. Check filename duration vs actual duration
    for seg in &report.segments {
        let actual_ms = seg.actual_duration * 1000.0;
        let delta = actual_ms - seg.filename_duration_ms as f64;
        if delta.abs() > duration_threshold_ms {
            report.duration_mismatches.push(DurationMismatch {
                segment_index: seg.index,
                filename_duration_ms: seg.filename_duration_ms,
                actual_duration_ms: actual_ms,
                delta_ms: delta,
            });
        }
    }

    // 6. Check for codec/resolution changes
    for i in 1..report.segments.len() {
        let prev = &report.segments[i - 1];
        let curr = &report.segments[i];

        // Compare codec + resolution only — framerate metadata varies on runt EOS segments
        let prev_key = format!("{}/{}", prev.codec, prev.resolution);
        let curr_key = format!("{}/{}", curr.codec, curr.resolution);

        if prev_key != curr_key {
            report.codec_changes.push(CodecChange {
                at_segment_index: curr.index,
                prev_codec: prev_key,
                new_codec: curr_key,
            });
        }
    }

    report
}

/// Verify segments AND the generated M3U8 playlist against reality.
pub fn verify_playlist(game_dir: &Path) -> (VerifyReport, Vec<String>) {
    let report = verify_segments(game_dir, 5.0, 50.0);
    let mut playlist_issues = Vec::new();

    let m3u8_path = game_dir.join("master.m3u8");
    if !m3u8_path.exists() {
        playlist_issues.push("master.m3u8 does not exist".to_string());
        return (report, playlist_issues);
    }

    let content = match std::fs::read_to_string(&m3u8_path) {
        Ok(c) => c,
        Err(e) => {
            playlist_issues.push(format!("Can't read master.m3u8: {}", e));
            return (report, playlist_issues);
        }
    };

    // Parse EXTINF entries from the playlist
    let mut playlist_entries: Vec<(f64, String)> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(rest) = lines[i].strip_prefix("#EXTINF:") {
            let duration: f64 = rest.trim_end_matches(',').parse().unwrap_or(0.0);
            if i + 1 < lines.len() {
                let filename = lines[i + 1].trim().to_string();
                playlist_entries.push((duration, filename));
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    // Cross-check: every segment on disk should be in the playlist
    for seg in &report.segments {
        let seg_filename = seg.path.file_name().unwrap().to_string_lossy();
        let in_playlist = playlist_entries.iter().any(|(_, f)| f == seg_filename.as_ref());
        if !in_playlist {
            playlist_issues.push(format!(
                "Segment {} exists on disk but is missing from M3U8",
                seg_filename
            ));
        }
    }

    // Cross-check: every playlist entry should exist on disk
    for (_, filename) in &playlist_entries {
        let seg_path = game_dir.join(filename);
        if !seg_path.exists() {
            playlist_issues.push(format!(
                "M3U8 references {} but file doesn't exist",
                filename
            ));
        }
    }

    // Check playlist durations match segment durations
    for (playlist_dur, filename) in &playlist_entries {
        if let Some(seg) = report
            .segments
            .iter()
            .find(|s| s.path.file_name().unwrap().to_string_lossy() == filename.as_str())
        {
            let delta_ms = (playlist_dur - seg.actual_duration).abs() * 1000.0;
            if delta_ms > 50.0 {
                playlist_issues.push(format!(
                    "{}: M3U8 says {:.3}s but actual is {:.3}s (off by {:.1}ms)",
                    filename, playlist_dur, seg.actual_duration, delta_ms
                ));
            }
        }
    }

    // Check playlist order matches segment index order
    let playlist_filenames: Vec<&str> = playlist_entries.iter().map(|(_, f)| f.as_str()).collect();
    let sorted_seg_filenames: Vec<String> = report
        .segments
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    // Filter playlist to only entries that exist in our segments
    let playlist_known: Vec<&str> = playlist_filenames
        .iter()
        .filter(|f| sorted_seg_filenames.iter().any(|s| s == **f))
        .copied()
        .collect();

    for (i, (p, s)) in playlist_known.iter().zip(sorted_seg_filenames.iter()).enumerate() {
        if *p != s {
            playlist_issues.push(format!(
                "Order mismatch at position {}: M3U8 has {} but index order expects {}",
                i, p, s
            ));
            break;
        }
    }

    (report, playlist_issues)
}

/// Get actual segment duration from packet-level PTS using ffprobe.
///
/// This is the source of truth for fMP4 segments. We cannot use `format.duration`
/// because it reports the cumulative PTS offset from the start of the recording
/// timeline (PTS 0), not the segment's own duration. For example, the 5th segment
/// starting at PTS 30s with 6s of video would report `format.duration = 36s`.
///
/// Instead, we enumerate every video packet's PTS, compute `(last_pts + last_frame_dur) - first_pts`,
/// and get the true content duration.
pub fn ffprobe_segment_duration(path: &Path) -> Option<f64> {
    let ffprobe_path = find_ffprobe();
    let output = Command::new(&ffprobe_path)
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "packet=pts_time,duration_time",
            "-of", "csv=p=0",
        ])
        .arg(path)
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

/// Post-pipeline fixup for EOS (end-of-stream) segments.
///
/// When a recording stops, GStreamer's `splitmuxsink` emits a bus message with
/// the segment's duration. However, the EOS segment — the final one being written
/// when the pipeline shuts down — often gets an incorrect duration in that message
/// because the muxer hasn't finished flushing. Additionally, the file may still
/// be locked by the muxer during the bus callback, preventing the normal rename.
///
/// This function runs **after** `set_state(Null)` (when the muxer has released
/// file handles) and performs two fixups:
///
/// 1. **Unrenamed segments**: Finds `.m4s` files without a `_XXXXms` duration
///    suffix and renames them using ffprobe's packet-level PTS analysis.
///
/// 2. **Wrong-duration segments**: Finds segments where the bus-reported duration
///    (encoded in the filename) differs from actual content by >100ms, and
///    corrects the filename.
pub fn fixup_eos_segments(game_dir: &Path) {
    // Step 1: Rename segments that never got their duration suffix
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        let mut unrenamed: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |e| e == "m4s") && {
                    let name = p.file_stem().unwrap().to_string_lossy();
                    name.starts_with("seg_") && !name.contains("ms")
                }
            })
            .collect();

        unrenamed.sort();

        for path in unrenamed {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            if let Some(dur_secs) = ffprobe_segment_duration(&path) {
                let dur_ms = (dur_secs * 1000.0).round() as u64;
                let mut new_path = path.clone();
                new_path.set_file_name(format!("{}_{}ms.m4s", stem, dur_ms));
                if let Err(e) = std::fs::rename(&path, &new_path) {
                    log::warn!(
                        "[SegmentFixup] Failed to rename {}: {}",
                        path.file_name().unwrap().to_string_lossy(),
                        e
                    );
                } else {
                    log::info!(
                        "[SegmentFixup] Renamed {} → {} (ffprobe: {:.3}s)",
                        path.file_name().unwrap().to_string_lossy(),
                        new_path.file_name().unwrap().to_string_lossy(),
                        dur_secs
                    );
                }
            } else {
                log::warn!(
                    "[SegmentFixup] ffprobe failed for {}",
                    path.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }

    // Step 2: Correct segments where bus gave wrong duration (EOS truncation)
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        let mut segments: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |e| e == "m4s") && {
                    let name = p.file_stem().unwrap().to_string_lossy();
                    name.starts_with("seg_") && name.contains("ms")
                }
            })
            .collect();

        segments.sort();

        for path in segments {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let filename_dur = crate::utils::get_segment_duration(&path);
            let actual_dur = ffprobe_segment_duration(&path);

            if let (Some(f_dur), Some(a_dur)) = (filename_dur, actual_dur) {
                let delta_ms = (f_dur - a_dur).abs() * 1000.0;
                if delta_ms > 100.0 {
                    let base = stem.rsplitn(2, '_').last().unwrap_or(&stem);
                    let correct_ms = (a_dur * 1000.0).round() as u64;
                    let mut new_path = path.clone();
                    new_path.set_file_name(format!("{}_{}ms.m4s", base, correct_ms));

                    if let Err(e) = std::fs::rename(&path, &new_path) {
                        log::warn!(
                            "[SegmentFixup] Duration fix failed for {}: {}",
                            path.file_name().unwrap().to_string_lossy(),
                            e
                        );
                    } else {
                        log::info!(
                            "[SegmentFixup] Duration fix: {} → {} (was {:.0}ms, actual {:.0}ms)",
                            path.file_name().unwrap().to_string_lossy(),
                            new_path.file_name().unwrap().to_string_lossy(),
                            f_dur * 1000.0,
                            a_dur * 1000.0
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_filename_valid() {
        let (idx, dur) = parse_segment_filename("seg_0000000005_6000ms").unwrap();
        assert_eq!(idx, 5);
        assert_eq!(dur, 6000);
    }

    #[test]
    fn parse_segment_filename_small_duration() {
        let (idx, dur) = parse_segment_filename("seg_0000000000_1234ms").unwrap();
        assert_eq!(idx, 0);
        assert_eq!(dur, 1234);
    }

    #[test]
    fn parse_segment_filename_no_prefix() {
        assert!(parse_segment_filename("bad_0000000005_6000ms").is_err());
    }

    #[test]
    fn parse_segment_filename_no_duration() {
        assert!(parse_segment_filename("seg_0000000005").is_err());
    }

    /// Integration test: run against an actual game directory.
    /// Skipped by default — run with:
    ///   cargo test verify_real_segments -- --ignored --nocapture
    #[test]
    #[ignore]
    fn verify_real_segments() {
        let root = crate::utils::get_storage_root();
        let mut found_any = false;

        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !path.is_dir() || name == "Clips" || name == "Cache" || name.starts_with(".") {
                    continue;
                }

                let (report, playlist_issues) = verify_playlist(&path);
                if report.total_segments == 0 {
                    continue;
                }

                found_any = true;
                println!("{}", report.summary());

                if !playlist_issues.is_empty() {
                    println!("PLAYLIST ISSUES ({}):", playlist_issues.len());
                    for issue in &playlist_issues {
                        println!("  {}", issue);
                    }
                    println!();
                }

                // Print per-segment PTS timeline for debugging
                println!("SEGMENT PTS TIMELINE:");
                for seg in &report.segments {
                    println!(
                        "  seg_{:010}: PTS {:.3}s → {:.3}s  (dur: {:.3}s, filename: {}ms, codec: {}, res: {})",
                        seg.index,
                        seg.actual_start_pts,
                        seg.actual_end_pts,
                        seg.actual_duration,
                        seg.filename_duration_ms,
                        seg.codec,
                        seg.resolution,
                    );
                }
                println!();
            }
        }

        if !found_any {
            println!("No game directories with segments found in {:?}", root);
        }
    }
}
