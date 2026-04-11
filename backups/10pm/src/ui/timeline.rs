use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{LumaWorkspace, TimelineDragTarget};

impl LumaWorkspace {
    pub fn render_timeline(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
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
        
        let clip_start = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::InMarker) {
            self.scrubbing_progress
        } else {
            self.clip_start
        };

        let clip_end = if self.is_scrubbing && self.drag_target == Some(TimelineDragTarget::OutMarker) {
            self.scrubbing_progress
        } else {
            self.clip_end
        };

        let view = cx.entity().downgrade();

        div()
            .w_full()
            .child(
                div()
                    .id("timeline-container")
                    .relative()
                    .w_full()
                    .h(px(45.0))
                    .bg(theme.tokens.background)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .overflow_hidden()
                    .cursor(CursorStyle::IBeam)
                    // 1. Ruler Ticks
                    .child(self.render_ticks(theme.tokens.border))
                    // 2. Clip Range Highlight
                    .child(self.render_clip_range(clip_start, clip_end, theme.tokens.primary))
                    // 3. Progress Fill
                    .child(self.render_progress_fill(progress, theme.tokens.primary))
                    // 4. Playhead
                    .child(self.render_playhead(progress, theme.tokens.foreground))
                    // 5. Clip Markers
                    .child(self.render_clip_marker(clip_start, true, theme.tokens.primary, theme.tokens.background))
                    .child(self.render_clip_marker(clip_end, false, theme.tokens.primary, theme.tokens.background))
                    // Interaction Layer
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
                        // Detect what we're clicking
                        let width = this.timeline_bounds.size.width;
                        let relative_x = (event.position.x - this.timeline_bounds.left()).clamp(px(0.0), width);
                        let percentage = (f32::from(relative_x) / f32::from(width));

                        if (percentage - this.clip_start).abs() < 0.02 {
                            this.drag_target = Some(TimelineDragTarget::InMarker);
                        } else if (percentage - this.clip_end).abs() < 0.02 {
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
                                    this.clip_start = this.scrubbing_progress;
                                }
                                Some(TimelineDragTarget::OutMarker) => {
                                    this.update_progress_from_mouse(event.position.x);
                                    this.clip_end = this.scrubbing_progress;
                                }
                                None => {}
                            }
                            cx.notify();
                        }
                    }))
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

    fn render_ticks(&self, color: Hsla) -> impl IntoElement {
        let mut ticks = HStack::new().absolute().inset_0().items_end().px(px(10.0));
        for i in 0..21 {
            ticks = ticks.child(
                div()
                    .w(px(1.0))
                    .h(relative(if i % 5 == 0 { 0.4 } else { 0.2 }))
                    .bg(color)
                    .map(|this| {
                        if i < 20 {
                            this.mr_auto()
                        } else {
                            this
                        }
                    })
            );
        }
        ticks
    }

    fn render_clip_range(&self, start: f32, end: f32, color: Hsla) -> impl IntoElement {
        if start < 0.0 {
            return div().into_any_element();
        }
        
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

    fn render_progress_fill(&self, progress: f32, color: Hsla) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(relative(progress))
            .bg(color.opacity(0.1))
    }

    fn render_playhead(&self, progress: f32, color: Hsla) -> impl IntoElement {
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
                    .top(relative(0.5))
                    .left(relative(0.5))
                    .ml(px(-2.0))
                    .mt(px(-2.0))
                    .w(px(4.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(color)
            )
    }

    fn render_clip_marker(&self, position: f32, is_in: bool, color: Hsla, text_color: Hsla) -> impl IntoElement {
        if position < 0.0 {
            return div().into_any_element();
        }
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
