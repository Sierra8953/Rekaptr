use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::components::slider::Slider;
use crate::ui::{RekaptrWorkspace, TimelineDragTarget};

impl RekaptrWorkspace {
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

        // Marker positions + kinds for the canvas
        let marker_data: Vec<(f32, crate::state::MarkerKind)> = self.timeline_markers.iter()
            .map(|m| ((m.time_secs / duration) as f32, m.kind))
            .collect();

        // Outer card wrapper for the entire timeline
        div()
            .w_full()
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .rounded_lg()
            .p_3()
            .child(
                HStack::new()
                    .w_full()
                    .gap_2()
                    .when_some(self.audio_track_volume_popover, |this, track_idx| {
                        let theme = theme.clone();

                        let current_vol_percentage = self.playback_volumes.get(track_idx).copied().unwrap_or(100.0);

                        this.child(
                            div()
                                .w(px(52.0))
                                .bg(theme.tokens.card)
                                .border_1()
                                .border_color(theme.tokens.border)
                                .rounded_md()
                                .py_3()
                                .px_1()
                                .child(
                                    VStack::new()
                                        .size_full()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.tokens.foreground)
                                                .child(format!("{:.0}%", current_vol_percentage))
                                        )
                                        .child({
                                            let view_for_slider = view.clone();
                                            div()
                                                .flex_1()
                                                .flex()
                                                .justify_center()
                                                .h(px(120.0))
                                                .child(
                                                    Slider::new(self.volume_slider_state.clone())
                                                        .vertical()
                                                        .on_change(move |value: f32, _window, cx| {
                                                            let _ = view_for_slider.update(cx, |this, cx| {
                                                                let volume = (value * 1.5) as f64; // 0-100 slider -> 0-150 volume
                                                                if let Some(idx) = this.last_audio_track_volume_popover {
                                                                    if this.playback_volumes.len() <= idx {
                                                                        this.playback_volumes.resize(idx + 1, 100.0);
                                                                    }
                                                                    this.playback_volumes[idx] = volume;
                                                                    this.volume_slider_last_value = volume as f32;

                                                                    // Throttle mpv updates to avoid lag
                                                                    let now = std::time::Instant::now();
                                                                    if now.duration_since(this.last_volume_update_at).as_millis() > 50 {
                                                                        this.last_volume_update_at = now;
                                                                        this.update_mpv_audio_mix();
                                                                    }
                                                                }
                                                                cx.notify();
                                                            });
                                                        })
                                                )
                                        })
                                )
                        )
                    })
                    .child(
                        // 1. Headers Column
                        VStack::new()
                            .w(px(160.0))
                            .pt(px(26.0)) // match marker icon area padding
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
                            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                                if event.modifiers.control {
                                    // Zooming
                                    let old_zoom = this.timeline_zoom;
                                    let delta = event.delta.pixel_delta(px(1.0)).y.0;
                                    let zoom_factor = if delta > 0.0 { 1.1 } else { 0.9 };
                                    this.timeline_zoom = (this.timeline_zoom * zoom_factor).clamp(1.0, 100.0);

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
                                    .pt(px(24.0)) // space for marker icons above tracks
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

                                            // Helper to convert progress (0.0 - 1.0) to screen X
                                            let to_x = |p: f32| visible_left + zoomed_width * p;

                                            // 1. Draw Clip Range Highlight with border edges
                                            if clip_start_prog >= 0.0 {
                                                let end = if clip_end_prog < 0.0 { 1.0 } else { clip_end_prog };
                                                let x_start = to_x(clip_start_prog).max(left);
                                                let x_end = to_x(end).min(left + width);

                                                if x_end > x_start {
                                                    let range_rect = Bounds::new(
                                                        point(x_start, top),
                                                        size(x_end - x_start, height)
                                                    );
                                                    // Filled highlight
                                                    window.paint_quad(fill(range_rect, theme.tokens.primary.opacity(0.08)));
                                                    // Top edge accent
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(x_start, top), size(x_end - x_start, px(2.0))),
                                                        theme.tokens.primary.opacity(0.3)
                                                    ));
                                                    // Bottom edge accent
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(x_start, top + height - px(2.0)), size(x_end - x_start, px(2.0))),
                                                        theme.tokens.primary.opacity(0.3)
                                                    ));
                                                }
                                            }

                                            // 2. Draw Playhead with glow and triangle head
                                            let playhead_x = to_x(progress);
                                            if playhead_x >= left && playhead_x <= left + width {
                                                // Glow behind playhead line
                                                let glow_rect = Bounds::new(
                                                    point(playhead_x - px(4.0), top),
                                                    size(px(8.0), height)
                                                );
                                                window.paint_quad(fill(glow_rect, gpui::white().opacity(0.06)));

                                                // Main playhead line (2px for visibility)
                                                let line_rect = Bounds::new(
                                                    point(playhead_x - px(1.0), top),
                                                    size(px(2.0), height)
                                                );
                                                window.paint_quad(fill(line_rect, gpui::white().opacity(0.9)));

                                                // Triangle head (downward-pointing arrow)
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

                                                // Bottom line cap (small rounded rect)
                                                let cap_rect = Bounds::new(
                                                    point(playhead_x - px(3.0), top + height - px(4.0)),
                                                    size(px(6.0), px(4.0))
                                                );
                                                window.paint_quad(fill(cap_rect, gpui::white().opacity(0.7)).corner_radii(px(1.0)));
                                            }

                                            // 3. Draw In/Out Markers with bracket handles
                                            let in_color = gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0);
                                            let out_color = gpui::hsla(346.0/360.0, 0.84, 0.61, 1.0);

                                            // Helper to draw a polished marker
                                            let draw_marker = |window: &mut Window, marker_x: Pixels, color: Hsla, is_in: bool| {
                                                let bracket_w = px(10.0);
                                                let bracket_h = px(16.0);
                                                let bracket_thickness = px(2.0);

                                                // Glow behind marker
                                                let glow_rect = Bounds::new(
                                                    point(marker_x - px(3.0), top),
                                                    size(px(6.0), height)
                                                );
                                                window.paint_quad(fill(glow_rect, color.opacity(0.1)));

                                                // Main vertical line (2px)
                                                let line_rect = Bounds::new(
                                                    point(marker_x - px(1.0), top),
                                                    size(bracket_thickness, height)
                                                );
                                                window.paint_quad(fill(line_rect, color.opacity(0.8)).corner_radii(px(1.0)));

                                                // Top bracket
                                                if is_in {
                                                    // [ shape — bracket extends right
                                                    // Horizontal top
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top), size(bracket_w, bracket_thickness)),
                                                        color
                                                    ).corner_radii(Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(0.0), bottom_right: px(0.0) }));
                                                    // Vertical side
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top), size(bracket_thickness, bracket_h)),
                                                        color
                                                    ).corner_radii(px(1.0)));
                                                    // Bottom bracket
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top + height - bracket_thickness), size(bracket_w, bracket_thickness)),
                                                        color
                                                    ).corner_radii(Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: px(2.0), bottom_right: px(2.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x, top + height - bracket_h), size(bracket_thickness, bracket_h)),
                                                        color
                                                    ).corner_radii(px(1.0)));
                                                } else {
                                                    // ] shape — bracket extends left
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_w, top), size(bracket_w, bracket_thickness)),
                                                        color
                                                    ).corner_radii(Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(0.0), bottom_right: px(0.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_thickness, top), size(bracket_thickness, bracket_h)),
                                                        color
                                                    ).corner_radii(px(1.0)));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_w, top + height - bracket_thickness), size(bracket_w, bracket_thickness)),
                                                        color
                                                    ).corner_radii(Corners { top_left: px(0.0), top_right: px(0.0), bottom_left: px(2.0), bottom_right: px(2.0) }));
                                                    window.paint_quad(fill(
                                                        Bounds::new(point(marker_x - bracket_thickness, top + height - bracket_h), size(bracket_thickness, bracket_h)),
                                                        color
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

                                            // 4. Draw user markers (icon above track + full-height line)
                                            let icon_sz = px(18.0);
                                            let icon_y = top - icon_sz - px(4.0);
                                            for &(prog, kind) in &marker_data {
                                                let mx = to_x(prog);
                                                if mx < left || mx > left + width { continue; }

                                                let color = match kind {
                                                    crate::state::MarkerKind::Flag => gpui::hsla(45.0/360.0, 0.9, 0.55, 1.0),
                                                    crate::state::MarkerKind::Kill => gpui::hsla(0.0/360.0, 0.85, 0.55, 1.0),
                                                    crate::state::MarkerKind::Death => gpui::hsla(270.0/360.0, 0.6, 0.55, 1.0),
                                                    crate::state::MarkerKind::Highlight => gpui::hsla(50.0/360.0, 1.0, 0.55, 1.0),
                                                };

                                                // Full-height vertical line like the playhead
                                                window.paint_quad(fill(
                                                    Bounds::new(point(mx - px(1.0), top), size(px(2.0), height)),
                                                    color.opacity(0.5)
                                                ));

                                                // SVG icon sitting just above the track
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

                                        let hit_threshold = 0.02 / this.timeline_zoom;

                                        // Check user markers first — clicking one seeks to it
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

                                        // Right-click on a user marker removes it
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
                    )
            )
    }

    #[allow(dead_code)]
    fn format_time(secs: f64, show_hours: bool) -> String {
        let total = secs.max(0.0) as u64;
        let h = total / 3600;
        let m = (total % 3600) / 60;
        let s = total % 60;
        if show_hours {
            format!("{:01}:{:02}:{:02}", h, m, s)
        } else {
            format!("{:01}:{:02}", m, s)
        }
    }

    fn render_track_header(&self, name: &str, audio_idx: Option<usize>, _volume: Option<f32>, color: Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let is_video = audio_idx.is_none();
        
        let is_selected = self.audio_track_volume_popover.is_some() && self.audio_track_volume_popover == audio_idx;
        let bg_color = if is_selected { theme.tokens.muted } else { theme.tokens.card };
        let border_color = if is_selected { theme.tokens.primary } else { theme.tokens.border };
        
        div()
            .w(px(160.0))
            .h(px(if is_video { 54.0 } else { 38.0 }))
            .px_3()
            .bg(bg_color)
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .when_some(audio_idx, |this, idx| {
                this.cursor(CursorStyle::PointingHand)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.open_volume_popover(idx, cx);
                    }))
            })
            .child(
                HStack::new()
                    .size_full()
                    .justify_between()
                    .items_center()
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .when(is_video, |this| this.child(Icon::new("video").size(px(16.0)).color(color)))
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(name.to_string()))
                    )
                    .when_some(audio_idx, |this, _| {
                        this.child(
                            Icon::new("speaker").size(px(14.0)).color(if is_selected { theme.tokens.primary } else { theme.tokens.muted_foreground })
                        )
                    })
            )
    }

    #[allow(dead_code)]
    fn render_ruler_row(&self, duration: f64, zoom: f32, scroll: f32) -> impl IntoElement {
        let theme = use_theme();
        let show_hours = duration >= 3600.0;

        div()
            .h(px(18.0))
            .w_full()
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left(px(-scroll))
                    .w(relative(zoom))
                    .h_full()
                    .children({
                        let num_labels = (5.0 * zoom).max(2.0) as i32;
                        (0..=num_labels).map(move |i| {
                            let p = i as f64 / num_labels as f64;
                            let time_secs = p * duration;
                            let label = Self::format_time(time_secs, show_hours);
                            let left_pct = (p * 100.0) as f32;

                            div()
                                .absolute()
                                .left(relative(left_pct / 100.0))
                                .top_0()
                                .text_xs()
                                .line_height(px(14.0))
                                .text_color(theme.tokens.muted_foreground.opacity(0.7))
                                .child(label)
                        })
                    })
            )
    }

    fn render_track_lane(&self, progress: f32, is_video: bool, color: Hsla) -> Div {
        let theme = use_theme();
        let zoom = self.timeline_zoom;
        let scroll = self.timeline_scroll;

        div()
            .flex_1()
            .h(px(if is_video { 54.0 } else { 38.0 }))
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
                    // Progress fill with gradient fade-out at the edge
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress))
                            .bg(color.opacity(0.08))
                    )
                    // Visual Filmstrip for Video Lane
                    .when(is_video, |this| {
                        this.child(
                            HStack::new()
                                .absolute()
                                .inset_0()
                                .size_full()
                                .opacity(0.12)
                                .gap(px(3.0))
                                .px(px(2.0))
                                .py(px(3.0))
                                .children((0..20).map(|i| {
                                    let shade = if i % 2 == 0 { 0.6 } else { 0.4 };
                                    div()
                                        .w(px(80.0))
                                        .h_full()
                                        .rounded(px(3.0))
                                        .bg(theme.tokens.muted_foreground.opacity(shade))
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
