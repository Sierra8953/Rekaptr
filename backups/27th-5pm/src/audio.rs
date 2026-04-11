use dashmap::DashMap;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct MicProvider {
    pub subscribers: Arc<DashMap<u64, AppSrc>>,
}

pub fn start_mic_provider(
    provider_storage: Arc<Mutex<Option<Arc<MicProvider>>>>,
    device_id: String,
) {
    // We spawn a thread that acts as the "Single Source of Truth" for the Mic
    let _ = std::thread::Builder::new()
        .name("Luma Mic Provider".to_string())
        .spawn(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Threading::*;
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            }

            // Lazy init
            let _ = gst::init();
        
        let subscribers = Arc::new(DashMap::new());
        {
            let mut storage = provider_storage.lock();
            *storage = Some(Arc::new(MicProvider {
                subscribers: subscribers.clone(),
            }));
        }

        // Use GStreamer to capture from the Mic
        let src_str = format!(
            "wasapi2src device='{}' low-latency=true provide-clock=false ! queue ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,rate=48000,channels=2 ! appsink name=mic_sink emit-signals=true max-buffers=5 drop=true",
            device_id
        );

        if let Ok(pipeline) = gst::parse::launch(&src_str) {
            let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();
            let appsink = pipeline
                .by_name("mic_sink")
                .unwrap()
                .downcast::<gstreamer_app::AppSink>()
                .unwrap();

            // Phase 4: Create a BufferPool to avoid allocating new memory for every audio chunk
            let pool = gst::BufferPool::new();
            let mut config = pool.config();
            let caps = gst::Caps::builder("audio/x-raw")
                .field("format", "F32LE")
                .field("rate", 48000)
                .field("channels", 2)
                .build();
            // Estimate size for ~10ms of 48kHz stereo float32 audio (480 samples * 2 channels * 4 bytes) = 3840 bytes. We use 8192 to be safe.
            config.set_params(Some(&caps), 8192, 10, 50);
            let _ = pool.set_config(config);
            let _ = pool.set_active(true);

            let subs_clone = subscribers.clone();
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        if let Ok(sample) = sink.pull_sample() {
                            if let Some(in_buffer) = sample.buffer() {
                                let in_map = in_buffer.map_readable().unwrap();
                                
                                // Fan out to all subscribers
                                for entry in subs_clone.iter() {
                                    let appsrc = entry.value();
                                    
                                    // Acquire a recycled buffer from the pool instead of a fresh allocation
                                    if let Ok(mut out_buffer) = pool.acquire_buffer(None) {
                                        // Ensure the buffer is big enough
                                        let size = in_map.len();
                                        if out_buffer.size() < size {
                                            let mut_buf = out_buffer.get_mut().unwrap();
                                            mut_buf.set_size(size);
                                        }

                                        // Copy the data
                                        let mut out_map = out_buffer.get_mut().unwrap().map_writable().unwrap();
                                        out_map.copy_from_slice(in_map.as_slice());
                                        drop(out_map);

                                        // Push the recycled buffer to the appsrc
                                        let _ = appsrc.push_buffer(out_buffer);
                                    } else {
                                        // Fallback if the pool fails (rare)
                                        let _ = appsrc.push_buffer(in_buffer.copy_deep().unwrap());
                                    }
                                }
                            }
                        }
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );

            let _ = pipeline.set_state(gst::State::Playing);

            // Keep this thread alive to manage the pipeline
            let bus = pipeline.bus().unwrap();
            for _msg in bus.iter_timed(gst::ClockTime::NONE) {
                // Handle errors/EOS if needed, or just keep running
            }
        }
    });
}
