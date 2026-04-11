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
        
        // Use the actual live position for rendering
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

        HStack::new()
            .w_full()
            .gap_2()
            .child(
                // 1. Headers Column
                VStack::new()
                    .w(px(160.0))
                    .gap_1()
                    .child(self.render_track_header("Video", None, None, theme.tokens.primary, cx))
                    .children(enabled_audio_tracks.iter().enumerate().map(|(i, track)| {
                        self.render_track_header(&track.name, Some(i), Some(track.volume), theme.tokens.primary, cx)
                    }))
            )
            .child(
                // 2. Multi-Lane Track Area
                div()
                    .id("timeline-tracks")
                    .relative()
                    .flex_1()
                    .child(
                        VStack::new()
                            .gap_1()
                            .child(self.render_track_lane(progress, true, theme.tokens.primary))
                            .children(enabled_audio_tracks.iter().map(|_| {
                                self.render_track_lane(progress, false, theme.tokens.primary)
                            }))
                    )
                    // Overlays (Playhead & Markers)
                    .child(
                        canvas(
                            move |_, window, cx| {
                                let layout_id = window.request_layout(Style {
                                    size: size(relative(1.0).into(), relative(1.0).into()),
                                    ..Default::default()
                                }, [], cx);
                                (layout_id, ())
                            },
                            {
                                let theme = theme.clone();
                                move |bounds, _, window, cx| {
                                    let _ = view.update(cx, |this, _| {
                                        this.timeline_bounds = bounds;
                                    });

                                    let width = bounds.size.width;
                                    let left = bounds.left();
                                    let top = bounds.top();
                                    let height = bounds.size.height;

                                    // 1. Draw Clip Range Highlight
                                    if clip_start_prog >= 0.0 {
                                        let end = if clip_end_prog < 0.0 { 1.0 } else { clip_end_prog };
                                        let range_rect = Bounds::new(
                                            point(left + width * clip_start_prog, top),
                                            size(width * (end - clip_start_prog), height)
                                        );
                                        window.paint_quad(fill(range_rect, theme.tokens.primary.opacity(0.1)));
                                    }

                                    // 2. Draw Playhead Line
                                    let playhead_x = left + width * progress;
                                    let line_rect = Bounds::new(
                                        point(playhead_x - px(1.0), top),
                                        size(px(2.0), height)
                                    );
                                    window.paint_quad(fill(line_rect, theme.tokens.foreground));
                                    
                                    // Playhead Head
                                    let head_rect = Bounds::new(
                                        point(playhead_x - px(4.0), top - px(4.0)),
                                        size(px(8.0), px(8.0))
                                    );
                                    window.paint_quad(fill(head_rect, theme.tokens.foreground).corner_radii(px(4.0)));

                                    // 3. Draw Markers
                                    let color = theme.tokens.primary;
                                    if clip_start_prog >= 0.0 {
                                        let marker_x = left + width * clip_start_prog;
                                        window.paint_quad(fill(Bounds::new(point(marker_x, top), size(px(2.0), height)), color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(6.0), top), size(px(14.0), px(18.0))), color).corner_radii(px(3.0)));
                                    }
                                    if clip_end_prog >= 0.0 {
                                        let marker_x = left + width * clip_end_prog;
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(2.0), top), size(px(2.0), height)), color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(6.0), top), size(px(14.0), px(18.0))), color).corner_radii(px(3.0)));
                                    }
                                }
                            }
                        )
                        .absolute()
                        .inset_0()
                        .size_full()
                    )
                    // Global Interaction Area
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .size_full()
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
                                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
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
        let width = self.timeline_bounds.size.width;
        if width > px(0.0) {
            let relative_x = (mouse_x - self.timeline_bounds.left()).clamp(px(0.0), width);
            self.scrubbing_progress = f32::from(relative_x) / f32::from(width);
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
}
