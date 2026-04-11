//! Shared microphone input provider using a pub/sub architecture.
//!
//! A single WASAPI capture pipeline feeds raw audio to zero or more GStreamer
//! `appsrc` subscribers. This avoids opening multiple mic streams (which Windows
//! doesn't allow concurrently for the same device) and lets multiple recordings
//! share one mic without contention.
//!
//! The pipeline runs on a dedicated high-priority thread to minimize audio
//! dropout risk — the OS audio scheduler is real-time, and if we can't drain
//! the WASAPI buffer fast enough we'll lose frames.

use dashmap::DashMap;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use parking_lot::Mutex;
use std::sync::Arc;

/// Handle to a running mic capture pipeline. Subscribers are keyed by a unique
/// ID (typically the recording session ID) and receive a copy of every audio
/// buffer produced by the pipeline.
pub struct MicProvider {
    pub subscribers: Arc<DashMap<u64, AppSrc>>,
}

/// Spawn the mic capture pipeline on a dedicated thread.
///
/// Initializes a WASAPI → GStreamer pipeline that captures from `device_id`,
/// resamples to 48kHz stereo F32LE, and fans out buffers to all registered
/// subscribers. The `provider_storage` is populated once the pipeline is ready,
/// allowing callers to register `AppSrc` subscribers after the fact.
pub fn start_mic_provider(
    provider_storage: Arc<Mutex<Option<Arc<MicProvider>>>>,
    device_id: String,
) {
    let _ = std::thread::Builder::new()
        .name("Luma Mic Provider".to_string())
        .spawn(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Threading::*;
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            }

            let _ = gst::init();
        
        let subscribers = Arc::new(DashMap::new());
        {
            let mut storage = provider_storage.lock();
            *storage = Some(Arc::new(MicProvider {
                subscribers: subscribers.clone(),
            }));
        }

        let device_prop = if device_id.is_empty() || device_id == "Default" {
            String::new()
        } else {
            format!("device='{}'", device_id)
        };

        let src_str = format!(
            "wasapi2src {} low-latency=true provide-clock=false ! queue ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,rate=48000,channels=2 ! appsink name=mic_sink emit-signals=true max-buffers=5 drop=true",
            device_prop
        );

        if let Ok(pipeline) = gst::parse::launch(&src_str) {
            let pipeline = match pipeline.downcast::<gst::Pipeline>() {
                Ok(p) => p,
                Err(_) => { log::error!("[MicProvider] Failed to cast to Pipeline"); return; }
            };
            let appsink = match pipeline.by_name("mic_sink").and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok()) {
                Some(s) => s,
                None => { log::error!("[MicProvider] Failed to get mic_sink appsink"); return; }
            };

            let subs_clone = subscribers.clone();
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        if let Ok(sample) = sink.pull_sample() {
                            if let Some(in_buffer) = sample.buffer() {
                                // Shallow-copy the buffer for each subscriber and strip PTS/DTS.
                                // Why: each subscriber's pipeline has its own clock. If we forwarded
                                // the original timestamps, subscribers that started later would see
                                // a massive PTS jump and either drop frames or desync audio/video.
                                // Letting each appsrc generate its own timestamps from its pipeline
                                // clock keeps everything aligned.
                                for entry in subs_clone.iter() {
                                    let appsrc = entry.value();
                                    let mut out_buffer = in_buffer.copy();
                                    let mut_buf = out_buffer.make_mut();
                                    mut_buf.set_pts(gst::ClockTime::NONE);
                                    mut_buf.set_dts(gst::ClockTime::NONE);
                                    let _ = appsrc.push_buffer(out_buffer);
                                }
                            }
                        }
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );

            let _ = pipeline.set_state(gst::State::Playing);
            let bus = match pipeline.bus() {
                Some(b) => b,
                None => { log::error!("[MicProvider] No bus on pipeline"); return; }
            };
            for msg in bus.iter_timed(gst::ClockTime::NONE) {
                use gst::MessageView;
                match msg.view() {
                    MessageView::Eos(..) => break,
                    MessageView::Error(err) => {
                        eprintln!("MicProvider error: {} ({:?})", err.error(), err.debug());
                        break;
                    }
                    _ => (),
                }
            }
        }
    });
}
