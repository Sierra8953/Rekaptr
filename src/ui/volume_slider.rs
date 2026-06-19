use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::prelude::*;
use gpui::*;
use std::sync::Arc;
use std::time::Instant;

const THROTTLE_MS: u128 = 100;

pub struct VolumeSlider {
    value: f32,
    muted: bool,
    dragging: bool,
    bounds: Bounds<Pixels>,
    last_change_at: Instant,
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>>,
    /// Compact "meter bar" variant used by the dashboard audio mixer: a flat
    /// filled bar with an edge tick, no mute button and no percent label. Still
    /// fully draggable.
    compact: bool,
    /// Fill color for the compact variant (defaults to the primary violet).
    fill_color: Option<Hsla>,
}

impl VolumeSlider {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            value: 0.75,
            muted: false,
            dragging: false,
            bounds: Bounds::default(),
            last_change_at: Instant::now(),
            on_change: None,
            compact: false,
            fill_color: None,
        }
    }

    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn fill_color(mut self, color: Hsla) -> Self {
        self.fill_color = Some(color);
        self
    }

    pub fn on_change(
        mut self,
        f: impl Fn(f32, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    pub fn effective_value(&self) -> f32 {
        if self.muted { 0.0 } else { self.value }
    }

    fn volume_icon(&self) -> &'static str {
        let v = self.effective_value();
        if self.muted || v == 0.0 {
            "volume-x"
        } else if v < 0.5 {
            "volume-1"
        } else {
            "volume-2"
        }
    }

    fn update_from_mouse(&mut self, x: Pixels) {
        let track_left = self.bounds.left();
        let track_width = self.bounds.size.width;
        if track_width <= px(0.0) {
            return;
        }
        let relative = (x - track_left).clamp(px(0.0), track_width);
        self.value = (f32::from(relative) / f32::from(track_width)).clamp(0.0, 1.0);
    }

    fn fire_throttled(&mut self, window: &mut Window, cx: &mut App) {
        let now = Instant::now();
        if now.duration_since(self.last_change_at).as_millis() >= THROTTLE_MS {
            self.last_change_at = now;
            if let Some(cb) = &self.on_change {
                (cb)(self.value, window, cx);
            }
        }
    }

    fn fire_immediate(&mut self, window: &mut Window, cx: &mut App) {
        self.last_change_at = Instant::now();
        if let Some(cb) = &self.on_change {
            (cb)(self.value, window, cx);
        }
    }
}

impl VolumeSlider {
    /// Compact meter-bar variant rendered to match the dashboard mockup's
    /// `meter()` exactly: a rounded-full well with a gradient fill and a thin
    /// white peak tick. No mute button or percent label (the mixer draws those
    /// separately). The whole 24px cell is the drag target even though the bar
    /// itself is 6px tall.
    fn render_compact(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = use_theme();
        let value = self.effective_value();
        let dragging = self.dragging;
        let fill_color = self.fill_color.unwrap_or(gpui::hsla(258.0 / 360.0, 0.9, 0.67, 1.0));
        let grad = gpui::linear_gradient(
            90.0,
            gpui::linear_color_stop(fill_color.opacity(0.67), 0.0),
            gpui::linear_color_stop(fill_color, 1.0),
        );

        div()
            .relative()
            .w_full()
            .child(
                div()
                    .id("vol-track-area")
                    .relative()
                    .w_full()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.dragging = true;
                            this.update_from_mouse(event.position.x);
                            if this.muted {
                                this.muted = false;
                            }
                            cx.notify();
                            this.fire_immediate(window, cx);
                        }),
                    )
                    // the meter bar — exact mockup markup
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(8.0))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(theme.tokens.background)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .child(div().absolute().top_0().bottom_0().left_0().w(relative(value)).bg(grad))
                            .when(value > 0.0, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(relative(value))
                                        .w(px(2.0))
                                        .bg(gpui::white().opacity(if dragging { 1.0 } else { 0.6 })),
                                )
                            }),
                    )
                    // invisible bounds capture spanning the full drag cell
                    .child({
                        let entity = cx.entity().clone();
                        canvas(
                            move |bounds, _, cx| {
                                entity.update(cx, |this, _| {
                                    this.bounds = bounds;
                                });
                            },
                            move |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0()
                        .size_full()
                    }),
            )
            .when(dragging, |el| {
                el.child(
                    deferred(
                        div()
                            .id("h-drag-overlay")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    this.dragging = false;
                                    this.fire_immediate(window, cx);
                                    cx.notify();
                                    return;
                                }
                                this.update_from_mouse(event.position.x);
                                cx.notify();
                                this.fire_throttled(window, cx);
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.dragging = false;
                                    cx.notify();
                                    this.fire_immediate(window, cx);
                                }),
                            ),
                    )
                    .with_priority(2),
                )
            })
            .into_any_element()
    }
}

impl Render for VolumeSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.compact {
            return self.render_compact(cx);
        }

        let theme = use_theme();
        let value = self.effective_value();
        let pct_text: SharedString = format!("{}%", (value * 100.0).round() as i32).into();
        let icon_name = self.volume_icon();
        let dragging = self.dragging;

        div()
            .relative()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(28.0))
                    .child(
                        div()
                            .id("vol-mute-btn")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.tokens.accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.muted = !this.muted;
                                    cx.notify();
                                    this.fire_immediate(window, cx);
                                }),
                            )
                            .child(
                                Icon::new(IconSource::Named(icon_name.into()))
                                    .size(px(16.0))
                                    .color(if self.muted {
                                        theme.tokens.muted_foreground.into()
                                    } else {
                                        theme.tokens.foreground.into()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("vol-track-area")
                            .relative()
                            .flex_1()
                            .h(px(28.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.dragging = true;
                                    this.update_from_mouse(event.position.x);
                                    if this.muted {
                                        this.muted = false;
                                    }
                                    cx.notify();
                                    this.fire_immediate(window, cx);
                                }),
                            )
                            .child({
                                let entity = cx.entity().clone();
                                let val = value;
                                canvas(
                                    move |bounds, _, cx| {
                                        entity.update(cx, |this, _| {
                                            this.bounds = bounds;
                                        });
                                    },
                                    move |bounds, _, window, _cx| {
                                        let track_h = px(4.0);
                                        let track_y =
                                            bounds.top() + (bounds.size.height - track_h) / 2.0;
                                        let track_w = bounds.size.width;

                                        let track_rect = Bounds::new(
                                            point(bounds.left(), track_y),
                                            size(track_w, track_h),
                                        );
                                        window.paint_quad(
                                            fill(track_rect, gpui::hsla(0.0, 0.0, 0.3, 1.0))
                                                .corner_radii(px(2.0)),
                                        );

                                        let fill_w = track_w * val;
                                        if fill_w > px(0.0) {
                                            let fill_rect = Bounds::new(
                                                point(bounds.left(), track_y),
                                                size(fill_w, track_h),
                                            );
                                            window.paint_quad(
                                                fill(
                                                    fill_rect,
                                                    gpui::hsla(258.0 / 360.0, 0.9, 0.67, 1.0),
                                                )
                                                .corner_radii(px(2.0)),
                                            );
                                        }

                                        let thumb_r = if dragging { px(8.0) } else { px(6.0) };
                                        let thumb_x = bounds.left() + fill_w;
                                        let thumb_y = track_y + track_h / 2.0;

                                        if dragging {
                                            let glow_r = thumb_r + px(4.0);
                                            let glow_rect = Bounds::new(
                                                point(thumb_x - glow_r, thumb_y - glow_r),
                                                size(glow_r * 2.0, glow_r * 2.0),
                                            );
                                            window.paint_quad(
                                                fill(
                                                    glow_rect,
                                                    gpui::hsla(258.0 / 360.0, 0.9, 0.67, 0.2),
                                                )
                                                .corner_radii(glow_r),
                                            );
                                        }

                                        let thumb_rect = Bounds::new(
                                            point(thumb_x - thumb_r, thumb_y - thumb_r),
                                            size(thumb_r * 2.0, thumb_r * 2.0),
                                        );
                                        window.paint_quad(
                                            fill(
                                                thumb_rect,
                                                gpui::hsla(258.0 / 360.0, 0.9, 0.67, 1.0),
                                            )
                                            .corner_radii(thumb_r),
                                        );

                                        let inner_r = px(2.0);
                                        let inner_rect = Bounds::new(
                                            point(thumb_x - inner_r, thumb_y - inner_r),
                                            size(inner_r * 2.0, inner_r * 2.0),
                                        );
                                        window.paint_quad(
                                            fill(inner_rect, gpui::white().opacity(0.3))
                                                .corner_radii(inner_r),
                                        );
                                    },
                                )
                                .size_full()
                            }),
                    )
                    .child(
                        div()
                            .min_w(px(36.0))
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .text_right()
                            .font_family("Consolas")
                            .child(pct_text),
                    ),
            )
            .when(dragging, |el| {
                el.child(
                    deferred(
                        div()
                            .id("h-drag-overlay")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    this.dragging = false;
                                    this.fire_immediate(window, cx);
                                    cx.notify();
                                    return;
                                }
                                this.update_from_mouse(event.position.x);
                                cx.notify();
                                this.fire_throttled(window, cx);
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.dragging = false;
                                    cx.notify();
                                    this.fire_immediate(window, cx);
                                }),
                            ),
                    )
                    .with_priority(2),
                )
            })
            .into_any_element()
    }
}
