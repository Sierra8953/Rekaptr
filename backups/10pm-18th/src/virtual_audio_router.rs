use anyhow::{anyhow, Context, Result};
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use ringbuf::traits::{Consumer, Observer, Split, Producer};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use wasapi::{Direction, WaveFormat, SampleType, StreamMode};

// Wrapper structs to safely move WASAPI COM objects between threads
struct SendClient { inner: wasapi::AudioClient }
unsafe impl Send for SendClient {}
impl std::ops::Deref for SendClient {
    type Target = wasapi::AudioClient;
    fn deref(&self) -> &Self::Target { &self.inner }
}

struct SendCaptureClient { inner: wasapi::AudioCaptureClient }
unsafe impl Send for SendCaptureClient {}
impl std::ops::Deref for SendCaptureClient {
    type Target = wasapi::AudioCaptureClient;
    fn deref(&self) -> &Self::Target { &self.inner }
}

pub struct VirtualAudioRouter {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl VirtualAudioRouter {
    pub fn new(target_pid: u32, app_src: AppSrc) -> Result<Self> {
        let _ = wasapi::initialize_mta();

        // 1. Initialize modern Windows 11 Application Loopback
        let mut client = wasapi::AudioClient::new_application_loopback_client(target_pid, true)
            .map_err(|e| anyhow!("Failed to create loopback client: {}", e))?;

        let mix_format = client.get_mixformat()
            .map_err(|e| anyhow!("Failed to get mix format: {}", e))?;

        // Use Polling Shared mode for lowest latency and stability
        client.initialize_client(
            &mix_format,
            &Direction::Capture,
            &StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 10000000,
            },
        ).map_err(|e| anyhow!("Failed to init WASAPI client: {}", e))?;

        let capture_client = client.get_audiocaptureclient()
            .map_err(|e| anyhow!("Failed to get capture client: {}", e))?;
        
        client.start_stream()
            .map_err(|e| anyhow!("Failed to start WASAPI stream: {}", e))?;

        // 2. Configure GStreamer AppSrc to match WASAPI's internal format
        let sample_rate = mix_format.get_samplespersec();
        let channels = mix_format.get_nchannels() as u32;
        let bits_per_sample = mix_format.get_bitspersample() as u32;
        let sample_type = mix_format.get_subformat().unwrap_or(SampleType::Int);

        let format_str = match (sample_type, bits_per_sample) {
            (SampleType::Float, 32) => "F32LE",
            (SampleType::Int, 16) => "S16LE",
            (SampleType::Int, 24) => "S24LE",
            (SampleType::Int, 32) => "S32LE",
            _ => "S16LE",
        };

        let caps = gstreamer::Caps::builder("audio/x-raw")
            .field("format", format_str)
            .field("rate", sample_rate as i32)
            .field("channels", channels as i32)
            .field("layout", "interleaved")
            .build();

        app_src.set_caps(Some(&caps));

        let bytes_per_frame = (bits_per_sample / 8) * channels;
        let silence_frames = (sample_rate / 100) as usize; // 10ms of silence

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // 3. Ring buffer for thread decoupling
        let rb = HeapRb::<u8>::new(1024 * 1024 * 4); // 4MB
        let (mut producer, mut consumer) = rb.split();

        let send_client = SendClient { inner: client };
        let send_capture_client = SendCaptureClient { inner: capture_client };

        let handle = thread::Builder::new()
            .name(format!("AudioCapture_{}", target_pid))
            .spawn(move || {
                let silence_buffer = vec![0u8; silence_frames * bytes_per_frame as usize];
                
                while running_clone.load(Ordering::Relaxed) {
                    let frames_avail = match send_capture_client.get_next_packet_size() {
                        Ok(Some(f)) => f,
                        _ => 0,
                    };
                    
                    if frames_avail == 0 {
                        let _ = producer.push_slice(&silence_buffer);
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }

                    let mut chunk = vec![0u8; frames_avail as usize * bytes_per_frame as usize];
                    if send_capture_client.read_from_device(&mut chunk).is_ok() {
                        let _ = producer.push_slice(&chunk);
                    } else {
                        let _ = producer.push_slice(&silence_buffer);
                    }
                    
                    thread::sleep(Duration::from_millis(1));
                }
                let _ = send_client.stop_stream();
            })
            .context("Failed to spawn capture thread")?;

        // 4. AppSrc Push Loop
        let running_push = running.clone();
        thread::Builder::new()
            .name(format!("AudioPush_{}", target_pid))
            .spawn(move || {
                let mut pts = 0;
                while running_push.load(Ordering::Relaxed) {
                    let avail = consumer.occupied_len();
                    if avail > 0 {
                        let mut chunk = vec![0u8; avail];
                        consumer.pop_slice(&mut chunk);
                        
                        let mut buffer = gstreamer::Buffer::from_mut_slice(chunk);
                        let duration = gstreamer::ClockTime::from_nseconds(
                            (avail as u64 * 1_000_000_000) / (sample_rate as u64 * bytes_per_frame as u64),
                        );
                        
                        if let Some(buffer_ref) = buffer.get_mut() {
                            buffer_ref.set_pts(gstreamer::ClockTime::from_nseconds(pts));
                            buffer_ref.set_duration(duration);
                        }
                        pts += duration.nseconds();

                        if app_src.push_buffer(buffer).is_err() { break; }
                    } else {
                        thread::sleep(Duration::from_millis(2));
                    }
                }
                let _ = app_src.end_of_stream();
            })
            .context("Failed to spawn push thread")?;

        Ok(Self {
            running,
            handle: Some(handle),
        })
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for VirtualAudioRouter {
    fn drop(&mut self) {
        self.stop();
    }
}
