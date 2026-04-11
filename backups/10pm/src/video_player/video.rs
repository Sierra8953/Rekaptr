use crate::video_player::Error;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_app::prelude::*;
use gstreamer_video as gst_video;
use gst::message::MessageView;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Position in the media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Position based on time.
    Time(Duration),
    /// Position based on nth frame.
    Frame(u64),
}

impl From<Position> for gst::GenericFormattedValue {
    fn from(pos: Position) -> Self {
        match pos {
            Position::Time(t) => gst::ClockTime::from_nseconds(t.as_nanos() as _).into(),
            Position::Frame(f) => gst::format::Default::from_u64(f).into(),
        }
    }
}

impl From<Duration> for Position {
    fn from(t: Duration) -> Self {
        Position::Time(t)
    }
}

impl From<u64> for Position {
    fn from(f: u64) -> Self {
        Position::Frame(f)
    }
}

#[derive(Debug)]
pub(crate) struct Frame(pub(crate) gst::Sample);

impl Frame {
    pub fn empty() -> Self {
        Self(gst::Sample::builder().build())
    }

    pub fn readable(&'_ self) -> Option<gst::BufferMap<'_, gst::buffer::Readable>> {
        self.0.buffer().and_then(|x| x.map_readable().ok())
    }
}

/// Options for initializing a `Video` without post-construction locking.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Optional initial frame buffer capacity (0 disables buffering). Defaults to 3.
    pub frame_buffer_capacity: Option<usize>,
    /// Optional initial looping flag. Defaults to false.
    pub looping: Option<bool>,
    /// Optional initial playback speed. Defaults to 1.0.
    pub speed: Option<f64>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            frame_buffer_capacity: Some(3),
            looping: Some(false),
            speed: Some(1.0),
        }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct Internal {
    pub(crate) id: u64,
    pub(crate) bus: gst::Bus,
    pub(crate) source: gst::Pipeline,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    pub(crate) duration: Duration,
    pub(crate) speed: Arc<AtomicU64>,

    pub(crate) frame: Arc<Mutex<Frame>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) frame_buffer: Arc<Mutex<VecDeque<Frame>>>,
    pub(crate) frame_buffer_capacity: Arc<AtomicUsize>,
    pub(crate) last_frame_time: Arc<Mutex<Instant>>,
    pub(crate) looping: Arc<AtomicBool>,
    pub(crate) is_eos: Arc<AtomicBool>,
    pub(crate) restart_stream: bool,

    pub(crate) subtitle_text: Arc<Mutex<Option<String>>>,
    pub(crate) upload_text: Arc<AtomicBool>,

    // Optional display size overrides. If only one is set, the other is
    // inferred using the natural aspect ratio (width / height).
    pub(crate) display_width_override: Option<u32>,
    pub(crate) display_height_override: Option<u32>,
}

impl Internal {
    pub(crate) fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {       
        let position = position.into();
        let current_speed = f64::from_bits(self.speed.load(Ordering::SeqCst));

        // Clear EOS so the worker resumes pulling after a seek.
        self.is_eos.store(false, Ordering::SeqCst);

        // Build seek flags. When not accurate, snap in the playback direction to
        // avoid jumping backward to a previous keyframe.
        let mut flags = gst::SeekFlags::FLUSH;
        if accurate {
            flags |= gst::SeekFlags::ACCURATE;
        } else {
            flags |= gst::SeekFlags::KEY_UNIT;
            if current_speed >= 0.0 {
                flags |= gst::SeekFlags::SNAP_AFTER;
            } else {
                flags |= gst::SeekFlags::SNAP_BEFORE;
            }
        }

        match &position {
            Position::Time(_) => self.source.seek(
                current_speed,
                flags,
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::None,
                gst::ClockTime::NONE,
            )?,
            Position::Frame(_) => self.source.seek(
                current_speed,
                flags,
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::None,
                gst::format::Default::NONE,
            )?,
        };

        *self.subtitle_text.lock() = None;
        self.upload_text.store(true, Ordering::SeqCst);

        // Clear any buffered frames so old frames do not display after a seek,
        // which can visually appear as a larger-than-intended jump.
        self.frame_buffer.lock().clear();
        self.upload_frame.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub(crate) fn set_speed(&mut self, speed: f64) -> Result<(), Error> {
        let Some(position) = self.source.query_position::<gst::ClockTime>() else {
            return Err(Error::Caps);
        };
        if speed > 0.0 {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                position,
                gst::SeekType::End,
                gst::ClockTime::from_seconds(0),
            )?;
        } else {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                gst::ClockTime::from_seconds(0),
                gst::SeekType::Set,
                position,
            )?;
        }
        self.speed.store(speed.to_bits(), Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn restart_stream(&mut self) -> Result<(), Error> {
        self.is_eos.store(false, Ordering::SeqCst);
        self.set_paused(false);
        self.seek(0, false)?;
        Ok(())
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        let _ = self.source
            .set_state(if paused {
                gst::State::Paused
            } else {
                gst::State::Playing
            });

        if self.is_eos.load(Ordering::Acquire) && !paused {
            self.restart_stream = true;
        }
    }

    pub(crate) fn paused(&self) -> bool {
        self.source.current_state() == gst::State::Paused
    }
}

/// A multimedia video loaded from a URI (e.g., a local file path or HTTP stream).
#[derive(Debug, Clone)]
pub struct Video(pub(crate) Arc<RwLock<Internal>>);

impl Drop for Video {
    fn drop(&mut self) {
        // Only cleanup if this is the last reference
        if Arc::strong_count(&self.0) == 1 {
            if let Some(mut inner) = self.0.try_write() {
                let _ = inner
                    .source
                    .set_state(gst::State::Null);

                inner.alive.store(false, Ordering::SeqCst);
                if let Some(worker) = inner.worker.take() {
                    let _ = worker.join();
                }
            }
        }
    }
}

impl Video {
    /// Create a new video player from a given video which loads from `uri`.
    pub fn new(uri: &url::Url) -> Result<Self, Error> {
        Self::new_with_options(uri, VideoOptions::default())
    }

    /// Create a new video player from a given video which loads from `uri`,
    /// applying initialization options.
    pub fn new_with_options(uri: &url::Url, options: VideoOptions) -> Result<Self, Error> {
        gst::init()?;

        let pipeline_str = format!(
            "playbin uri=\"{}\" video-sink=\"videoscale ! videoconvert ! appsink name=gpui_video drop=true max-buffers=200 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
            uri.as_str()
        );
        let pipeline = gst::parse::launch(&pipeline_str)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Error::Cast)?;

        let video_sink: gst::Element = pipeline.property("video-sink");
        let pad = video_sink.pads().first().cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let video_sink = bin.by_name("gpui_video").unwrap();
        let video_sink = video_sink.downcast::<gst_app::AppSink>().unwrap();

        Self::from_gst_pipeline_with_options(pipeline, video_sink, None, options)
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink.
    pub fn from_gst_pipeline(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        text_sink: Option<gst_app::AppSink>,
    ) -> Result<Self, Error> {
        Self::from_gst_pipeline_with_options(
            pipeline,
            video_sink,
            text_sink,
            VideoOptions::default(),
        )
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink,
    /// applying initialization options.
    pub fn from_gst_pipeline_with_options(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        text_sink: Option<gst_app::AppSink>,
        options: VideoOptions,
    ) -> Result<Self, Error> {
        gst::init()?;
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        macro_rules! cleanup {
            ($expr:expr) => {
                $expr.map_err(|e| {
                    let _ = pipeline.set_state(gst::State::Null);
                    e
                })
            };
        }

        let pad = video_sink.pads().first().cloned().unwrap();

        cleanup!(pipeline.set_state(gst::State::Playing))?;

        // Wait a brief moment for the pipeline to start playing
        let _ = pipeline.state(gst::ClockTime::from_mseconds(100));
        cleanup!(pipeline.state(gst::ClockTime::from_seconds(5)).0)?;

        let caps = cleanup!(pad.current_caps().ok_or(Error::Caps))?;
        let s = cleanup!(caps.structure(0).ok_or(Error::Caps))?;
        let width = cleanup!(s.get::<i32>("width").map_err(|_| Error::Caps))?;
        let height = cleanup!(s.get::<i32>("height").map_err(|_| Error::Caps))?;
        let framerate = cleanup!(s.get::<gst::Fraction>("framerate").map_err(|_| Error::Caps))?;
        let framerate = framerate.numer() as f64 / framerate.denom() as f64;

        if framerate.is_nan()
            || framerate.is_infinite()
            || framerate < 0.0
            || framerate.abs() < f64::EPSILON
        {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(Error::Framerate(framerate));
        }

        let duration = Duration::from_nanos(
            pipeline
                .query_duration::<gst::ClockTime>()
                .map(|duration| duration.nseconds())
                .unwrap_or(0),
        );

        let frame = Arc::new(Mutex::new(Frame::empty()));
        let upload_frame = Arc::new(AtomicBool::new(false));
        let frame_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let frame_buffer_capacity = Arc::new(AtomicUsize::new(
            options.frame_buffer_capacity.unwrap_or_default(),
        ));
        let alive = Arc::new(AtomicBool::new(true));
        let last_frame_time = Arc::new(Mutex::new(Instant::now()));
        let initial_looping = options.looping.unwrap_or_default();
        let looping_flag = Arc::new(AtomicBool::new(initial_looping));
        let looping_ref = Arc::clone(&looping_flag);
        let initial_speed = options.speed.unwrap_or_default();
        let speed_state = Arc::new(AtomicU64::new(initial_speed.to_bits()));
        let speed_ref = Arc::clone(&speed_state);

        let frame_ref = Arc::clone(&frame);
        let upload_frame_ref = Arc::clone(&upload_frame);
        let frame_buffer_ref = Arc::clone(&frame_buffer);
        let frame_buffer_capacity_ref = Arc::clone(&frame_buffer_capacity);
        let alive_ref = Arc::clone(&alive);
        let last_frame_time_ref = Arc::clone(&last_frame_time);

        let subtitle_text = Arc::new(Mutex::new(None));
        let upload_text = Arc::new(AtomicBool::new(false));
        let subtitle_text_ref = Arc::clone(&subtitle_text);
        let upload_text_ref = Arc::clone(&upload_text);

        let pipeline_ref = pipeline.clone();
        let bus_ref = pipeline_ref.bus().unwrap();
        let is_eos = Arc::new(AtomicBool::new(false));
        let is_eos_ref = Arc::clone(&is_eos);

        let worker = std::thread::spawn(move || {
            let mut clear_subtitles_at: Option<gst::ClockTime> = None;

            while alive_ref.load(Ordering::Acquire) {
                // Drain bus messages to detect EOS/errors
                while let Some(msg) = bus_ref.timed_pop(gst::ClockTime::from_seconds(0)) {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            if looping_ref.load(Ordering::SeqCst) {
                                let mut flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
                                let current_speed =
                                    f64::from_bits(speed_ref.load(Ordering::SeqCst));
                                if current_speed >= 0.0 {
                                    flags |= gst::SeekFlags::SNAP_AFTER;
                                } else {
                                    flags |= gst::SeekFlags::SNAP_BEFORE;
                                }
                                match pipeline_ref.seek(
                                    current_speed,
                                    flags,
                                    gst::SeekType::Set,
                                    gst::GenericFormattedValue::from(gst::ClockTime::from_seconds(        
                                        0,
                                    )),
                                    gst::SeekType::None,
                                    gst::ClockTime::NONE,
                                ) {
                                    Ok(_) => {
                                        is_eos_ref.store(false, Ordering::SeqCst);
                                        let _ = pipeline_ref.set_state(gst::State::Playing);
                                        frame_buffer_ref.lock().clear();
                                        upload_frame_ref.store(false, Ordering::SeqCst);
                                        *subtitle_text_ref.lock() = None;
                                        upload_text_ref.store(true, Ordering::SeqCst);
                                        *last_frame_time_ref.lock() = Instant::now();
                                        continue;
                                    }
                                    Err(_) => {
                                        is_eos_ref.store(true, Ordering::SeqCst);
                                    }
                                }
                            } else {
                                is_eos_ref.store(true, Ordering::SeqCst);
                            }
                        }
                        _ => {}
                    }
                }

                if is_eos_ref.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                if let Err(_) = (|| -> Result<(), gst::FlowError> {
                    let maybe_sample =
                        if pipeline_ref.current_state() != gst::State::Playing {
                            video_sink.try_pull_preroll(gst::ClockTime::from_mseconds(16))
                        } else {
                            video_sink.try_pull_sample(gst::ClockTime::from_mseconds(16))
                        };

                    let Some(sample) = maybe_sample else {
                        return Ok(());
                    };

                    *last_frame_time_ref.lock() = Instant::now();

                    let frame_pts = sample.buffer().ok_or(gst::FlowError::Error)?.pts().ok_or(gst::FlowError::Error)?;

                    {
                        let mut frame_guard = frame_ref.lock();
                        *frame_guard = Frame(sample);
                    }

                    let capacity = frame_buffer_capacity_ref.load(Ordering::SeqCst);
                    if capacity > 0 {
                        let sample_for_buffer = frame_ref.lock().0.clone();
                        let mut buf = frame_buffer_ref.lock();
                        buf.push_back(Frame(sample_for_buffer));
                        while buf.len() > capacity {
                            buf.pop_front();
                        }
                    }

                    upload_frame_ref.store(true, Ordering::SeqCst);

                    if let Some(at) = clear_subtitles_at {
                        if frame_pts >= at {
                            *subtitle_text_ref.lock() = None;
                            upload_text_ref.store(true, Ordering::SeqCst);
                            clear_subtitles_at = None;
                        }
                    }

                    Ok(())
                })() {}
            }
        });

        // Apply initial playback speed if specified (must be after pipeline started)
        if (initial_speed - 1.0).abs() > f64::EPSILON {
            if let Some(position) = pipeline.query_position::<gst::ClockTime>() {
                if initial_speed > 0.0 {
                    let _ = pipeline.seek(
                        initial_speed,
                        gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                        gst::SeekType::Set,
                        position,
                        gst::SeekType::End,
                        gst::ClockTime::from_seconds(0),
                    );
                } else {
                    let _ = pipeline.seek(
                        initial_speed,
                        gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                        gst::SeekType::Set,
                        gst::ClockTime::from_seconds(0),
                        gst::SeekType::Set,
                        position,
                    );
                }
            }
        }

        Ok(Video(Arc::new(RwLock::new(Internal {
            id,
            bus: pipeline.bus().unwrap(),
            source: pipeline,
            alive,
            worker: Some(worker),

            width,
            height,
            framerate,
            duration,
            speed: speed_state,

            frame,
            upload_frame,
            frame_buffer,
            frame_buffer_capacity,
            last_frame_time,
            looping: looping_flag,
            is_eos,
            restart_stream: false,

            subtitle_text,
            upload_text,

            display_width_override: None,
            display_height_override: None,
        }))))
    }

    pub(crate) fn read(&'_ self) -> parking_lot::RwLockReadGuard<'_, Internal> {
        self.0.read()
    }

    pub(crate) fn write(&'_ self) -> parking_lot::RwLockWriteGuard<'_, Internal> {
        self.0.write()
    }

    pub fn size(&self) -> (i32, i32) {
        (self.read().width, self.read().height)
    }

    pub fn aspect_ratio(&self) -> f32 {
        let (w, h) = self.size();
        if w <= 0 || h <= 0 {
            return 1.0;
        }
        w as f32 / h as f32
    }

    pub fn set_display_size(&self, width: Option<u32>, height: Option<u32>) {
        let mut inner = self.write();
        inner.display_width_override = width;
        inner.display_height_override = height;
    }

    pub fn display_size(&self) -> (u32, u32) {
        let inner = self.read();
        let natural_w = inner.width.max(0) as u32;
        let natural_h = inner.height.max(0) as u32;
        let ar = if natural_h == 0 {
            1.0
        } else {
            natural_w as f32 / natural_h as f32
        };

        match (inner.display_width_override, inner.display_height_override) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let h = if ar == 0.0 {
                    natural_h
                } else {
                    (w as f32 / ar).round() as u32
                };
                (w, h)
            }
            (None, Some(h)) => {
                let w = ((h as f32) * ar).round() as u32;
                (w, h)
            }
            (None, None) => (natural_w, natural_h),
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.write().set_paused(paused)
    }

    pub fn paused(&self) -> bool {
        self.read().paused()
    }

    pub fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        self.read().seek(position, accurate)
    }

    pub fn position(&self) -> Duration {
        Duration::from_nanos(
            self.read()
                .source
                .query_position::<gst::ClockTime>()
                .map_or(0, |pos| pos.nseconds()),
        )
    }

    pub fn duration(&self) -> Duration {
        self.read().duration
    }

    pub fn buffered_len(&self) -> usize {
        self.read().frame_buffer.lock().len()
    }

    pub fn pop_buffered_frame(&self) -> Option<(Vec<u8>, u32, u32)> {
        let (width, height) = self.size();
        let inner = self.read();
        let maybe_frame = inner.frame_buffer.lock().pop_front();
        if let Some(frame) = maybe_frame {
            if let Some(readable) = frame.readable() {
                let data = readable.as_slice().to_vec();
                if !data.is_empty() {
                    return Some((data, width as u32, height as u32));
                }
            }
        }
        None
    }

    pub fn take_frame_ready(&self) -> bool {
        self.read().upload_frame.swap(false, Ordering::SeqCst)
    }

    pub fn set_frame_buffer_capacity(&self, capacity: usize) {
        let inner = self.read();
        inner
            .frame_buffer_capacity
            .store(capacity, Ordering::SeqCst);
        if capacity == 0 {
            inner.frame_buffer.lock().clear();
        } else {
            let mut buf = inner.frame_buffer.lock();
            while buf.len() > capacity {
                buf.pop_front();
            }
        }
    }

    pub fn current_frame_data(&self) -> Option<(Vec<u8>, u32, u32)> {
        let inner = self.read();
        if let Some(readable) = inner.frame.lock().readable() {
            let data = readable.as_slice().to_vec();
            if !data.is_empty() {
                return Some((data, inner.width as u32, inner.height as u32));
            }
        }
        None
    }
}
