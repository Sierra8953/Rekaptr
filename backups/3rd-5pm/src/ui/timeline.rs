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

                                    // 0. Draw Ruler Marks
                                    let num_marks = 20; 
                                    let ruler_offset = px(2.0);
                                    for i in 0..=num_marks {
                                        let x = left + width * (i as f32 / num_marks as f32);
                                        let is_major = i % 4 == 0; 
                                        let mark_height = if is_major { px(12.0) } else { px(6.0) };
                                        window.paint_quad(fill(
                                            Bounds::new(point(x - px(0.5), top + ruler_offset), size(px(1.0), mark_height)),
                                            theme.tokens.muted_foreground.opacity(0.4)
                                        ));
                                    }

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
                                        point(playhead_x - px(0.5), top),
                                        size(px(1.0), height)
                                    );
                                    window.paint_quad(fill(line_rect, gpui::white()));
                                    
                                    let head_rect = Bounds::new(
                                        point(playhead_x - px(3.0), top - px(1.0)),
                                        size(px(6.0), px(6.0))
                                    );
                                    window.paint_quad(fill(head_rect, gpui::white()).corner_radii(px(1.0)));

                                    // 3. Draw Markers
                                    let in_color = gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0);
                                    let out_color = gpui::hsla(346.0/360.0, 0.84, 0.61, 1.0);

                                    if clip_start_prog >= 0.0 {
                                        let marker_x = left + width * clip_start_prog;
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(0.5), top), size(px(1.0), height)), in_color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x, top), size(px(8.0), px(2.0))), in_color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x, top + height - px(2.0)), size(px(8.0), px(2.0))), in_color));
                                    }
                                    if clip_end_prog >= 0.0 {
                                        let marker_x = left + width * clip_end_prog;
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(0.5), top), size(px(1.0), height)), out_color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(8.0), top), size(px(8.0), px(2.0))), out_color));
                                        window.paint_quad(fill(Bounds::new(point(marker_x - px(8.0), top + height - px(2.0)), size(px(8.0), px(2.0))), out_color));
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
    }

    fn render_track_header(&self, name: &str, audio_idx: Option<usize>, _volume: Option<f32>, color: Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let is_video = audio_idx.is_none();
        
        HStack::new()
            .w(px(160.0))
            .h(px(if is_video { 36.0 } else { 32.0 }))
            .px_3()
            .bg(theme.tokens.card)
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .justify_between()
            .items_center()
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(Icon::new(if is_video { "video" } else { "play" }).size(px(16.0)).color(if is_video { color } else { theme.tokens.primary }))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(name.to_string()))
            )
            .when_some(audio_idx, |this, idx| {
                this.child(
                    div()
                        .cursor(CursorStyle::PointingHand)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.open_volume_popover(idx, cx);
                        }))
                        .child(Icon::new("play").size(px(14.0)).color(theme.tokens.muted_foreground))
                )
            })
    }

    fn render_track_lane(&self, progress: f32, is_video: bool, color: Hsla) -> impl IntoElement {
        let theme = use_theme();
        div()
            .flex_1()
            .h(px(if is_video { 36.0 } else { 32.0 }))
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
                            .bg(color.opacity(0.05))
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
