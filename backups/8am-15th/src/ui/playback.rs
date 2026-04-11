//! Video playback and multi-track audio mixing via mpv.
//!
//! Luma uses libmpv for playback, which provides hardware-accelerated decoding and the
//! `lavfi-complex` filter graph for real-time audio processing.
//!
//! ## Audio mixing with lavfi-complex
//! Recordings can have multiple audio tracks (system audio, mic, app-specific capture).
//! mpv's `lavfi-complex` property lets us build an FFmpeg filter graph that:
//! 1. Applies per-track volume scaling: `[aid1]volume=volume=0.8[a1]`
//! 2. Mixes enabled tracks together: `[a1][a2]amix=inputs=2:normalize=0[ao]`
//!
//! The `normalize=0` flag is critical — without it, `amix` would auto-normalize volumes
//! based on the number of inputs, making each track quieter as more are enabled.
//!
//! For single-track playback, we skip `amix` entirely and just apply volume directly,
//! avoiding any unnecessary filter overhead.
//!
//! ## HLS playback
//! Session recordings are served via a local HTTP server on `127.0.0.1` (port auto-selected).
//! The URL pattern is `/{game_title}/master.m3u8`, where `game_title` is the sanitized
//! source name. mpv connects to this URL and streams the HLS playlist. Direct file paths
//! (e.g., exported clips) bypass HLS and are loaded directly.

use crate::config::{AppConfig, AudioRouting};
use crate::ui::LumaWorkspace;
use gpui::*;

impl LumaWorkspace {
    /// Rebuilds the mpv `lavfi-complex` filter graph based on current track enable/volume state.
    ///
    /// Called whenever the user toggles a track or adjusts volume. The filter graph is
    /// reconstructed from scratch each time — this is cheap and avoids the complexity of
    /// incremental filter graph updates.
    pub fn update_mpv_audio_mix(&self) {
        if let Some(v) = &self.video_source {
            let active_tracks = self.get_current_audio_tracks();
            let mut enabled_aids = Vec::new();
            for (i, t) in active_tracks.iter().enumerate() {
                if t.enabled {
                    enabled_aids.push(i);
                }
            }

            if enabled_aids.is_empty() {
                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", "");
            } else if enabled_aids.len() == 1 {
                let idx = enabled_aids[0];
                let vol = self.playback_volumes.get(idx).copied().unwrap_or(100.0) / 100.0;
                let complex = format!("[aid{}]volume=volume={}[ao]", idx + 1, vol);

                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
            } else {
                let mut complex = String::new();
                for &idx in &enabled_aids {
                    let vol = self.playback_volumes.get(idx).copied().unwrap_or(100.0) / 100.0;
                    complex.push_str(&format!("[aid{}]volume=volume={}[a{}];", idx + 1, vol, idx + 1));
                }
                for &idx in &enabled_aids {
                    complex.push_str(&format!("[a{}]", idx + 1));
                }
                complex.push_str(&format!("amix=inputs={}:normalize=0[ao]", enabled_aids.len()));

                let _ = v.read().mpv.set_property("aid", "no");
                let _ = v.read().mpv.set_property("lavfi-complex", &*complex);
            }
        }
    }

    pub fn get_current_audio_tracks(&self) -> Vec<AudioRouting> {
        let config = AppConfig::load();
        if let Some(source) = &self.selected_source {
            if source == "monitor" {
                return config.global_audio_tracks.clone();
            }
            if let Some(game) = config.game_registry.get(source) {
                if let Some(audio) = &game.audio_routing {
                    return audio.clone();
                }
            }
        }
        config.global_audio_tracks.clone()
    }

    /// Loads a video source into the main player.
    ///
    /// Handles two cases:
    /// - **Direct files** (`.mp4`/`.mkv`): Loaded straight into mpv by file path.
    /// - **Session recordings**: Generates the HLS playlist, then loads via the local
    ///   HTTP server URL. If the same source is already loaded (e.g., during live recording),
    ///   we just regenerate the playlist and reload to pick up new segments.
    ///
    /// The D3D11 device handle is passed to mpv so it can use hardware-accelerated decoding
    /// on the same GPU device as the rest of the app.
    pub fn load_video(&mut self, source_name: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let path = std::path::Path::new(source_name);
        let is_direct_file = path.exists() && path.extension().map_or(false, |ext| ext == "mkv" || ext == "mp4");

        let already_loaded = if let Some(v) = &self.video_source {
            v.read().source_name == source_name
        } else { false };

        if already_loaded && !is_direct_file {
            let source_name_str = source_name.to_string();
            let recording_id = self.recording_session_id;
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                let name_for_bg = source_name_str.clone();
                async move {
                    if let Some((_, blocks)) = cx.background_executor().spawn(async move {
                        crate::utils::generate_session_playlist(&name_for_bg, recording_id)
                    }).await {
                        let _ = this.update(&mut cx, |this, cx| {
                            *this.app_state.current_session_blocks.lock() = blocks;
                            if let Some(v) = &this.video_source {
                                let safe_title = if source_name_str == "monitor" { "monitor".to_string() } else { crate::utils::clean_title(&source_name_str) };
                                let url = format!("{}/{}/master.m3u8", crate::server::base_url(), safe_title);
                                let _ = v.load_file(&url);
                            }
                            cx.notify();
                        });
                    }
                }
            }).detach();
            return;
        }

        if self.is_loading_video { return; }
        self.is_loading_video = true;
        let source_name_str = source_name.to_string();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let recording_id = this.read_with(&cx, |this, _| this.recording_session_id).ok().flatten();
                let (video_url, blocks) = if is_direct_file {
                    (Some(source_name_str.clone()), Vec::new())
                } else {
                    let name_clone = source_name_str.clone();
                    if let Some((_, b)) = cx.background_executor().spawn(async move {
                        crate::utils::generate_session_playlist(&name_clone, recording_id)
                    }).await {
                        let safe_title = if source_name_str == "monitor" { "monitor".to_string() } else { crate::utils::clean_title(&source_name_str) };
                        let url = format!("{}/{}/master.m3u8", crate::server::base_url(), safe_title);
                        (Some(url), b)
                    } else {
                        (None, Vec::new())
                    }
                };

                let _ = this.update(&mut cx, |this, cx| {
                    this.is_loading_video = false;
                    *this.app_state.current_session_blocks.lock() = blocks;

                    if let Some(url) = video_url {
                        let d3d_device_handle = match this.app_state.d3d11_device.lock().as_ref() {
                            Some(h) => h.0,
                            None => {
                                this.video_source = None;
                                cx.notify();
                                return;
                            }
                        };
                        match crate::video_player::Video::new_with_options(
                            &url,
                            crate::video_player::VideoOptions { source_name: Some(source_name_str), ..Default::default() },
                            Some(d3d_device_handle.0),
                        ) {
                            Ok(video) => {
                                this.video_source = Some(video);
                                this.update_mpv_audio_mix();
                            }
                            Err(_) => this.video_source = None,
                        }
                    } else {
                        this.video_source = None;
                    }
                    cx.notify();
                });
            }
        }).detach();
        cx.notify();
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
                self.clip_end = -1.0;
            } else {
                self.clip_start = v.position().as_secs_f64();
                self.clip_end = -1.0;
            }
            cx.notify();
        }
    }

    pub fn set_clip_out(&mut self, cx: &mut Context<Self>) {
        if let Some(v) = &self.video_source {
            if self.clip_end >= 0.0 {
                self.clip_start = -1.0;
                self.clip_end = -1.0;
            } else {
                self.clip_end = v.position().as_secs_f64();
            }
            cx.notify();
        }
    }

    pub fn open_volume_popover(&mut self, track_idx: usize, cx: &mut Context<Self>) {
        if self.audio_track_volume_popover == Some(track_idx) {
            self.audio_track_volume_popover = None;
            cx.notify();
            return;
        }

        self.audio_track_volume_popover = Some(track_idx);
        self.last_audio_track_volume_popover = Some(track_idx);

        let current_playback_volume = self.playback_volumes.get(track_idx).copied().unwrap_or(100.0);
        self.volume_slider_last_value = current_playback_volume as f32;

        self.volume_slider_state.update(cx, |state, cx| {
            state.set_step(0.1, cx);
            state.set_value((current_playback_volume / 1.5) as f32, cx);
        });

        cx.notify();
    }
}
