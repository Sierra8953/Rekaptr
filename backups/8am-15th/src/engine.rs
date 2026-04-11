//! GStreamer pipeline construction and per-application audio capture.
//!
//! The recording pipeline captures the screen via D3D11 (DXGI/WGC), encodes on
//! the GPU, and writes fragmented MP4 segments to disk via `splitmuxsink`. Each
//! segment is independently decodable — a hard requirement for HLS playback and
//! the rolling buffer architecture.
//!
//! Audio is captured separately per source type:
//! - **System**: WASAPI loopback (captures desktop audio)
//! - **Mic**: fed from a shared `MicProvider` via `appsrc` (see `audio` module)
//! - **App**: per-process WASAPI loopback via `start_app_capture`
//!
//! The WASAPI-to-GStreamer bridge uses a lock-free ring buffer to decouple the
//! WASAPI capture thread's timing from GStreamer's clock. Without this, WASAPI
//! packet timing jitter would cause GStreamer queue underruns and audible glitches.

use crate::config::{AudioRouting, MicSettings, VideoSettings};
use sysinfo::System;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use anyhow::{Result, Context};
use wasapi::*;
use ringbuf::HeapRb;
use ringbuf::traits::{Split, Producer, Consumer, Observer};
use std::time::Duration;

pub fn parse_res(res: &str) -> (i32, i32) {
    let parts: Vec<&str> = res.split('x').collect();
    if parts.len() == 2 {
        if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
            return (w, h);
        }
    }
    (1920, 1080)
}

/// Resolves a process name to a PID, picking the instance with the highest memory
/// usage. This is a deliberate heuristic: games often spawn child processes (crash
/// reporters, launchers, anti-cheat) that share the same executable name. The main
/// game process almost always has the largest working set because it holds textures,
/// meshes, and audio buffers in memory.
pub fn resolve_pid(app_name: &str) -> u32 {
    if app_name.is_empty() || app_name == "Default" {
        return 0;
    }
    let mut sys = System::new_all();
    sys.refresh_processes();

    let target_name = app_name.to_lowercase();
    let target_name_no_ext = if target_name.ends_with(".exe") {
        &target_name[..target_name.len() - 4]
    } else {
        &target_name
    };

    let matching_processes: Vec<_> = sys
        .processes()
        .values()
        .filter(|p| {
            let name = p.name().to_string().to_lowercase();
            name == target_name
                || name == target_name_no_ext
                || (name.ends_with(".exe") && &name[..name.len() - 4] == target_name_no_ext)
        })
        .collect();

    if matching_processes.is_empty() {
        return 0;
    }

    // Pick the process with the highest memory usage as a heuristic for the main process
    match matching_processes.iter().max_by_key(|p| p.memory()) {
        Some(best) => best.pid().as_u32(),
        None => 0,
    }
}

/// Validates that the selected encoder is available as a GStreamer element.
///
/// Encoder names are remapped from the generic `nv*enc` names to their `nvd3d11*enc`
/// counterparts. The D3D11 variants operate directly on GPU memory (D3D11 textures),
/// avoiding a costly GPU->CPU->GPU roundtrip that the non-D3D11 encoders would require.
/// This is critical for recording at high resolutions — a 4K NV12 frame is ~12MB, and
/// transferring that across the PCIe bus twice per frame would saturate bandwidth.
pub fn validate_encoder(v: &VideoSettings) -> Result<()> {
    let encoder_element = match v.encoder.as_str() {
        "nvav1enc" => "nvd3d11av1enc",
        "nvh264enc" => "nvd3d11h264enc",
        "nvh265enc" => "nvd3d11h265enc",
        _ => v.encoder.as_str(),
    };

    if gst::ElementFactory::find(encoder_element).is_none() {
        return Err(anyhow::anyhow!(
            "GStreamer element '{}' not found. Your GPU might not support this encoder or the plugin is missing.",
            encoder_element
        ));
    }
    Ok(())
}

/// Builds a GStreamer pipeline description string for screen recording.
///
/// The pipeline follows this data flow:
/// ```text
/// d3d11screencapturesrc -> videorate -> d3d11convert -> nvd3d11*enc -> parser -> splitmuxsink
///                                                                                  ^
///                          wasapi2src / appsrc -> audioconvert -> audioresample ----+
/// ```
///
/// Key design decisions in the muxer/sink configuration:
/// - `reset-muxer=true`: forces `isofmp4mux` to reset between segments, making each
///   `.m4s` file independently decodable. Without this, segments share state and can
///   only be played in sequence — breaking HLS and the rolling buffer delete strategy.
/// - `send-keyframe-requests=true` + `strict-gop=true`: forces the encoder to emit an
///   IDR frame at each segment boundary. HLS players can only start decoding from an
///   IDR frame, so without this, seeking to a segment boundary would show corruption
///   until the next natural keyframe.
/// - `decode-time-offset`: offsets DTS/PTS timestamps so that segments from multiple
///   recording sessions have monotonically increasing timestamps. This prevents the
///   HLS player from getting confused by timestamp discontinuities when sessions are
///   concatenated into a single playlist.
/// - `start-index`: continues segment numbering from existing files on disk so that
///   restarting a recording session doesn't overwrite previous segments.
pub fn generate_pipeline_string(
    v: &VideoSettings,
    game_path: &str,
    audio_tracks: &[AudioRouting],
    _mic: &MicSettings,
    target_hwnd: Option<u64>,
    _target_pid: Option<u32>,
    adapter_index: i32,
    _session_id: u64,
    ts_offset_ns: i64,
) -> String {
    let adapter_str = if adapter_index >= 0 {
        format!("adapter-index={}", adapter_index)
    } else {
        "".to_string()
    };

    let use_hardware = v.encoder.starts_with("nv");
    // Remap to D3D11 variants — see validate_encoder() for rationale.
    let encoder_element = match v.encoder.as_str() {
        "nvav1enc" => "nvd3d11av1enc",
        "nvh264enc" => "nvd3d11h264enc",
        "nvh265enc" => "nvd3d11h265enc",
        _ => v.encoder.as_str(),
    };

    let (res_width, res_height) = parse_res(&v.resolution);

    let encoder_settings = if use_hardware {
        let gop = v.fps * 2;
        let base = format!(
            "gop-size={} bframes={} zerolatency={} preset={} rc-lookahead={} spatial-aq={} temporal-aq={} strict-gop=true {}",
            gop, v.bframes, v.zero_latency, v.preset,
            if v.lookahead { v.lookahead_frames } else { 0 },
            v.spatial_aq, v.temporal_aq,
            adapter_str
        );
        match v.rate_control_index {
            0 => format!(
                "rc-mode=constqp qp-const-i={} qp-const-p={} qp-const-b={} {}",
                v.cq_level, v.cq_level, v.cq_level, base
            ),
            1 => format!("rc-mode=vbr bitrate={} {}", v.bitrate_kbps, base),
            _ => format!("bitrate={} {}", v.bitrate_kbps, base),
        }
    } else {
        let gop = v.fps * 2;
        match v.rate_control_index {
            0 => format!(
                "pass=quant quantizer={} key-int-max={} tune=zerolatency",
                v.cq_level, gop
            ),
            _ => format!(
                "bitrate={} tune=zerolatency key-int-max={}",
                v.bitrate_kbps, gop
            ),
        }
    };

    let encoder_core = if use_hardware {
        format!("{} {}", encoder_element, encoder_settings)
    } else {
        format!("x264enc {} speed-preset=ultrafast", encoder_settings)
    };

    // Hardware encoders consume D3D11Memory directly (zero-copy). Software encoders
    // (x264) need system memory, so we must d3d11download to pull frames off the GPU.
    let conversion_and_download = if use_hardware {
        format!(
            "d3d11convert method=nearest-neighbour add-borders=false msaa=disabled ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={}",
            res_width, res_height
        )
    } else {
        format!(
            "d3d11convert method=nearest-neighbour add-borders=false msaa=disabled ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={} ! d3d11download ! video/x-raw,format=NV12",
            res_width, res_height
        )
    };

    let parser = if v.encoder.contains("av1") {
        "av1parse"
    } else if v.encoder.contains("h265") || v.encoder.contains("hevc") {
        "h265parse config-interval=-1"
    } else {
        "h264parse config-interval=-1"
    };
    
    let source = if let Some(hwnd) = target_hwnd {
        format!(
            "d3d11screencapturesrc {} capture-api=wgc window-handle={} show-cursor=true do-timestamp=true",
            adapter_str, hwnd
        )
    } else {
        format!(
            "d3d11screencapturesrc {} capture-api=dxgi show-cursor=true do-timestamp=true",
            adapter_str
        )
    };

    let mut pipeline = format!(
        "{} ! videorate ! video/x-raw(memory:D3D11Memory),framerate={}/1 ! queue leaky=downstream max-size-time=100000000 ! {} ! queue max-size-time=200000000 ! {} ! {} ! identity ts-offset=0 ! queue max-size-time=200000000 ! sink.video",
        source, v.fps, conversion_and_download, encoder_core, parser
    );

    for (i, track) in audio_tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }

        let src = if track.source_type == "System" {
            "wasapi2src loopback=true low-latency=true provide-clock=false ! level interval=100000000 post-messages=true".to_string()
        } else if track.source_type == "Mic" {
            // Tier 2: Shared Context - The main app will feed this appsrc from the MicProvider
            format!(
                "appsrc name=mic_src_{} format=time is-live=true do-timestamp=true",
                i
            )
        } else if track.source_type == "App" {
            format!(
                "appsrc name=audio_app_{} format=time is-live=true do-timestamp=true",
                i
            )
        } else {
            continue;
        };

        pipeline.push_str(&format!(
            " {} ! queue ! audioconvert ! audioresample ! audio/x-raw,channels=2,format=S16LE,layout=interleaved,rate=48000 ! volume volume={} name=vol_{} ! sink.audio_{}", 
            src, track.volume, i, i
        ));
    }

    // Find highest index
    let mut highest_index = 0;
    let mut found_any = false;
    if let Ok(entries) = std::fs::read_dir(game_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("seg_") && name.ends_with(".m4s") {
                let stem = &name[4..name.len()-4];
                let first_part = stem.split('_').next().unwrap_or(stem);
                if let Ok(idx) = first_part.parse::<u64>() {
                    if idx >= highest_index {
                        highest_index = idx;
                        found_any = true;
                    }
                }
            }
        }
    }
    let start_index = if found_any { highest_index + 1 } else { 0 };

    pipeline.push_str(&format!(
        " splitmuxsink name=sink muxer=\"isofmp4mux fragment-duration=1000000000 decode-time-offset={}\" location=\"{}/seg_%010d.m4s\" max-size-time=6000000000 max-files=300 start-index={} send-keyframe-requests=true reset-muxer=true",
        ts_offset_ns,
        game_path.trim_end_matches(&['\\', '/'][..]),
        start_index
    ));

    log::info!("Generated GStreamer Pipeline:\n{}", pipeline);

    pipeline
}

/// Captures audio from a specific application via WASAPI process loopback and feeds
/// it into a GStreamer `appsrc`.
///
/// Architecture: WASAPI -> ring buffer -> appsrc
///
/// WASAPI delivers audio in variable-size packets at its own cadence (typically 10ms
/// intervals, but timing is not guaranteed). GStreamer expects a steady stream of
/// samples aligned to its internal clock. The lock-free ring buffer bridges this gap:
/// - The WASAPI side pushes packets as they arrive (producer)
/// - The GStreamer side pulls accumulated samples each iteration (consumer)
/// - If WASAPI goes silent for >20ms (e.g., the app pauses audio), we inject silence
///   frames to prevent GStreamer from starving and dropping the audio stream entirely
///
/// The silence injection threshold (20ms) matches WASAPI's typical packet interval
/// so we only inject when there's a genuine gap, not just normal timing jitter.
pub fn start_app_capture(app_name: String, explicit_pid: Option<u32>, appsrc: AppSrc) -> Result<()> {
    if wasapi::initialize_mta().is_err() {
        let _ = appsrc.end_of_stream();
        return Ok(());
    }
    let mut sys = System::new_all();
    sys.refresh_processes();

    let target_pid = if let Some(pid) = explicit_pid {
        pid
    } else {
        let target_name = app_name.to_lowercase();
        let target_name_no_ext = if target_name.ends_with(".exe") {
            &target_name[..target_name.len() - 4]
        } else {
            &target_name
        };

        let mut matching_processes: Vec<_> = sys
            .processes()
            .values()
            .filter(|p| {
                let name = p.name().to_string().to_lowercase();
                name == target_name
                    || name == target_name_no_ext
                    || (name.ends_with(".exe") && &name[..name.len() - 4] == target_name_no_ext)
            })
            .collect();

        // Fuzzy fallback
        if matching_processes.is_empty() {
            matching_processes = sys
                .processes()
                .values()
                .filter(|p| {
                    let name = p.name().to_string().to_lowercase();
                    let name_no_ext = if name.ends_with(".exe") { &name[..name.len() - 4] } else { &name };
                    target_name.contains(name_no_ext)
                })
                .collect();
        }

        if matching_processes.is_empty() {
            log::warn!("[AppCapture] No process found for '{}'", app_name);
            let _ = appsrc.end_of_stream();
            return Ok(());
        }

        let pids: Vec<u32> = matching_processes
            .iter()
            .map(|p| p.pid().as_u32())
            .collect();
        let root_process = matching_processes
            .iter()
            .find(|p| {
                if let Some(parent_pid) = p.parent() {
                    !pids.contains(&parent_pid.as_u32())
                } else {
                    true
                }
            })
            .copied()
            .unwrap_or(matching_processes[0]);

        root_process.pid().as_u32()
    };

    let fallback_format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);

    let mut client = AudioClient::new_application_loopback_client(target_pid, true)
        .map_err(|e| anyhow::anyhow!("Failed to create loopback client: {}", e))?;

    let (fmt, _) = match client.get_mixformat() {
        Ok(f) => (f, false),
        Err(_) => (fallback_format.clone(), true),
    };

    client
        .initialize_client(
            &fmt,
            &Direction::Capture,
            &StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 1000000,
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to initialize audio client: {}", e))?;

    let capture = client.get_audiocaptureclient()?;
    client.start_stream()?;

    let (rate, channels, block_align) = (
        fmt.get_samplespersec(),
        fmt.get_nchannels(),
        fmt.get_blockalign() as usize,
    );
    let caps = gst::Caps::builder("audio/x-raw")
        .field(
            "format",
            &if fmt.get_bitspersample() == 32 {
                "F32LE"
            } else {
                "S16LE"
            },
        )
        .field("rate", &(rate as i32))
        .field("channels", &(channels as i32))
        .field("layout", &"interleaved")
        .build();

    appsrc.set_caps(Some(&caps));
    appsrc.set_format(gst::Format::Time);

    // Lock-free ring buffer sized for 1 second of audio. This is generous — typical
    // WASAPI latency is 10-20ms — but protects against GC pauses or scheduling delays
    // in the game process that could cause a burst of late packets.
    let rb = HeapRb::<u8>::new(rate as usize * channels as usize * 4);
    let (mut prod, mut cons) = rb.split();

    let (silence_interval, silence_bytes) = (
        Duration::from_millis(20),
        (rate as u64 * 20 / 1000) as usize * block_align,
    );
    let mut last_data = std::time::Instant::now();

    loop {
        // 1. Read from WASAPI
        let mut did_read = false;
        while let Ok(Some(frames)) = capture.get_next_packet_size() {
            if frames == 0 {
                break;
            }
            let size = frames as usize * block_align;
            let mut buf = vec![0u8; size];
            if capture.read_from_device(&mut buf).is_ok() {
                let _ = prod.push_slice(&buf);
                did_read = true;
                last_data = std::time::Instant::now();
            }
        }

        if !did_read && last_data.elapsed() >= silence_interval {
            let silence = vec![0u8; silence_bytes];
            let _ = prod.push_slice(&silence);
            last_data = std::time::Instant::now();
        }

        // 2. Push to GStreamer
        let avail = cons.occupied_len();
        if avail > 0 {
            let mut buf = vec![0u8; avail];
            let read = cons.pop_slice(&mut buf);
            if read > 0 {
                let mut gst_buf = match gst::Buffer::with_size(read) {
                    Ok(buf) => buf,
                    Err(_) => { log::warn!("[Audio] Failed to allocate GStreamer buffer"); continue; }
                };
                if let Some(buf_ref) = gst_buf.get_mut() {
                    if let Ok(mut map) = buf_ref.map_writable() {
                        map.copy_from_slice(&buf[..read]);
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
                if appsrc.push_buffer(gst_buf).is_err() {
                    break;
                }
            }
        }

        if !did_read && avail == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    let _ = appsrc.end_of_stream();
    Ok(())
}

/// Exports the last `seconds` of the current recording as an instant replay clip.
///
/// Called from the tray hotkey thread — has no access to UI state, so it reads
/// `recording_source` from `AppState` and performs the export entirely via FFmpeg.
/// Shows a tray balloon notification on success/failure.
pub fn export_instant_replay(
    app_state: &std::sync::Arc<crate::state::AppState>,
    seconds: i32,
) {
    let source_name = match app_state.recording_source.lock().clone() {
        Some(s) => s,
        None => {
            log::warn!("[InstantReplay] No active recording source");
            return;
        }
    };

    let safe_title = crate::utils::clean_title(&source_name);
    let storage_root = crate::utils::get_storage_root();

    // Regenerate ffconcat so it includes the very latest segments
    if crate::utils::generate_ffconcat_playlist(&source_name).is_none() {
        log::error!("[InstantReplay] No segments found for '{}'", source_name);
        crate::tray::show_notification("Instant Replay Failed", "No recording segments found.");
        return;
    }

    let playlist_path = storage_root.join(&safe_title).join("view.ffconcat");
    if !playlist_path.exists() {
        crate::tray::show_notification("Instant Replay Failed", "Playlist not found.");
        return;
    }

    // Calculate total duration from the current recording to determine the start point
    let total_duration = *app_state.current_recording_duration.lock();
    let start = (total_duration - seconds as f64).max(0.0);
    let end = total_duration;

    if end - start < 0.5 {
        crate::tray::show_notification("Instant Replay", "Not enough recording data yet.");
        return;
    }

    let clips_dir = storage_root.join("Clips").join(&safe_title);
    let _ = std::fs::create_dir_all(&clips_dir);

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let output_path = clips_dir.join(format!("replay_{}_{}.mp4", safe_title, timestamp));

    let ffmpeg_path = crate::utils::get_ffmpeg_path();
    if !ffmpeg_path.exists() {
        crate::tray::show_notification("Instant Replay Failed", "ffmpeg.exe not found.");
        return;
    }

    log::info!(
        "[InstantReplay] Exporting {:.1}s clip ({:.1} - {:.1}) from '{}'",
        end - start, start, end, source_name
    );

    let result = std::process::Command::new(&ffmpeg_path)
        .arg("-y")
        .arg("-ss").arg(format!("{:.3}", start))
        .arg("-to").arg(format!("{:.3}", end))
        .arg("-f").arg("concat")
        .arg("-safe").arg("0")
        .arg("-i").arg(&playlist_path)
        .arg("-map").arg("0:v:0")
        .arg("-map").arg("0:a?")
        .arg("-c:v").arg("copy")
        .arg("-c:a").arg("aac")
        .arg("-b:a").arg("320k")
        .arg("-movflags").arg("+faststart")
        .arg(&output_path)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            // Extract thumbnail
            let mut thumb_path = output_path.clone();
            thumb_path.set_extension("jpg");
            let thumb_time = start + (end - start) / 2.0;
            let _ = std::process::Command::new(&ffmpeg_path)
                .arg("-y")
                .arg("-ss").arg(format!("{:.3}", thumb_time))
                .arg("-f").arg("concat")
                .arg("-safe").arg("0")
                .arg("-i").arg(&playlist_path)
                .arg("-vframes").arg("1")
                .arg("-q:v").arg("2")
                .arg(&thumb_path)
                .output();

            let duration_str = format!("{}s", seconds);
            crate::tray::show_notification(
                "Instant Replay Saved",
                &format!("Last {} saved to Clips/{}", duration_str, safe_title),
            );
            log::info!("[InstantReplay] Saved to {:?}", output_path);
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            log::error!("[InstantReplay] FFmpeg failed: {}", err);
            crate::tray::show_notification("Instant Replay Failed", "FFmpeg returned an error.");
        }
        Err(e) => {
            log::error!("[InstantReplay] Could not run FFmpeg: {}", e);
            crate::tray::show_notification("Instant Replay Failed", "Could not run ffmpeg.exe");
        }
    }
}
