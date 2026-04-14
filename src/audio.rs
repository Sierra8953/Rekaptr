use dashmap::DashMap;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct MicProvider {
    pub subscribers: Arc<DashMap<u64, AppSrc>>,
    /// Monotonic counter incremented when DSP settings change.
    /// The audio thread polls this to know when to reload config.
    pub settings_generation: Arc<AtomicU64>,
}

impl MicProvider {
    /// Call this from the UI thread when mic DSP settings change.
    pub fn notify_settings_changed(&self) {
        self.settings_generation.fetch_add(1, Ordering::Release);
    }
}

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

        let settings_generation = Arc::new(AtomicU64::new(0));
        let subscribers = Arc::new(DashMap::new());
        {
            let mut storage = provider_storage.lock();
            *storage = Some(Arc::new(MicProvider {
                subscribers: subscribers.clone(),
                settings_generation: settings_generation.clone(),
            }));
        }

        let device_id = crate::engine::resolve_device_id(&device_id, true);
        let device_prop = if device_id.is_empty() || device_id == "Default" {
            String::new()
        } else {
            format!("device=\"{}\"", device_id)
        };

        let config = crate::config::AppConfig::load();
        let denoise_element = if config.mic_settings.noise_suppression {
            "audiornnoise name=denoiser ! "
        } else {
            ""
        };

        let src_str = format!(
            "wasapi2src {} low-latency=true provide-clock=false ! queue ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,rate=48000,channels=2 ! {}appsink name=mic_sink emit-signals=true max-buffers=5 drop=true",
            device_prop,
            denoise_element
        );
        log::info!("[MicProvider] Pipeline: {}", src_str);

        if let Ok(pipeline) = gst::parse::launch(&src_str) {
            let pipeline = match pipeline.downcast::<gst::Pipeline>() {
                Ok(p) => p,
                Err(_) => { log::error!("[MicProvider] Failed to downcast pipeline"); return; }
            };
            let appsink = match pipeline.by_name("mic_sink")
                .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok()) {
                Some(s) => s,
                None => { log::error!("[MicProvider] Failed to find mic_sink element"); return; }
            };

            let subs_clone = subscribers.clone();
            let gen_clone = settings_generation.clone();
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample({
                        let mut dsp = crate::mic_dsp::MicDsp::new(48000.0, 2);
                        let config = crate::config::AppConfig::load();
                        dsp.load_settings(&config.mic_settings);
                        let mut last_gen = 0u64;

                        move |sink| {
                            // Hot-reload DSP settings when they change (lock-free check)
                            let current_gen = gen_clone.load(Ordering::Acquire);
                            if current_gen != last_gen {
                                last_gen = current_gen;
                                let config = crate::config::AppConfig::load();
                                dsp.load_settings(&config.mic_settings);
                            }

                            if let Ok(sample) = sink.pull_sample() {
                                if let Some(in_buffer) = sample.buffer() {
                                    // Copy buffer so we can process samples in-place
                                    let mut out_buffer = in_buffer.copy();
                                    {
                                        let mut_buf = out_buffer.make_mut();
                                        mut_buf.set_pts(gst::ClockTime::NONE);
                                        mut_buf.set_dts(gst::ClockTime::NONE);

                                        // Apply DSP to the raw F32LE samples
                                        if let Ok(mut map) = mut_buf.map_writable() {
                                            let samples: &mut [f32] = unsafe {
                                                std::slice::from_raw_parts_mut(
                                                    map.as_mut_ptr() as *mut f32,
                                                    map.len() / std::mem::size_of::<f32>(),
                                                )
                                            };
                                            dsp.process(samples);
                                        }
                                    }

                                    for entry in subs_clone.iter() {
                                        let appsrc = entry.value();
                                        let _ = appsrc.push_buffer(out_buffer.copy());
                                    }
                                }
                            }
                            Ok(gst::FlowSuccess::Ok)
                        }
                    })
                    .build(),
            );

            let _ = pipeline.set_state(gst::State::Playing);
            let bus = match pipeline.bus() {
                Some(b) => b,
                None => { log::error!("[MicProvider] Pipeline has no bus"); return; }
            };
            loop {
                use gst::MessageView;
                for msg in bus.iter_timed(gst::ClockTime::from_seconds(1)) {
                    match msg.view() {
                        MessageView::Eos(..) => {
                            log::info!("[MicProvider] Pipeline reached EOS");
                            let _ = pipeline.set_state(gst::State::Null);
                            return;
                        }
                        MessageView::Error(err) => {
                            log::error!("[MicProvider] Pipeline error: {} ({:?})", err.error(), err.debug());
                            let _ = pipeline.set_state(gst::State::Null);
                            return;
                        }
                        _ => (),
                    }
                }
                let (_, current, _) = pipeline.state(gst::ClockTime::from_mseconds(100));
                if current == gst::State::Null {
                    log::warn!("[MicProvider] Pipeline unexpectedly in Null state, exiting");
                    return;
                }
            }
        }
    });
}
