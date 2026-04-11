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
    std::thread::Builder::new()
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
        // We use a simple pipeline: wasapi2src -> appsink
        // The appsink will then "fan out" the buffers to all subscribers (appsrcs)

        let src_str = format!(
            "wasapi2src device='{}' low-latency=true provide-clock=false ! queue ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,rate=48000,channels=2 ! appsink name=mic_sink emit-signals=true",
            device_id
        );

        if let Ok(pipeline) = gst::parse::launch(&src_str) {
            let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();
            let appsink = pipeline
                .by_name("mic_sink")
                .unwrap()
                .downcast::<gstreamer_app::AppSink>()
                .unwrap();

            let subs_clone = subscribers.clone();
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        if let Ok(sample) = sink.pull_sample() {
                            if let Some(buffer) = sample.buffer() {
                                // Fan out to all subscribers
                                for entry in subs_clone.iter() {
                                    let appsrc = entry.value();
                                    let _ = appsrc.push_buffer(buffer.copy_deep().unwrap());
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
