use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use adabraka_ui::components::input::Input;
use crate::config::AudioRouting;
use crate::ui::RekaptrWorkspace;

/// Playback audio-mixer state, grouped out of the `RekaptrWorkspace` god-object.
/// Per-enabled-track volume/mute/solo plus the master bar, all indexed in the
/// same enabled-track order.
pub struct MixerState {
    /// Per-track playback volume (0..150, 100 == unity).
    pub volumes: Vec<f64>,
    /// Last `lavfi-complex` string applied to the playback mpv, so we don't
    /// rebuild the audio filter graph every poll tick. Reset to `None` whenever
    /// the video source changes.
    pub last_mix_sig: Option<String>,
    pub sliders: Vec<gpui::Entity<crate::ui::volume_slider::VolumeSlider>>,
    /// Per-enabled-track mute / solo state, indexed like `sliders`.
    pub muted: Vec<bool>,
    pub solo: Vec<bool>,
    /// Master volume bar (drives mpv's overall `volume` property).
    pub master_slider: Option<gpui::Entity<crate::ui::volume_slider::VolumeSlider>>,
}

/// Dashboard Sources-list state, grouped out of the `RekaptrWorkspace`
/// god-object: the filter box plus the custom scrollbar's persistent
/// scroll/drag/geometry state.
pub struct SourcesState {
    /// Filter query for the Sources table.
    pub search_input: Entity<adabraka_ui::components::input_state::InputState>,
    /// Persistent scroll position + scrollbar state (must outlive a frame).
    pub scroll_handle: ScrollHandle,
    /// The thumb pops up while the box is hovered or being dragged.
    pub box_hovered: bool,
    pub scrollbar_dragging: bool,
    /// Scroll area's window-space rect, captured each frame so a drag can map
    /// cursor-Y to a scroll offset.
    pub track_bounds: Bounds<Pixels>,
}

impl SourcesState {
    pub fn new(cx: &mut Context<RekaptrWorkspace>) -> Self {
        Self {
            search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            scroll_handle: ScrollHandle::new(),
            box_hovered: false,
            scrollbar_dragging: false,
            track_bounds: Bounds::default(),
        }
    }
}

impl MixerState {
    pub fn new() -> Self {
        Self {
            volumes: vec![100.0; 10],
            last_mix_sig: None,
            sliders: Vec::new(),
            muted: Vec::new(),
            solo: Vec::new(),
            master_slider: None,
        }
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

impl RekaptrWorkspace {
    pub fn render_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        // Audio tracks drive the side mixer; make sure a volume slider exists per
        // enabled track (these are the same sliders that mix playback audio).
        let enabled_tracks: Vec<AudioRouting> = self
            .get_current_audio_tracks()
            .into_iter()
            .filter(|t| t.enabled)
            .collect();
        self.ensure_track_vol_sliders(enabled_tracks.len(), cx);

        let is_recording = self.app_state.recording.phase.lock().is_recording();
        let rec_elapsed = self.recording_start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);

        div()
            .id("dashboard-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.tokens.background)
            // No top bar — the preview + mixer sit flush at the very top. The row
            // flexes to whatever space is left after the transport/timeline box
            // and the sources list.
            .child(
                div()
                    .id("dashboard-content")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .px_8()
                    .pt_3()
                    .pb_8()
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .min_h(px(240.0))
                            .flex()
                            .gap_5()
                            .child(self.render_preview_pane(is_recording, rec_elapsed, cx))
                            .child(self.render_mixer(&enabled_tracks, cx)),
                    )
                    .child(self.render_transport_strip(cx))
                    // Everything under the timeline box is nudged down 5px.
                    .child(div().mt(px(5.0)).child(self.render_sources_list(window, cx))),
            )
    }
}

mod mixer;
mod preview;
mod sources;
