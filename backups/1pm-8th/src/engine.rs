use crate::config::{AudioRouting, MicSettings, VideoSettings};
use sysinfo::System;

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
}

pub fn generate_pipeline_string(
    v: &VideoSettings,
    game_path: &str, // Now represents the root game directory
    audio_tracks: &[AudioRouting],
    _mic: &MicSettings,
    target_hwnd: Option<u64>,
    target_pid: Option<u32>,
    adapter_index: i32,
    _session_id: u64, // No longer used in filename, but kept for signature compatibility
    ts_offset_ns: i64, 
) -> String {
    // ... (adapter_str and encoder logic remains the same)

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
    let max_files = if v.retention_minutes > 0 {
        v.retention_minutes * 30
    } else {
        0
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
            "wasapi2src loopback=true low-latency=true provide-clock=false do-timestamp=true ! level interval=100000000 post-messages=true".to_string()
        } else if track.source_type == "Mic" {
            format!(
                "appsrc name=mic_src_{} caps=\"audio/x-raw,format=F32LE,rate=48000,channels=2\" format=time do-timestamp=true block=false max-bytes=2000000",
                i
            )
        } else if track.source_type == "App" {
            let pid = if track.device_name.is_empty() || track.device_name == "Default" {
                target_pid.unwrap_or(0)
            } else {
                resolve_pid(&track.device_name)
            };

            if pid > 0 {
                format!(
                    "wasapi2src loopback=true low-latency=true provide-clock=false do-timestamp=true loopback-mode=1 loopback-target-pid={} ! level interval=100000000 post-messages=true",
                    pid
                )
            } else {
                "audiotestsrc wave=silence do-timestamp=true ! audio/x-raw,rate=48000,channels=2".to_string()
            }
        } else {
            continue;
        };

        pipeline.push_str(&format!(
            " {} ! queue ! audioconvert ! audioresample ! audio/x-raw,channels=2,rate=48000 ! volume volume={} name=vol_{} ! queue ! audioconvert ! avenc_aac ! aacparse ! identity ts-offset=0 ! queue ! sink.audio_{}",
            src, track.volume, i, i
        ));
    }

    // Find the highest existing segment index to continue the global counter
    let mut highest_index = 0;
    if let Ok(entries) = std::fs::read_dir(game_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("seg_") && name.ends_with(".m4s") {
                // Parse the global index from seg_%010d.m4s
                if let Ok(idx) = name[4..name.len()-4].parse::<u64>() {
                    if idx > highest_index {
                        highest_index = idx;
                    }
                }
            }
        }
    }
    let start_index = highest_index + 1;

    pipeline.push_str(&format!(
        " splitmuxsink name=sink muxer=\"isofmp4mux fragment-duration=1000000000 decode-time-offset={}\" location=\"{}/seg_%010d.m4s\" max-size-time=6000000000 max-files={} start-index={} send-keyframe-requests=true reset-muxer=true",
        ts_offset_ns,
        game_path.trim_end_matches(&['\\', '/'][..]),
        max_files,
        start_index
    ));

    pipeline
}
