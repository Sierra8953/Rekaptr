use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{LumaWorkspace, TimelineDragTarget};

impl LumaWorkspace {
    pub fn render_timeline(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let audio_tracks = self.get_current_audio_tracks();
        let enabled_audio_tracks: Vec<_> = audio_tracks.into_iter().filter(|t| t.enabled).collect();

        // Use direct video player duration and position for the unified master playlist
        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64().max(1.0))
        } else {
            (0.0, 1.0)
        };
        
        let progress = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::Playhead) {
            self.scrubbing_progress
        } else {
            (position / duration) as f32
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
        let zoom = self.timeline_zoom;
        let scroll = self.timeline_scroll;

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
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                        if event.modifiers.control {
                            // Zooming
                            let old_zoom = this.timeline_zoom;
                            let delta = event.delta.pixel_delta(px(1.0)).y.0;
                            let zoom_factor = if delta > 0.0 { 1.1 } else { 0.9 };
                            this.timeline_zoom = (this.timeline_zoom * zoom_factor).clamp(1.0, 100.0);
                            
                            // Adjust scroll to keep mouse centered? (Simple version for now)
                            this.timeline_scroll *= this.timeline_zoom / old_zoom;
                        } else {
                            // Scrolling
                            let delta_x = event.delta.pixel_delta(px(1.0)).x.0;
                            this.timeline_scroll = (this.timeline_scroll - delta_x).max(0.0);
                        }
                        cx.notify();
                    }))
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

                                    let zoomed_width = width * zoom;
                                    let scroll_px = px(scroll);
                                    let visible_left = left - scroll_px;

                                    // Helper to convert progress (0.0 - 1.0) to screen X
                                    let to_x = |p: f32| visible_left + zoomed_width * p;

                                    // 0. Draw Ruler Marks
                                    let num_marks = (20.0 * zoom) as i32;
                                    let ruler_offset = px(2.0);
                                    for i in 0..=num_marks {
                                        let p = i as f32 / num_marks as f32;
                                        let x = to_x(p);
                                        if x < left || x > left + width { continue; }
                                        
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
                                        let x_start = to_x(clip_start_prog).max(left);
                                        let x_end = to_x(end).min(left + width);
                                        
                                        if x_end > x_start {
                                            let range_rect = Bounds::new(
                                                point(x_start, top),
                                                size(x_end - x_start, height)
                                            );
                                            window.paint_quad(fill(range_rect, theme.tokens.primary.opacity(0.1)));
                                        }
                                    }

                                    // 2. Draw Playhead Line
                                    let playhead_x = to_x(progress);
                                    if playhead_x >= left && playhead_x <= left + width {
                                        let line_rect = Bounds::new(
                                            point(playhead_x - px(0.5), top),
                                            size(px(1.0), height)
                                        );
                                        window.paint_quad(fill(line_rect, gpui::white()));
                                        
                                        let head_rect = Bounds::new(
                                            point(playhead_x - px(4.0), top - px(2.0)),
                                            size(px(8.0), px(8.0))
                                        );
                                        window.paint_quad(fill(head_rect, gpui::white()).corner_radii(px(2.0)));
                                    }

                                    // 3. Draw Markers (Reverted to simple style)
                                    let in_color = gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0);
                                    let out_color = gpui::hsla(346.0/360.0, 0.84, 0.61, 1.0);

                                    if clip_start_prog >= 0.0 {
                                        let marker_x = to_x(clip_start_prog);
                                        if marker_x >= left && marker_x <= left + width {
                                            window.paint_quad(fill(Bounds::new(point(marker_x - px(0.5), top), size(px(1.0), height)), in_color));
                                            window.paint_quad(fill(Bounds::new(point(marker_x, top), size(px(8.0), px(2.0))), in_color));
                                            window.paint_quad(fill(Bounds::new(point(marker_x, top + height - px(2.0)), size(px(8.0), px(2.0))), in_color));
                                        }
                                    }
                                    if clip_end_prog >= 0.0 {
                                        let marker_x = to_x(clip_end_prog);
                                        if marker_x >= left && marker_x <= left + width {
                                            window.paint_quad(fill(Bounds::new(point(marker_x - px(0.5), top), size(px(1.0), height)), out_color));
                                            window.paint_quad(fill(Bounds::new(point(marker_x - px(8.0), top), size(px(8.0), px(2.0))), out_color));
                                            window.paint_quad(fill(Bounds::new(point(marker_x - px(8.0), top + height - px(2.0)), size(px(8.0), px(2.0))), out_color));
                                        }
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
                                let zoomed_width = width * this.timeline_zoom;
                                let scroll_px = px(this.timeline_scroll);
                                
                                let mouse_x_relative = event.position.x - this.timeline_bounds.left() + scroll_px;
                                let percentage = f32::from(mouse_x_relative) / f32::from(zoomed_width);

                                let current_in_prog = if duration > 0.0 && this.clip_start >= 0.0 { (this.clip_start / duration) as f32 } else { -1.0 };
                                let current_out_prog = if duration > 0.0 && this.clip_end >= 0.0 { (this.clip_end / duration) as f32 } else { -1.0 };

                                // Hit testing for markers (wider hit area at zoom)
                                let hit_threshold = 0.02 / this.timeline_zoom;
                                
                                if (percentage - current_in_prog).abs() < hit_threshold {
                                    this.drag_target = Some(TimelineDragTarget::InMarker);
                                } else if (percentage - current_out_prog).abs() < hit_threshold {
                                    this.drag_target = Some(TimelineDragTarget::OutMarker);
                                } else {
                                    this.drag_target = Some(TimelineDragTarget::Playhead);
                                    this.seek_to_mouse_x(event.position.x, true, duration);
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
                                            this.seek_to_mouse_x(event.position.x, !throttle, duration);
                                            if throttle { this.last_seek_at = now; }
                                        }
                                        Some(TimelineDragTarget::InMarker) => {
                                            this.update_progress_from_mouse(event.position.x, duration);
                                            this.clip_start = this.scrubbing_progress as f64 * duration;
                                        }
                                        Some(TimelineDragTarget::OutMarker) => {
                                            this.update_progress_from_mouse(event.position.x, duration);
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
            .h(px(if is_video { 48.0 } else { 32.0 }))
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
        let zoom = self.timeline_zoom;
        let scroll = self.timeline_scroll;

        div()
            .flex_1()
            .h(px(if is_video { 48.0 } else { 32.0 }))
            .bg(theme.tokens.background)
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left(px(-scroll))
                    .w(relative(zoom))
                    .h_full()
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress))
                            .bg(color.opacity(0.05))
                    )
                    // Visual Filmstrip Placeholder for Video Lane
                    .when(is_video, |this| {
                        this.child(
                            HStack::new()
                                .size_full()
                                .opacity(0.15)
                                .gap_1()
                                .children((0..10).map(|_| {
                                    div()
                                        .w(px(120.0))
                                        .h_full()
                                        .bg(theme.tokens.muted)
                                        .border_1()
                                        .border_color(theme.tokens.border)
                                }))
                        )
                    })
            )
    }

    fn update_progress_from_mouse(&mut self, mouse_x: Pixels, duration: f64) {
        let width = self.timeline_bounds.size.width;
        let zoomed_width = width * self.timeline_zoom;
        let scroll_px = px(self.timeline_scroll);

        if zoomed_width > px(0.0) {
            let relative_x = (mouse_x - self.timeline_bounds.left() + scroll_px).clamp(px(0.0), zoomed_width);
            let mut percentage = f32::from(relative_x) / f32::from(zoomed_width);
            
            // Magnetic Snapping
            let snap_threshold_secs = 0.5; // Snap within 0.5s
            let snap_threshold_prog = (snap_threshold_secs / duration.max(1.0)) as f32;
            
            let blocks = self.app_state.current_session_blocks.lock();
            let mut cumulative_duration = 0.0;
            for block in blocks.iter() {
                cumulative_duration += block.duration_secs;
                let boundary_prog = (cumulative_duration / duration) as f32;
                if (percentage - boundary_prog).abs() < snap_threshold_prog {
                    percentage = boundary_prog;
                    break;
                }
            }
            
            self.scrubbing_progress = percentage;
        }
    }

    fn seek_to_mouse_x(&mut self, mouse_x: Pixels, perform_seek: bool, duration: f64) {
        self.update_progress_from_mouse(mouse_x, duration);
        if perform_seek {
            if let Some(v) = &self.video_source {
                let target_time = self.scrubbing_progress as f64 * duration;
                let _ = v.seek(std::time::Duration::from_secs_f64(target_time), false);
            }
        }
    }
}
