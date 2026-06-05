use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{RekaptrWorkspace, TimelineDragTarget};

impl RekaptrWorkspace {
    pub fn render_timeline(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let audio_tracks = self.get_current_audio_tracks();
        let enabled_audio_tracks: Vec<_> = audio_tracks.into_iter().filter(|t| t.enabled).collect();

        self.ensure_track_vol_sliders(enabled_audio_tracks.len(), cx);

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

        let marker_data: Vec<(f32, crate::state::MarkerKind)> = self.timeline_markers.iter()
            .map(|m| ((m.time_secs / duration) as f32, m.kind))
            .collect();

        let track_colors = [
            gpui::hsla(187.0 / 360.0, 0.82, 0.55, 1.0), // cyan
            gpui::hsla(330.0 / 360.0, 0.81, 0.71, 1.0), // pink
            gpui::hsla(142.0 / 360.0, 0.69, 0.58, 1.0), // green
            gpui::hsla(45.0 / 360.0, 0.93, 0.58, 1.0),  // amber
            gpui::hsla(210.0 / 360.0, 0.78, 0.60, 1.0), // blue
        ];

        div()
            .w_full()
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .rounded_lg()
            .p_3()
            .child(
                div()
                    .flex()
                    .w_full()
                    .gap(px(8.0))
                    // Track headers with inline mixers
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .w(px(220.0))
                            .pt(px(22.0))
                            .child(self.render_video_track_header(theme.tokens.primary))
                            .children(
                                enabled_audio_tracks.iter().enumerate().map(|(i, track)| {
                                    let color = track_colors[i % track_colors.len()];
                                    let icon = crate::ui::audio_track_icon(&track.source_type);
                                    let slider = self.track_vol_sliders.get(i).cloned();
                                    self.render_audio_track_header(&track.name, icon, color, slider)
                                }),
                            ),
                    )
                    // Track lanes
                    .child(
                        div()
                            .id("timeline-tracks")
                            .relative()
                            .flex_1()
                            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                                if event.modifiers.control {
                                    let old_zoom = this.timeline_zoom;
                                    let delta = event.delta.pixel_delta(px(1.0)).y.0;
                                    let zoom_factor = if delta > 0.0 { 1.1 } else { 0.9 };
                                    this.timeline_zoom = (this.timeline_zoom * zoom_factor).clamp(1.0, 100.0);
                                    this.timeline_scroll *= this.timeline_zoom / old_zoom;
                                } else {
                                    let delta_x = event.delta.pixel_delta(px(1.0)).x.0;
                                    this.timeline_scroll = (this.timeline_scroll - delta_x).max(0.0);
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .pt(px(22.0))
                                    .child(self.render_track_lane(progress, true, theme.tokens.primary))
                                    .children(enabled_audio_tracks.iter().enumerate().map(|(i, _)| {
                                        let color = track_colors[i % track_colors.len()];
                                        self.render_track_lane(progress, false, color)
                                    }))
                            )
                            // Canvas overlays
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
                                        let marker_data = marker_data.clone();
                                        let theme = theme.clone();
                                        move |bounds, _, window, cx| {
                                            let _ = view.update(cx, |this, _| {
                                                this.timeline_bounds = bounds;
                                            });

                                            let width = bounds.size.width;
                                            let left = bounds.left();
                                            let marker_area_h = px(20.0);
                                            let top = bounds.top() + marker_area_h;
                                            let height = bounds.size.height - marker_area_h;

                                            let zoomed_width = width * zoom;
                                            let scroll_px = px(scroll);
                                            let visible_left = left - scroll_px;

                                            let to_x = |p: f32| visible_left + zoomed_width * p;

                                            // Clip range highlight
                                            if clip_start_prog >= 0.0 {
                                                let end = if clip_end_prog < 0.0 { 1.0 } else { clip_end_prog };
                                                let x_start = to_x(clip_start_prog).max(left);
                                                let x_end = to_x(end).min(left + width);

                                                if x_end > x_start {
                                                    let range_rect = Bounds::new(
                                                        point(x_start, top),
                                                        size(x_end - x_start, height)
                                                    );
                                                    window.paint_quad(fill(range_rect, theme.tokens.primary.opacity(0.08)));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(x_start, top), size(x_end - x_start, px(2.0))),
                                                        theme.tokens.primary.opacity(0.3)
                                                    ));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(x_start, top + height - px(2.0)), size(x_end - x_start, px(2.0))),
                                                        theme.tokens.primary.opacity(0.3)
                                                    ));
                                                }
                                            }

                                            // Playhead
                                            let playhead_x = to_x(progress);
                                            if playhead_x >= left && playhead_x <= left + width {
                                                let glow_rect = Bounds::new(
                                                    point(playhead_x - px(4.0), top),
                                                    size(px(8.0), height)
                                                );
                                                window.paint_quad(fill(glow_rect, gpui::white().opacity(0.06)));

                                                let line_rect = Bounds::new(
                                                    point(playhead_x - px(1.0), top),
                                                    size(px(2.0), height)
                                                );
                                                window.paint_quad(fill(line_rect, gpui::white().opacity(0.9)));

                                                let head_w = px(12.0);
                                                let head_h = px(10.0);
                                                if let Ok(path) = {
                                                    let mut pb = PathBuilder::fill();
                                                    pb.move_to(point(playhead_x - head_w / 2.0, top));
                                                    pb.line_to(point(playhead_x + head_w / 2.0, top));
                                                    pb.line_to(point(playhead_x, top + head_h));
                                                    pb.close();
                                                    pb.build()
                                                } {
                                                    window.paint_path(path, gpui::white());
                                                }

                                                let cap_rect = Bounds::new(
                                                    point(playhead_x - px(3.0), top + height - px(4.0)),
                                                    size(px(6.0), px(4.0))
                                                );
                                                window.paint_quad(fill(cap_rect, gpui::white().opacity(0.7)).corner_radii(px(1.0)));
                                            }

                                            // In/Out markers
                                            let in_color = gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0);
                                            let out_color = gpui::hsla(346.0/360.0, 0.84, 0.61, 1.0);

                                            let draw_marker = |window: &mut Window, marker_x: Pixels, color: Hsla, is_in: bool| {
                                                let bracket_w = px(10.0);
                                                let bracket_h = px(16.0);
                                                let bracket_thickness = px(2.0);

                                                window.paint_quad(fill(
                                                    Bounds::new(point(marker_x - px(3.0), top), size(px(6.0), height)),
                                                    color.opacity(0.1)
                                                ));
                                                window.paint_quad(fill(
                                                    Bounds::new(point(marker_x - px(1.0), top), size(bracket_thickness, height)),
                                                    color.opacity(0.8)
                                                ).corner_radii(px(1.0)));

                                                if is_in {
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top), size(bracket_w, bracket_thickness)), color
                                                    ).corner_radii(Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(0.0), bottom_right: px(0.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top), size(bracket_thickness, bracket_h)), color
                                                    ).corner_radii(px(1.0)));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top + height - bracket_thickness), size(bracket_w, bracket_thickness)), color
                                                    ).corner_radii(Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: px(2.0), bottom_right: px(2.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top + height - bracket_h), size(bracket_thickness, bracket_h)), color
                                                    ).corner_radii(px(1.0)));
                                                } else {
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_w, top), size(bracket_w, bracket_thickness)), color
                                                    ).corner_radii(Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(0.0), bottom_right: px(0.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_thickness, top), size(bracket_thickness, bracket_h)), color
                                                    ).corner_radii(px(1.0)));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_w, top + height - bracket_thickness), size(bracket_w, bracket_thickness)), color
                                                    ).corner_radii(Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: px(2.0), bottom_right: px(2.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_thickness, top + height - bracket_h), size(bracket_thickness, bracket_h)), color
                                                    ).corner_radii(px(1.0)));
                                                }
                                            };

                                            if clip_start_prog >= 0.0 {
                                                let marker_x = to_x(clip_start_prog);
                                                if marker_x >= left && marker_x <= left + width {
                                                    draw_marker(window, marker_x, in_color, true);
                                                }
                                            }
                                            if clip_end_prog >= 0.0 {
                                                let marker_x = to_x(clip_end_prog);
                                                if marker_x >= left && marker_x <= left + width {
                                                    draw_marker(window, marker_x, out_color, false);
                                                }
                                            }

                                            // User markers — centered within the marker band
                                            // above the lanes (bounds.top()..top), not clipped
                                            // off the top edge of the canvas.
                                            let icon_sz = px(16.0);
                                            let icon_y = bounds.top() + (marker_area_h - icon_sz) / 2.0;
                                            for &(prog, kind) in &marker_data {
                                                let mx = to_x(prog);
                                                if mx < left || mx > left + width { continue; }

                                                let (h, s, l, a) = kind.color_hsla();
                                                let color = gpui::hsla(h, s, l, a);

                                                window.paint_quad(fill(
                                                    Bounds::new(point(mx - px(1.0), top), size(px(2.0), height)),
                                                    color.opacity(0.5)
                                                ));

                                                let icon_path = SharedString::from(format!("icons/{}.svg", kind.icon_name()));
                                                let icon_bounds = Bounds::new(
                                                    point(mx - icon_sz / 2.0, icon_y),
                                                    size(icon_sz, icon_sz),
                                                );
                                                let _ = window.paint_svg(
                                                    icon_bounds,
                                                    icon_path,
                                                    TransformationMatrix::unit(),
                                                    color,
                                                    cx,
                                                );
                                            }
                                        }
                                    }
                                )
                                .absolute()
                                .inset_0()
                                .size_full()
                            )
                            // Interaction overlay
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

                                        let hit_threshold = 0.02 / this.timeline_zoom;

                                        let clicked_user_marker = this.timeline_markers.iter().position(|m| {
                                            let m_prog = (m.time_secs / duration) as f32;
                                            (percentage - m_prog).abs() < hit_threshold
                                        });

                                        if let Some(_marker_idx) = clicked_user_marker {
                                            let marker_time = this.timeline_markers[_marker_idx].time_secs;
                                            if let Some(v) = &this.video_source {
                                                let _ = v.seek(std::time::Duration::from_secs_f64(marker_time), false);
                                            }
                                            cx.notify();
                                            return;
                                        }

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
                                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                                        let width = this.timeline_bounds.size.width;
                                        let zoomed_width = width * this.timeline_zoom;
                                        let scroll_px = px(this.timeline_scroll);

                                        let mouse_x_relative = event.position.x - this.timeline_bounds.left() + scroll_px;
                                        let percentage = f32::from(mouse_x_relative) / f32::from(zoomed_width);

                                        let hit_threshold = 0.02 / this.timeline_zoom;

                                        let clicked_marker = this.timeline_markers.iter().position(|m| {
                                            let m_prog = (m.time_secs / duration) as f32;
                                            (percentage - m_prog).abs() < hit_threshold
                                        });

                                        if let Some(idx) = clicked_marker {
                                            this.remove_marker(idx, cx);
                                        }
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
                    ),
            )
    }

    fn render_video_track_header(&self, color: Hsla) -> impl IntoElement {
        let theme = use_theme();
        div()
            .w(px(220.0))
            .h(px(50.0))
            .px(px(10.0))
            .py(px(6.0))
            .bg(theme.tokens.card)
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(3.0))
                    .h(px(14.0))
                    .rounded(px(2.0))
                    .bg(color),
            )
            .child(Icon::new("video").size(px(13.0)).color(color))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Video"),
            )
    }

    fn render_audio_track_header(
        &self,
        name: &str,
        icon: &str,
        color: Hsla,
        slider: Option<Entity<super::volume_slider::VolumeSlider>>,
    ) -> impl IntoElement {
        let theme = use_theme();
        div()
            .w(px(220.0))
            .h(px(64.0))
            .px(px(10.0))
            .py(px(8.0))
            .bg(theme.tokens.card)
            .rounded_md()
            .border_1()
            .border_color(theme.tokens.border)
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(3.0))
                            .h(px(14.0))
                            .rounded(px(2.0))
                            .bg(color),
                    )
                    .child(Icon::new(icon).size(px(13.0)).color(color))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(name.to_string()),
                    ),
            )
            .when_some(slider, |this, s| this.child(s))
    }

    fn render_track_lane(&self, progress: f32, is_video: bool, color: Hsla) -> Div {
        let theme = use_theme();
        let zoom = self.timeline_zoom;
        let scroll = self.timeline_scroll;

        div()
            .flex_1()
            .h(px(if is_video { 50.0 } else { 64.0 }))
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
                            .bg(color.opacity(0.08))
                    )
            )
    }

    fn update_progress_from_mouse(&mut self, mouse_x: Pixels, duration: f64) {
        let width = self.timeline_bounds.size.width;
        let zoomed_width = width * self.timeline_zoom;
        let scroll_px = px(self.timeline_scroll);

        if zoomed_width > px(0.0) {
            let relative_x = (mouse_x - self.timeline_bounds.left() + scroll_px).clamp(px(0.0), zoomed_width);
            let mut percentage = f32::from(relative_x) / f32::from(zoomed_width);

            let snap_threshold_secs = 0.5;
            let snap_threshold_prog = (snap_threshold_secs / duration.max(1.0)) as f32;

            let blocks = self.app_state.recording.current_session_blocks.lock();
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
