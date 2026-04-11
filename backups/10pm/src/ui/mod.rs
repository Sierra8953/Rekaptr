use gpui::*;
use crate::video_player::{Video};
use adabraka_ui::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use gstreamer as gst;
use gst::prelude::*;

use crate::state::{AppState};
use crate::config::AppConfig;

mod sidebar;
mod dashboard;
mod settings;
mod clips;
mod add_source;
mod timeline;

pub struct LumaWorkspace {
    pub active_view: ActiveView,
    pub app_state: Arc<Mutex<AppState>>,
    pub video_source: Option<Video>,
    pub selected_source: Option<String>,
    pub show_add_source_modal: bool,
    pub advanced_settings_source: Option<String>,
    pub clip_start: f32,
    pub clip_end: f32,
    pub timeline_bounds: Bounds<Pixels>,
    pub is_scrubbing: bool,
    pub drag_target: Option<TimelineDragTarget>,
    pub scrubbing_progress: f32,
    pub last_seek_at: std::time::Instant,
    pub toast_manager: Entity<adabraka_ui::overlays::toast::ToastManager>,
    // Add Source Form State
    pub form_title: String,
    pub form_hwnd: Option<u64>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TimelineDragTarget {
    Playhead,
    InMarker,
    OutMarker,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Dashboard,
    Settings,
    Clips,
}

impl LumaWorkspace {
    pub fn new(app_state: Arc<Mutex<AppState>>, cx: &mut Context<Self>) -> Self {
        let toast_manager = cx.new(|cx| adabraka_ui::overlays::toast::ToastManager::new(cx));
        let mut workspace = Self {
            active_view: ActiveView::Dashboard,
            app_state,
            video_source: None,
            selected_source: None,
            show_add_source_modal: false,
            advanced_settings_source: None,
            clip_start: -1.0,
            clip_end: -1.0,
            timeline_bounds: Bounds::default(),
            is_scrubbing: false,
            drag_target: None,
            scrubbing_progress: 0.0,
            last_seek_at: std::time::Instant::now(),
            toast_manager,
            form_title: "New Source".to_string(),
            form_hwnd: None,
        };

        if let Some(playlist_path) = crate::utils::generate_session_playlist("desktop") {
            workspace.load_video(&playlist_path, cx);
        } else {
            workspace.load_video(std::path::Path::new("E:\\LumaRecordings\\desktop\\view.m3u8"), cx);
        }

        workspace
    }

    pub fn show_toast(&self, title: impl Into<SharedString>, description: Option<impl Into<SharedString>>, variant: adabraka_ui::overlays::toast::ToastVariant, window: &mut Window, cx: &mut Context<Self>) {
        let mut toast = adabraka_ui::overlays::toast::ToastItem::new(
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64,
            title.into()
        ).variant(variant);
        
        if let Some(desc) = description {
            toast = toast.description(desc.into());
        }

        self.toast_manager.update(cx, |manager, cx| {
            manager.add_toast(toast, window, cx);
        });
    }

    pub fn set_active_view(&mut self, view: ActiveView, cx: &mut Context<Self>) {
        self.active_view = view;
        cx.notify();
    }

    pub fn load_video(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        if let Ok(uri) = url::Url::from_file_path(path) {
            match Video::new(&uri) {
                Ok(v) => {
                    v.set_paused(false);
                    self.video_source = Some(v);
                    cx.notify();
                }
                Err(e) => {
                    println!("Failed to create video from URL: {} - Error: {:?}", uri, e);
                }
            }
        }
    }

    pub fn toggle_play_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            v.set_paused(!v.paused());
            cx.notify();
        }
    }

    pub fn set_clip_in(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_start >= 0.0 {
                self.clip_start = -1.0;
            } else {
                let pos = v.position().as_secs_f32();
                let dur = v.duration().as_secs_f32();
                if dur > 0.0 {
                    self.clip_start = pos / dur;
                }
            }
            cx.notify();
        }
    }

    pub fn set_clip_out(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_end >= 0.0 {
                self.clip_end = -1.0;
            } else {
                let pos = v.position().as_secs_f32();
                let dur = v.duration().as_secs_f32();
                if dur > 0.0 {
                    self.clip_end = pos / dur;
                }
            }
            cx.notify();
        }
    }

    pub fn save_clip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        println!("Saving clip from {} to {}", self.clip_start, self.clip_end);
        self.show_toast("Clip Saved", Some("Your recording has been saved to E:\\LumaRecordings"), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
    }

    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let is_recording = self.app_state.lock().is_recording.load(std::sync::atomic::Ordering::SeqCst);
        
        if is_recording {
            if let Some(pipeline) = self.app_state.lock().pipeline.take() {
                std::thread::spawn(move || {
                    pipeline.send_event(gst::event::Eos::new());
                    if let Some(bus) = pipeline.bus() {
                        let _ = bus.timed_pop_filtered(gst::ClockTime::from_seconds(5), &[gst::MessageType::Eos]);
                    }
                    let _ = pipeline.set_state(gst::State::Null);
                });
            }
            self.app_state.lock().is_recording.store(false, std::sync::atomic::Ordering::SeqCst);
            println!("Recording stopped.");
            self.show_toast("Recording Stopped", None::<&str>, adabraka_ui::overlays::toast::ToastVariant::Default, window, cx);
            cx.notify();
        } else {
            let config = AppConfig::load();
            let source_name = self.selected_source.clone().unwrap_or_else(|| "Monitor".to_string());
            
            let hwnd = if source_name == "Monitor" {
                None
            } else {
                let state = self.app_state.lock();
                state.available_windows.iter().find(|w| w.title == source_name).map(|w| w.hwnd)
            };
            
            let target_pid = if let Some(h) = hwnd {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
                    let mut pid = 0;
                    let _ = GetWindowThreadProcessId(windows::Win32::Foundation::HWND(h as *mut core::ffi::c_void), Some(&mut pid));
                    if pid > 0 { Some(pid) } else { None }
                }
            } else { None };

            let safe_title = crate::utils::clean_title(&source_name).replace(' ', "_");
            let s_dir = crate::utils::get_storage_root().join(&safe_title).join(format!(
                "session_{}",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
            ));

            if let Err(_) = std::fs::create_dir_all(&s_dir) {
                println!("Failed to create session directory.");
                self.show_toast("Error", Some("Failed to create session directory"), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                return;
            }

            let mut video_settings = config.global_video.clone();
            let mut audio_routing = config.global_audio_tracks.clone();

            if source_name != "Monitor" {
                if let Some(gs) = config.game_registry.get(&source_name) {
                    if let Some(vs) = &gs.video_overrides {
                        video_settings = vs.clone();
                    }
                    if let Some(ar) = &gs.audio_routing {
                        audio_routing = ar.clone();
                    }
                }
            }

            let pipeline_str = crate::engine::generate_pipeline_string(
                &video_settings,
                &s_dir.to_string_lossy().replace('\\', "/"),
                &audio_routing,
                &config.mic_settings,
                hwnd,
                target_pid,
                config.selected_adapter_index,
            );

            match gst::parse::launch(&pipeline_str) {
                Ok(element) => {
                    let pipeline = element.downcast::<gst::Pipeline>().unwrap();

                    let sink = pipeline.by_name("sink").unwrap();
                    let s_path = s_dir.to_string_lossy().replace('\\', "/");
                    let cache_handle = self.app_state.lock().playlist_cache.clone();
                    let cache_title = source_name.clone();
                    
                    sink.connect("format-location", false, move |values| {
                        let _elem = values[0].get::<gst::Element>().unwrap();
                        let id = values[1].get::<u32>().unwrap();
                        
                        if id == 3 {
                            let cleanup_path = s_path.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                for i in 1..=3 {
                                    let temp_file = format!("{}/temp_{}.mkv", cleanup_path, i);
                                    let _ = std::fs::remove_file(temp_file);
                                }
                            });
                        }

                        if id >= 3 {
                            cache_handle.remove(&cache_title);
                        }

                        let fname = if id < 3 {
                            format!("{}/temp_{}.mkv", s_path, id + 1)
                        } else {
                            format!("{}/segment_{:05}.mkv", s_path, id - 3)
                        };
                        Some(fname.to_value())
                    });

                    // Wire up Audio Sources
                    for (i, track) in audio_routing.iter().enumerate() {
                        if !track.enabled {
                            continue;
                        }
                        if track.source_type == "Mic" {
                            if let Some(appsrc_elem) = pipeline.by_name(&format!("mic_src_{}", i)) {
                                let appsrc = appsrc_elem.downcast::<gstreamer_app::AppSrc>().unwrap();
                                let caps = gst::Caps::builder("audio/x-raw")
                                    .field("format", &"F32LE")
                                    .field("rate", &48000)
                                    .field("channels", &2)
                                    .field("layout", &"interleaved")
                                    .build();
                                appsrc.set_caps(Some(&caps));

                                let provider_arc = {
                                    let state = self.app_state.lock();
                                    let p = state.mic_provider.lock().clone();
                                    p
                                };

                                if let Some(provider) = provider_arc {
                                    let sub_id = (std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_nanos()
                                        % 1000000) as u64;
                                    provider.subscribers.insert(sub_id, appsrc);
                                }
                            }
                        }
                    }

                    let bus = pipeline.bus().unwrap();
                    let state_arc = self.app_state.clone();
                    std::thread::spawn(move || {
                        for msg in bus.iter_timed(gst::ClockTime::NONE) {
                            if let gst::MessageView::Error(err) = msg.view() {
                                println!("GStreamer Error: {}", err.error());
                                state_arc.lock().is_recording.store(false, std::sync::atomic::Ordering::SeqCst);
                                break;
                            }
                        }
                    });

                    if pipeline.set_state(gst::State::Playing).is_ok() {
                        let mut state = self.app_state.lock();
                        state.pipeline = Some(pipeline);
                        state.is_recording.store(true, std::sync::atomic::Ordering::SeqCst);
                        println!("Recording started for: {}", source_name);
                        self.show_toast("Recording Started", Some(&format!("Capturing {}", source_name)), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                        cx.notify();
                    }
                }
                Err(e) => {
                    println!("Failed to launch pipeline: {:?}", e);
                    self.show_toast("Pipeline Error", Some("GStreamer failed to start"), adabraka_ui::overlays::toast::ToastVariant::Error, window, cx);
                }
            }
        }
    }
}

impl Render for LumaWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_view;
        let theme = use_theme();

        let mut root = HStack::new()
            .size_full()
            .bg(theme.tokens.background)
            .text_color(theme.tokens.foreground)
            .child(self.render_sidebar(window, cx))
            .child(
                VStack::new()
                    .flex_1()
                    .child(match active {
                        ActiveView::Dashboard => self.render_dashboard(window, cx).into_any_element(),
                        ActiveView::Settings => self.render_settings_view(window, cx).into_any_element(),
                        ActiveView::Clips => self.render_clips(window, cx).into_any_element(),
                    })
            );

        if self.show_add_source_modal {
            root = root.child(self.render_add_source_modal(window, cx));
        }

        if let Some(source) = self.advanced_settings_source.clone() {
            root = root.child(self.render_advanced_settings_dialog(&source, window, cx));
        }

        root.child(self.toast_manager.clone())
    }
}
