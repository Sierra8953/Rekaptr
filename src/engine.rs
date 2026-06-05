use crate::config::{AudioRouting, MicSettings, VideoSettings};
use sysinfo::System;
use gstreamer as gst;
use gstreamer_app::AppSrc;
use anyhow::{Result, Context};
use wasapi::*;
use ringbuf::HeapRb;
use ringbuf::traits::{Split, Producer, Consumer, Observer};
use std::time::Duration;

// D3DKMT GPU scheduling priority classes
#[allow(dead_code)]
#[repr(i32)]
enum D3DKmtSchedulingPriorityClass {
    Idle = 0,
    BelowNormal = 1,
    Normal = 2,
    AboveNormal = 3,
    High = 4,
    Realtime = 5,
}

/// Boosts the process-level GPU scheduling priority so the encoder gets
/// preferential access to GPU compute/video-engine resources over games.
///
/// Uses two mechanisms:
/// 1. `D3DKMTSetProcessSchedulingPriorityClass` — tells the Windows GPU
///    scheduler to prioritise this process's GPU work (HIGH priority).
/// 2. `IDXGIDevice::SetGPUThreadPriority(7)` — raises the per-device
///    thread priority to maximum on the D3D11 device shared with GStreamer.
pub fn boost_gpu_priority() {
    // --- Process-level GPU priority via D3DKMT (gdi32.dll) ---
    unsafe {
        use windows::Win32::System::LibraryLoader::{LoadLibraryW, GetProcAddress};
        use windows::Win32::System::Threading::GetCurrentProcess;
        use windows::Win32::Foundation::HANDLE;

        let lib = LoadLibraryW(windows::core::w!("gdi32.dll"));
        let lib = match lib {
            Ok(h) => h,
            Err(e) => {
                log::warn!("[GPU Priority] Failed to load gdi32.dll: {}", e);
                return;
            }
        };

        type SetPriorityFn = unsafe extern "system" fn(
            handle: HANDLE,
            priority: i32,
        ) -> i32;

        let proc = GetProcAddress(
            lib.into(),
            windows::core::s!("D3DKMTSetProcessSchedulingPriorityClass"),
        );
        if let Some(func) = proc {
            let set_priority: SetPriorityFn = std::mem::transmute(func);
            let current_process = HANDLE(GetCurrentProcess().0);

            // Try Realtime first (requires admin), fall back to High
            let status = set_priority(
                current_process,
                D3DKmtSchedulingPriorityClass::Realtime as i32,
            );
            if status == 0 {
                log::info!("[GPU Priority] Process GPU scheduling priority set to REALTIME (admin)");
            } else {
                let status = set_priority(
                    current_process,
                    D3DKmtSchedulingPriorityClass::High as i32,
                );
                if status == 0 {
                    log::info!("[GPU Priority] Process GPU scheduling priority set to HIGH");
                } else {
                    log::warn!(
                        "[GPU Priority] D3DKMTSetProcessSchedulingPriorityClass returned 0x{:08x}",
                        status
                    );
                }
            }
        }
    }
}

/// Sets GPU thread priority to maximum (7) on the D3D11 device used by the
/// GStreamer encoder pipeline. Call this after obtaining the D3D11 device handle.
///
/// # Safety
/// `d3d11_device_ptr` must be a valid `ID3D11Device` pointer. This function borrows
/// the pointer without taking ownership (no AddRef/Release). The caller must ensure
/// the device remains alive for the duration of this call.
pub fn boost_device_gpu_priority(d3d11_device_ptr: *mut std::ffi::c_void) {
    if d3d11_device_ptr.is_null() {
        return;
    }
    unsafe {
        use windows::Win32::Graphics::Dxgi::IDXGIDevice;
        use windows::Win32::Graphics::Direct3D11::ID3D11Device;
        use windows::core::Interface;

        // Wrap the raw pointer without taking ownership. ManuallyDrop prevents
        // the Drop impl from calling Release on the borrowed pointer.
        let d3d11_device = ID3D11Device::from_raw(d3d11_device_ptr);
        let d3d11_ref = std::mem::ManuallyDrop::new(d3d11_device);

        match d3d11_ref.cast::<IDXGIDevice>() {
            Ok(dxgi_device) => {
                // Priority range: -7 (lowest) to 7 (highest)
                if let Err(e) = dxgi_device.SetGPUThreadPriority(7) {
                    log::warn!("[GPU Priority] SetGPUThreadPriority failed: {}", e);
                } else {
                    log::info!("[GPU Priority] D3D11 device GPU thread priority set to 7 (maximum)");
                }
            }
            Err(e) => {
                log::warn!("[GPU Priority] Failed to cast D3D11Device to IDXGIDevice: {}", e);
            }
        }
    }
}

/// Enumerate available audio devices via WASAPI.
/// Returns a list of (device_id, friendly_name) tuples.
pub fn enumerate_audio_devices(capture: bool) -> Vec<(String, String)> {
    let mut devices = vec![("Default".to_string(), "Default".to_string())];

    let direction = if capture {
        wasapi::Direction::Capture
    } else {
        wasapi::Direction::Render
    };

    if wasapi::initialize_mta().is_err() {
        return devices;
    }

    let enumerator = match wasapi::DeviceEnumerator::new() {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[Audio] Failed to create device enumerator: {}", e);
            return devices;
        }
    };

    match enumerator.get_device_collection(&direction) {
        Ok(collection) => {
            let count = collection.get_nbr_devices().unwrap_or(0);
            for i in 0..count {
                if let Ok(device) = collection.get_device_at_index(i) {
                    let name = device.get_friendlyname().unwrap_or_else(|_| format!("Device {}", i));
                    let id = device.get_id().unwrap_or_else(|_| format!("device_{}", i));
                    devices.push((id, name));
                }
            }
        }
        Err(e) => {
            log::warn!("[Audio] Failed to enumerate devices: {}", e);
        }
    }

    devices
}

/// Resolve a device identifier to a WASAPI device ID.
/// If the value is already a device ID (contains '{'), returns it as-is.
/// Otherwise treats it as a friendly name and looks up the matching ID.
pub fn resolve_device_id(value: &str, capture: bool) -> String {
    if value.is_empty() || value == "Default" || value.contains('{') {
        return value.to_string();
    }
    // Legacy config stored friendly name — find the matching device ID
    let devices = enumerate_audio_devices(capture);
    if let Some((id, _)) = devices.iter().find(|(_, name)| name == value) {
        log::info!("[Audio] Resolved device name '{}' to ID '{}'", value, id);
        id.clone()
    } else {
        log::warn!("[Audio] Could not resolve device name '{}' to an ID, using as-is", value);
        value.to_string()
    }
}

pub fn parse_res(res: &str) -> (i32, i32) {
    let parts: Vec<&str> = res.split('x').collect();
    if parts.len() == 2 {
        if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
            return (w, h);
        }
    }
    (1920, 1080)
}

pub fn diagnose_pipeline_failure(pipeline_str: &str, err: &gst::glib::Error) {
    let kind = err.kind::<gst::ParseError>()
        .map(|k| format!("{:?}", k))
        .unwrap_or_else(|| "unknown".into());
    log::error!("error.domain={} error.kind={} error.message={}",
        err.domain().as_str(), kind, err.message());
    log::error!("pipeline={}", pipeline_str);

    let (major, minor, micro, nano) = gst::version();
    log::error!("gstreamer.version={}.{}.{}.{}", major, minor, micro, nano);
    for var in ["GST_PLUGIN_PATH", "GST_PLUGIN_SYSTEM_PATH", "GST_REGISTRY"] {
        log::error!("env.{}={}", var, std::env::var(var).unwrap_or_default());
    }

    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut prev_was_separator = true;
    for tok in pipeline_str.split_whitespace() {
        if tok == "!" || tok == "(" || tok == ")" {
            prev_was_separator = true;
            continue;
        }
        if !prev_was_separator { continue; }
        prev_was_separator = false;
        if tok.contains('=') || tok.contains('/') || tok.contains('"') { continue; }
        let name = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string();
        if name.is_empty() || !name.chars().next().unwrap().is_ascii_lowercase() { continue; }
        seen.insert(name);
    }

    for name in &seen {
        match gst::ElementFactory::find(name) {
            Some(_) => log::error!("factory.{}=found", name),
            None    => log::error!("factory.{}=missing", name),
        }
    }

    if let Some(idx) = pipeline_str.find("muxer=\"") {
        let rest = &pipeline_str[idx + "muxer=\"".len()..];
        if let Some(end) = rest.find('"') {
            let factory = rest[..end].split_whitespace().next().unwrap_or("");
            match gst::ElementFactory::find(factory) {
                Some(f) => {
                    for tmpl in f.static_pad_templates() {
                        if tmpl.direction() == gst::PadDirection::Sink {
                            log::error!("muxer.{}.sink_template[{}]={}",
                                factory, tmpl.name_template(), tmpl.caps());
                        }
                    }
                }
                None => log::error!("muxer.{}=missing", factory),
            }
        }
    }
}

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

pub fn generate_pipeline_string(
    v: &VideoSettings,
    game_path: &str,
    audio_tracks: &[AudioRouting],
    _mic: &MicSettings,
    target_hwnd: Option<u64>,
    _target_pid: Option<u32>,
    adapter_index: i32,
    session_id: u64,
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
            0 => if v.encoder == "nvav1enc" {
                format!(
                    "rc-mode=vbr const-quality={} bitrate=0 {}",
                    v.cq_level, base
                )
            } else {
                format!(
                    "rc-mode=constqp qp-const-i={} qp-const-p={} qp-const-b={} {}",
                    v.cq_level, v.cq_level, v.cq_level, base
                )
            },
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
        "{} ! videorate ! video/x-raw(memory:D3D11Memory),framerate={}/1 ! queue leaky=downstream max-size-buffers=5 ! {} ! queue max-size-buffers=10 ! {} ! {} ! identity ts-offset=0 ! queue ! sink.video",
        source, v.fps, conversion_and_download, encoder_core, parser
    );

    for (i, track) in audio_tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }

        let src = if track.source_type == "System" {
            "wasapi2src loopback=true low-latency=true provide-clock=false ! level interval=100000000 post-messages=true".to_string()
        } else if track.source_type == "Mic" {
            // Tier 2: Shared Context - The main app will feed this appsrc from the MicProvider.
            // Caps must match the mic provider output (F32LE, 48kHz, stereo) so audioconvert
            // can negotiate the conversion to S16LE downstream.
            format!(
                "appsrc name=mic_src_{} format=time is-live=true do-timestamp=true caps=audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved",
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
        " splitmuxsink name=sink muxer=\"isofmp4mux fragment-duration=1000000000 decode-time-offset={}\" location=\"{}/seg_%010d_{}.m4s\" max-size-time=6000000000 start-index={} send-keyframe-requests=true reset-muxer=true",
        ts_offset_ns,
        game_path.trim_end_matches(&['\\', '/'][..]),
        session_id,
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
        .context("Failed to create loopback client")?;

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
        .context("Failed to initialize audio client")?;

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
                let mut gst_buf = match gst::Buffer::with_size(read) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                if let Some(buf_ref) = gst_buf.get_mut() {
                    if let Ok(mut map) = buf_ref.map_writable() {
                        map.copy_from_slice(&buf[..read]);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_res_standard() {
        assert_eq!(parse_res("1920x1080"), (1920, 1080));
        assert_eq!(parse_res("2560x1440"), (2560, 1440));
        assert_eq!(parse_res("3840x2160"), (3840, 2160));
        assert_eq!(parse_res("1280x720"), (1280, 720));
    }

    #[test]
    fn test_parse_res_fallback() {
        // Invalid formats should fall back to 1920x1080
        assert_eq!(parse_res(""), (1920, 1080));
        assert_eq!(parse_res("invalid"), (1920, 1080));
        assert_eq!(parse_res("1920"), (1920, 1080));
        assert_eq!(parse_res("axb"), (1920, 1080));
        assert_eq!(parse_res("1920x"), (1920, 1080));
        assert_eq!(parse_res("x1080"), (1920, 1080));
    }

    #[test]
    fn test_resolve_pid_empty_input() {
        assert_eq!(resolve_pid(""), 0);
        assert_eq!(resolve_pid("Default"), 0);
    }
}
