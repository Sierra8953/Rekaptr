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
    let best = matching_processes.iter().max_by_key(|p| p.memory()).unwrap();
    best.pid().as_u32()
}

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
        format!("adapter={}", adapter_index)
    } else {
        "".to_string()
    };

    let use_hardware = v.encoder.starts_with("nv");
    let encoder_element = match v.encoder.as_str() {
        "nvav1enc" => "nvd3d11av1enc",
        "nvh264enc" => "nvd3d11h264enc",
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

    let hls_parser = if v.encoder.contains("h264") {
        "h264parse config-interval=-1"
    } else if v.encoder.contains("h265") || v.encoder.contains("hevc") {
        "h265parse config-interval=-1"
    } else if v.encoder.contains("av1") {
        "av1parse"
    } else {
        "h264parse config-interval=-1"
    };

    let mut pipeline = format!(
        "{} ! videorate ! video/x-raw(memory:D3D11Memory),framerate={}/1 ! queue leaky=downstream max-size-buffers=5 ! {} ! queue max-size-buffers=10 ! {} ! {} ! {} ! identity ts-offset=0 ! queue ! sink.video",
        source, v.fps, conversion_and_download, encoder_core, parser, hls_parser
    );

    for (i, track) in audio_tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }

        let src = if track.source_type == "System" {
            "wasapi2src loopback=true low-latency=true ! level interval=100000000 post-messages=true".to_string()
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
        " splitmuxsink name=sink muxer=\"isofmp4mux fragment-duration=1000000000 decode-time-offset={}\" location=\"{}/seg_%010d.m4s\" max-size-time=6000000000 start-index={} send-keyframe-requests=true reset-muxer=true",
        ts_offset_ns,
        game_path.trim_end_matches(&['\\', '/'][..]),
        start_index
    ));

    log::info!("Generated GStreamer Pipeline:\n{}", pipeline);

    pipeline
}

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

    // Tier 1: Lock-Free Ring Buffer
    let rb = HeapRb::<u8>::new(rate as usize * channels as usize * 4); // 1 second buffer
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
                let mut gst_buf = gst::Buffer::with_size(read).unwrap();
                gst_buf
                    .get_mut()
                    .unwrap()
                    .map_writable()
                    .unwrap()
                    .copy_from_slice(&buf[..read]);
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
