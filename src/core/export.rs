//! Clip export backend: builds and runs the ffmpeg command that trims the
//! marked span out of the recorded segments, mixes the enabled audio tracks,
//! and writes the output clip plus a thumbnail.
//!
//! This is the logic half of the export feature; the desktop modal and the
//! overlay export in `ui/export.rs` gather state, then hand a frozen
//! [`ExportParams`] to [`run_export`]. Progress is published to
//! `AppState::export.progress`; the result flows back as a return value.

use crate::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;

/// Frozen inputs for a single export run. The UI computes all of these (clip
/// span, output path, frozen record-time stream mapping, encode options) before
/// the user can change anything, so the ffmpeg invocation is fully determined.
pub struct ExportParams {
    pub ffmpeg_path: PathBuf,
    /// ffmpeg concat-demuxer list spanning the marked segments. Removed when the
    /// export finishes.
    pub concat_path: PathBuf,
    /// Trim points, in seconds, into the concatenated timeline.
    pub in_offset: f64,
    pub out_offset: f64,
    pub output_path: PathBuf,
    pub reencode: bool,
    pub encoder: String,
    pub bitrate: i32,
    pub preset: String,
    pub container: String,
    /// Audio tracks for this source, with the user's per-track include toggles.
    pub audio_tracks: Vec<crate::config::AudioRouting>,
    /// Physical audio-stream index in the recorded file for each entry in
    /// `audio_tracks`, or `None` if that track had no stream at record time.
    pub track_stream_idx: Vec<Option<usize>>,
}

/// Run the export synchronously (intended to be called from a background task).
///
/// Tries CUDA hardware decoding first, falls back to software decode, then
/// extracts a mid-clip thumbnail. Returns ffmpeg's `Output` (so the caller can
/// inspect `status`/`stderr`) alongside the output path. Progress is written to
/// `app_state.export.progress` as the clip encodes.
pub fn run_export(
    params: ExportParams,
    app_state: Arc<AppState>,
) -> (std::io::Result<std::process::Output>, PathBuf) {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let ExportParams {
        ffmpeg_path,
        concat_path,
        in_offset,
        out_offset,
        output_path,
        reencode: export_reencode,
        encoder,
        bitrate,
        preset,
        container,
        audio_tracks,
        track_stream_idx,
    } = params;

    let total_dur_secs = (out_offset - in_offset).max(0.001);

    // Physical stream indices for the tracks the user kept enabled, using
    // each track's frozen record-time mapping (None = no stream existed).
    let enabled_streams: Vec<usize> = audio_tracks
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            if t.enabled {
                track_stream_idx.get(i).copied().flatten()
            } else {
                None
            }
        })
        .collect();

    let build_cmd = |hwaccel: bool| {
        let mut cmd = Command::new(ffmpeg_path.clone());
        cmd.creation_flags(0x08000000);
        cmd.arg("-y");
        // Emit machine-readable progress on stdout; ffmpeg's normal logs
        // stay on stderr, so the two never collide.
        cmd.arg("-progress").arg("pipe:1").arg("-nostats");
        if hwaccel {
            cmd.arg("-hwaccel").arg("cuda")
               .arg("-hwaccel_output_format").arg("cuda");
        }
        cmd.arg("-f").arg("concat")
           .arg("-safe").arg("0")
           .arg("-i").arg(concat_path.clone())
           .arg("-ss").arg(format!("{:.3}", in_offset))
           .arg("-to").arg(format!("{:.3}", out_offset))
           .arg("-map").arg("0:v:0");

        // Combine the enabled audio tracks into a single output track:
        // one stream maps straight through; several are mixed with `amix`
        // (normalize=0 preserves each source's level instead of dividing
        // the volume by the input count).
        match enabled_streams.as_slice() {
            [] => {}
            [phys] => {
                // Trailing `?` makes the map optional: if this track's
                // stream isn't actually present in the spanned segments
                // (e.g. an older clip recorded before the track existed),
                // ffmpeg drops it instead of aborting the whole export.
                cmd.arg("-map").arg(format!("0:a:{}?", phys));
            }
            streams => {
                let mut filter = String::new();
                for &phys in streams {
                    filter.push_str(&format!("[0:a:{}]", phys));
                }
                filter.push_str(&format!(
                    "amix=inputs={}:normalize=0:duration=longest[aout]",
                    streams.len()
                ));
                cmd.arg("-filter_complex").arg(filter).arg("-map").arg("[aout]");
            }
        }

        if export_reencode {
            cmd.arg("-c:v")
                .arg(&encoder)
                .arg("-preset")
                .arg(&preset)
                .arg("-b:v")
                .arg(format!("{}k", bitrate));
        } else {
            cmd.arg("-c:v").arg("copy");
        }

        cmd.arg("-c:a").arg("aac")
            .arg("-b:a").arg("320k")
            .arg("-ar").arg("48000");
        if container == "mp4" || container == "mov" {
            cmd.arg("-movflags").arg("+faststart");
        }
        cmd.arg(&output_path);
        cmd
    };

    // Spawn ffmpeg, stream `-progress` from stdout to drive the real
    // progress bar, and collect stderr for error reporting. Mirrors
    // `Command::output()` semantics but with live progress.
    let run = |mut cmd: Command| -> std::io::Result<std::process::Output> {
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut child = cmd.spawn()?;

        let reader = child.stdout.take().map(|stdout| {
            let progress_state = app_state.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    // ffmpeg reports position as out_time_us (newer) or
                    // out_time_ms (older builds emit microseconds here too).
                    let micros = line.strip_prefix("out_time_us=")
                        .or_else(|| line.strip_prefix("out_time_ms="))
                        .and_then(|v| v.trim().parse::<i64>().ok());
                    if let Some(us) = micros {
                        let secs = us as f64 / 1_000_000.0;
                        let p = (secs / total_dur_secs).clamp(0.0, 0.99) as f32;
                        *progress_state.export.progress.lock() = p;
                    }
                }
            })
        });

        let output = child.wait_with_output()?;
        if let Some(t) = reader { let _ = t.join(); }
        Ok(output)
    };

    // Try with CUDA hardware decoding first, fall back to software
    let cmd = build_cmd(true);
    log::info!("[Export] Running FFmpeg (hwaccel cuda): {:?}", cmd);
    let clip_output = match run(cmd) {
        Ok(out) if out.status.success() => Ok(out),
        _ => {
            log::warn!("[Export] CUDA decode failed, retrying with software decoder");
            *app_state.export.progress.lock() = 0.0;
            let cmd = build_cmd(false);
            log::info!("[Export] Running FFmpeg (software): {:?}", cmd);
            run(cmd)
        }
    };

    // Extract a thumbnail from the middle of the clip
    let thumb_time = (out_offset - in_offset) / 2.0;
    let mut thumb_path = output_path.clone();
    thumb_path.set_extension("jpg");

    if clip_output.as_ref().map_or(false, |o| o.status.success()) {
        let mut thumb_cmd = Command::new(&ffmpeg_path);
        thumb_cmd.creation_flags(0x08000000);
        thumb_cmd.arg("-y")
                 .arg("-ss").arg(format!("{:.3}", thumb_time))
                 .arg("-i").arg(&output_path)
                 .arg("-vframes").arg("1")
                 .arg("-q:v").arg("2")
                 .arg(&thumb_path);

        log::info!("[Export] Generating thumbnail: {:?}", thumb_cmd);
        if let Ok(out) = thumb_cmd.output() {
            if !out.status.success() {
                log::warn!("[Export] Thumbnail generation failed: {}", String::from_utf8_lossy(&out.stderr));
            }
        }
    }

    let _ = std::fs::remove_file(&concat_path);

    (clip_output, output_path)
}
