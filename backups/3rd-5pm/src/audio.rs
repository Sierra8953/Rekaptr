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

            let subs_clone = subscribers.clone();
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        if let Ok(sample) = sink.pull_sample() {
                            if let Some(in_buffer) = sample.buffer() {
                                // Shallow copy and strip timestamps to prevent A/V desync
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
            let bus = pipeline.bus().unwrap();
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
