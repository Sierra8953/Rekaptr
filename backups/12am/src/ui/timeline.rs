use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{LumaWorkspace, TimelineDragTarget};
use crate::config::AppConfig;

impl LumaWorkspace {
    pub fn render_timeline(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let audio_tracks = self.get_current_audio_tracks();
        let enabled_audio_tracks: Vec<_> = audio_tracks.into_iter().filter(|t| t.enabled).collect();
        
        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64())
        } else {
            (0.0, 1.0)
        };
        
        let progress = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::Playhead) {
            self.scrubbing_progress
        } else if duration > 0.0 {
            (position / duration) as f32
        } else {
            0.0
        };
        
        let clip_start_prog = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::InMarker) {
            self.scrubbing_progress
        } else if duration > 0.0 && self.clip_start >= 0.0 {
            (self.clip_start / duration) as f32
        } else {
            -1.0
        };

        let clip_end_prog = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::OutMarker) {
            self.scrubbing_progress
        } else if duration > 0.0 && self.clip_end >= 0.0 {
            (self.clip_end / duration) as f32
        } else {
            -1.0
        };

        let view = cx.entity().downgrade();

        VStack::new()
            .w_full()
            .gap_3()
            .child(
                div()
                    .id("timeline-container")
                    .relative()
                    .w_full()
                    .child(
                        VStack::new()
                            .gap_1()
                            // 1. Video Lane
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .child(self.render_track_header("Video", None, None, theme.tokens.primary, cx))
                                    .child(self.render_track_lane(progress, true, theme.tokens.primary))
                            )
                            // 2. Audio Lanes
                            .children(enabled_audio_tracks.iter().enumerate().map(|(i, track)| {
                                HStack::new()
                                    .gap_2()
                                    .child(self.render_track_header(&track.name, Some(i), Some(track.volume), theme.tokens.primary, cx))
                                    .child(self.render_track_lane(progress, false, theme.tokens.primary))
                            }))
                    )
                    // Overlays (Spanning all tracks)
                    .child(self.render_ticks_overlay(theme.tokens.border))
                    .child(self.render_playhead_overlay(progress, theme.tokens.foreground))
                    .child(self.render_markers_overlay(clip_start_prog, clip_end_prog, theme.tokens.primary))
                    // Global Interaction Area
                    .child(
                        canvas(
                            move |bounds, _window, cx| {
                                let _ = view.update(cx, |this, _cx| {
                                    this.timeline_bounds = bounds;
                                });
                            },
                            move |_, _, _, _| {}
                        )
                        .absolute()
                        .size_full()
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        let width = this.timeline_bounds.size.width;
                        let relative_x = (event.position.x - this.timeline_bounds.left()).clamp(px(0.0), width);
                        let percentage = f32::from(relative_x) / f32::from(width);

                        let current_in_prog = if duration > 0.0 && this.clip_start >= 0.0 { (this.clip_start / duration) as f32 } else { -1.0 };
                        let current_out_prog = if duration > 0.0 && this.clip_end >= 0.0 { (this.clip_end / duration) as f32 } else { -1.0 };

                        if (percentage - current_in_prog).abs() < 0.02 {
                            this.drag_target = Some(TimelineDragTarget::InMarker);
                        } else if (percentage - current_out_prog).abs() < 0.02 {
                            this.drag_target = Some(TimelineDragTarget::OutMarker);
                        } else {
                            this.drag_target = Some(TimelineDragTarget::Playhead);
                            this.seek_to_mouse_x(event.position.x, true);
                        }
                        
                        this.is_scrubbing = true;
                        cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(move |this, _event: &MouseUpEvent, _window, cx| {
                        this.is_scrubbing = false;
                        this.drag_target = None;
                        cx.notify();
                    }))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                        if this.is_scrubbing {
                            match this.drag_target {
                                Some(TimelineDragTarget::Playhead) => {
                                    let now = std::time::Instant::now();
                                    let throttle = now.duration_since(this.last_seek_at).as_millis() > 33;
                                    this.seek_to_mouse_x(event.position.x, !throttle);
                                    if throttle { this.last_seek_at = now; }
                                }
                                Some(TimelineDragTarget::InMarker) => {
                                    this.update_progress_from_mouse(event.position.x);
                                    this.clip_start = this.scrubbing_progress as f64 * duration;
                                }
                                Some(TimelineDragTarget::OutMarker) => {
                                    this.update_progress_from_mouse(event.position.x);
                                    this.clip_end = this.scrubbing_progress as f64 * duration;
                                }
                                None => {}
                            }
                            cx.notify();
                        }
                    }))
            )
            .when_some(self.audio_track_volume_popover, |this, track_idx| {
                let theme = use_theme();
                let audio_tracks = self.get_current_audio_tracks();
                if let Some(track) = audio_tracks.get(track_idx) {
                    let view = cx.entity().downgrade();
                    let source_name = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                    
                    this.child(
                        div()
                            .absolute()
                            .inset_0()
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.audio_track_volume_popover = None;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(170.0))
                                    .bottom(px(10.0))
                                    .w(px(200.0))
                                    .p_4()
                                    .bg(theme.tokens.card)
                                    .border_1()
                                    .border_color(theme.tokens.border)
                                    .rounded_md()
                                    .shadow_lg()
                                    .child(
                                        VStack::new()
                                            .gap_2()
                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(format!("{} Volume", track.name)))
                                            .child(
                                                HStack::new()
                                                    .gap_3()
                                                    .items_center()
                                                    .child({
                                                        let slider_state = cx.new(adabraka_ui::components::slider::SliderState::new);
                                                        slider_state.update(cx, |state, cx| state.set_value(track.volume, cx));
                                                        adabraka_ui::components::slider::Slider::new(slider_state)
                                                            .on_change(move |val, _window, cx| {
                                                                let _ = view.update(cx, |_this, cx| {
                                                                    let mut config = AppConfig::load();
                                                                    if source_name == "monitor" {
                                                                        if let Some(t) = config.global_audio_tracks.get_mut(track_idx) {
                                                                            t.volume = val;
                                                                        }
                                                                    } else if let Some(game) = config.game_registry.get_mut(&source_name) {
                                                                        if let Some(audio) = game.audio_routing.as_mut() {
                                                                            if let Some(t) = audio.get_mut(track_idx) {
                                                                                t.volume = val;
                                                                            }
                                                                        }
                                                                    }
                                                                    config.save();
                                                                    cx.notify();
                                                                });
                                                            })
                                                    })
                                                    .child(div().w(px(40.0)).text_xs().text_right().child(format!("{:.0}%", track.volume * 100.0)))
                                            )
                                    )
                            )
                    )
                } else {
                    this
                }
            })
    }

    fn render_track_header(&self, name: &str, audio_idx: Option<usize>, _volume: Option<f32>, color: Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let is_video = audio_idx.is_none();
        
        HStack::new()
            .w(px(160.0))
            .h(px(if is_video { 32.0 } else { 24.0 }))
            .px_3()
            .bg(theme.tokens.muted.opacity(0.3))
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .justify_between()
            .items_center()
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(if is_video { "video" } else { "play" }).size(px(12.0)).color(if is_video { color } else { theme.tokens.muted_foreground }))
                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).child(name.to_string()))
            )
            .when_some(audio_idx, |this, idx| {
                this.child(
                    div()
                        .cursor(CursorStyle::PointingHand)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.audio_track_volume_popover = Some(idx);
                            cx.notify();
                        }))
                        .child(Icon::new("play").size(px(12.0)).color(theme.tokens.muted_foreground))
                )
            })
    }

    fn render_track_lane(&self, progress: f32, is_video: bool, color: Hsla) -> impl IntoElement {
        let theme = use_theme();
        div()
            .flex_1()
            .h(px(if is_video { 32.0 } else { 24.0 }))
            .bg(theme.tokens.background)
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress))
                            .bg(color.opacity(0.1))
                    )
            )
    }

    fn update_progress_from_mouse(&mut self, mouse_x: Pixels) {
        // Adjust for header width
        let header_width = px(160.0 + 8.0); // 160 header + 8 gap
        let timeline_width = self.timeline_bounds.size.width - header_width;
        if timeline_width > px(0.0) {
            let relative_x = (mouse_x - (self.timeline_bounds.left() + header_width)).clamp(px(0.0), timeline_width);
            self.scrubbing_progress = f32::from(relative_x) / f32::from(timeline_width);
        }
    }

    fn seek_to_mouse_x(&mut self, mouse_x: Pixels, perform_seek: bool) {
        self.update_progress_from_mouse(mouse_x);
        if perform_seek {
            if let Some(v) = &self.video_source {
                let duration = v.duration().as_secs_f64();
                let seek_time = self.scrubbing_progress as f64 * duration;
                let _ = v.seek(std::time::Duration::from_secs_f64(seek_time), false);
            }
        }
    }

    fn render_ticks_overlay(&self, color: Hsla) -> impl IntoElement {
        let header_width = px(160.0 + 8.0);
        let mut ticks = HStack::new().absolute().top_0().left(header_width).right_0().h(px(10.0)).items_end().px(px(10.0));
        for i in 0..21 {
            ticks = ticks.child(
                div()
                    .w(px(1.0))
                    .h(relative(if i % 5 == 0 { 0.8 } else { 0.4 }))
                    .bg(color)
                    .map(|this| if i < 20 { this.mr_auto() } else { this })
            );
        }
        ticks
    }

    fn render_playhead_overlay(&self, progress: f32, color: Hsla) -> impl IntoElement {
        let header_width = px(160.0 + 8.0);
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(header_width)
            .right_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(progress))
                    .ml(px(-1.0))
                    .w(px(2.0))
                    .bg(color)
                    .child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(relative(0.5))
                            .ml(px(-4.0))
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(color)
                    )
            )
    }

    fn render_markers_overlay(&self, start: f32, end: f32, color: Hsla) -> impl IntoElement {
        let header_width = px(160.0 + 8.0);
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(header_width)
            .right_0()
            .child(self.render_clip_range(start, end, color))
            .child(self.render_clip_marker(start, true, color, hsla(0.0, 0.0, 0.0, 1.0)))
            .child(self.render_clip_marker(end, false, color, hsla(0.0, 0.0, 0.0, 1.0)))
    }

    fn render_clip_range(&self, start: f32, end: f32, color: Hsla) -> impl IntoElement {
        if start < 0.0 { return div().into_any_element(); }
        let end_point = if end < 0.0 { 1.0 } else { end };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(start))
            .w(relative(end_point - start))
            .bg(color.opacity(0.15))
            .into_any_element()
    }

    fn render_clip_marker(&self, position: f32, is_in: bool, color: Hsla, text_color: Hsla) -> impl IntoElement {
        if position < 0.0 { return div().into_any_element(); }
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(position))
            .ml(px(if is_in { 0.0 } else { -2.0 }))
            .w(px(2.0))
            .bg(color)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(-6.0))
                    .w(px(14.0))
                    .h(px(18.0))
                    .bg(color)
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child(if is_in { "[" } else { "]" })
                    )
            )
            .into_any_element()
    }
}
