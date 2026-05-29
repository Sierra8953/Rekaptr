use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::layout::{HStack, VStack};
use adabraka_ui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

struct Assets {
    base: PathBuf,
}

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(Into::into)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        std::fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|e| {
                        e.ok()
                            .and_then(|e| e.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

const BG: u32 = 0x09090BFF;
const CARD: u32 = 0x18181BFF;
const BORDER: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const FG: u32 = 0xFAFAFAFF;
const MUTED: u32 = 0xA1A1AAFF;
const ACCENT: u32 = 0x27272AFF;

// ── Volume Slider ──────────────────────────────────────────────────

pub struct VolumeSlider {
    value: f32,      // 0.0 - 1.0
    muted: bool,
    dragging: bool,
    bounds: Bounds<Pixels>,
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>>,
}

impl VolumeSlider {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            value: 0.75,
            muted: false,
            dragging: false,
            bounds: Bounds::default(),
            on_change: None,
        }
    }

    fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    fn on_change(
        mut self,
        f: impl Fn(f32, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    fn effective_value(&self) -> f32 {
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
}

impl Render for VolumeSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .hover(|s| s.bg(rgba(ACCENT)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.muted = !this.muted;
                                    cx.notify();
                                    if let Some(cb) = &this.on_change {
                                        (cb)(this.effective_value(), window, cx);
                                    }
                                }),
                            )
                            .child(
                                Icon::new(IconSource::Named(icon_name.into()))
                                    .size(px(16.0))
                                    .color(if self.muted {
                                        rgba(MUTED).into()
                                    } else {
                                        rgba(FG).into()
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
                                    if let Some(cb) = &this.on_change {
                                        (cb)(this.value, window, cx);
                                    }
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
                            .text_color(rgba(MUTED))
                            .text_right()
                            .font_family("Consolas")
                            .child(pct_text),
                    ),
            )
            // Full-window drag overlay — captures mouse everywhere during drag
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
                                    cx.notify();
                                    return;
                                }
                                this.update_from_mouse(event.position.x);
                                cx.notify();
                                if let Some(cb) = &this.on_change {
                                    (cb)(this.value, window, cx);
                                }
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.dragging = false;
                                    cx.notify();
                                }),
                            ),
                    )
                    .with_priority(2),
                )
            })
    }
}

// ── Vertical Volume Slider ─────────────────────────────────────────

pub struct VerticalVolumeSlider {
    value: f32,
    muted: bool,
    dragging: bool,
    bounds: Bounds<Pixels>,
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>>,
}

impl VerticalVolumeSlider {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            value: 0.66,
            muted: false,
            dragging: false,
            bounds: Bounds::default(),
            on_change: None,
        }
    }

    fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    fn on_change(
        mut self,
        f: impl Fn(f32, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    fn effective_value(&self) -> f32 {
        if self.muted { 0.0 } else { self.value }
    }

    fn update_from_mouse(&mut self, y: Pixels) {
        let track_top = self.bounds.top();
        let track_height = self.bounds.size.height;
        if track_height <= px(0.0) {
            return;
        }
        let relative = (y - track_top).clamp(px(0.0), track_height);
        self.value = (1.0 - f32::from(relative) / f32::from(track_height)).clamp(0.0, 1.0);
    }
}

impl Render for VerticalVolumeSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.effective_value();
        let pct_text: SharedString = format!("{}%", (value * 100.0).round() as i32).into();
        let icon_name = if self.muted || value == 0.0 {
            "volume-x"
        } else if value < 0.5 {
            "volume-1"
        } else {
            "volume-2"
        };
        let dragging = self.dragging;

        div()
            .relative()
            .h_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.0))
                    .w(px(40.0))
                    .h_full()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(FG))
                            .child(pct_text),
                    )
                    .child(
                        div()
                            .id("vvol-track-area")
                            .relative()
                            .flex_1()
                            .w(px(28.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.dragging = true;
                                    this.update_from_mouse(event.position.y);
                                    if this.muted {
                                        this.muted = false;
                                    }
                                    cx.notify();
                                    if let Some(cb) = &this.on_change {
                                        (cb)(this.value, window, cx);
                                    }
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
                                        let track_w = px(4.0);
                                        let track_x =
                                            bounds.left() + (bounds.size.width - track_w) / 2.0;
                                        let track_h = bounds.size.height;

                                        let track_rect = Bounds::new(
                                            point(track_x, bounds.top()),
                                            size(track_w, track_h),
                                        );
                                        window.paint_quad(
                                            fill(track_rect, gpui::hsla(0.0, 0.0, 0.3, 1.0))
                                                .corner_radii(px(2.0)),
                                        );

                                        let fill_h = track_h * val;
                                        if fill_h > px(0.0) {
                                            let fill_rect = Bounds::new(
                                                point(track_x, bounds.top() + track_h - fill_h),
                                                size(track_w, fill_h),
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
                                        let center_x = track_x + track_w / 2.0;
                                        let thumb_y = bounds.top() + track_h - fill_h;

                                        if dragging {
                                            let glow_r = thumb_r + px(4.0);
                                            let glow_rect = Bounds::new(
                                                point(center_x - glow_r, thumb_y - glow_r),
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
                                            point(center_x - thumb_r, thumb_y - thumb_r),
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
                                            point(center_x - inner_r, thumb_y - inner_r),
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
                            .id("vvol-mute-btn")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(ACCENT)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.muted = !this.muted;
                                    cx.notify();
                                    if let Some(cb) = &this.on_change {
                                        (cb)(this.effective_value(), window, cx);
                                    }
                                }),
                            )
                            .child(
                                Icon::new(IconSource::Named(icon_name.into()))
                                    .size(px(16.0))
                                    .color(if self.muted {
                                        rgba(MUTED).into()
                                    } else {
                                        rgba(FG).into()
                                    }),
                            ),
                    ),
            )
            // Full-window drag overlay
            .when(dragging, |el| {
                el.child(
                    deferred(
                        div()
                            .id("v-drag-overlay")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                                if event.pressed_button != Some(MouseButton::Left) {
                                    this.dragging = false;
                                    cx.notify();
                                    return;
                                }
                                this.update_from_mouse(event.position.y);
                                cx.notify();
                                if let Some(cb) = &this.on_change {
                                    (cb)(this.value, window, cx);
                                }
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.dragging = false;
                                    cx.notify();
                                }),
                            ),
                    )
                    .with_priority(2),
                )
            })
    }
}

// ── Demo App ───────────────────────────────────────────────────────

struct SliderDemo {
    h_slider1: Entity<VolumeSlider>,
    h_slider2: Entity<VolumeSlider>,
    h_slider3: Entity<VolumeSlider>,
    v_slider1: Entity<VerticalVolumeSlider>,
    v_slider2: Entity<VerticalVolumeSlider>,
    v_slider3: Entity<VerticalVolumeSlider>,
    log: String,
}

impl SliderDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let e = cx.entity().downgrade();
        let e2 = cx.entity().downgrade();
        let e3 = cx.entity().downgrade();
        let ev1 = cx.entity().downgrade();
        let ev2 = cx.entity().downgrade();
        let ev3 = cx.entity().downgrade();

        Self {
            h_slider1: cx.new(|cx| {
                VolumeSlider::new(cx).with_value(0.75).on_change(move |v, _, cx| {
                    let _ = e.update(cx, |this, cx| {
                        this.log = format!("System Audio → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            h_slider2: cx.new(|cx| {
                VolumeSlider::new(cx).with_value(1.0).on_change(move |v, _, cx| {
                    let _ = e2.update(cx, |this, cx| {
                        this.log = format!("Game Audio → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            h_slider3: cx.new(|cx| {
                VolumeSlider::new(cx).with_value(0.5).on_change(move |v, _, cx| {
                    let _ = e3.update(cx, |this, cx| {
                        this.log = format!("Mic → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            v_slider1: cx.new(|cx| {
                VerticalVolumeSlider::new(cx).with_value(0.75).on_change(move |v, _, cx| {
                    let _ = ev1.update(cx, |this, cx| {
                        this.log = format!("V1 → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            v_slider2: cx.new(|cx| {
                VerticalVolumeSlider::new(cx).with_value(1.0).on_change(move |v, _, cx| {
                    let _ = ev2.update(cx, |this, cx| {
                        this.log = format!("V2 → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            v_slider3: cx.new(|cx| {
                VerticalVolumeSlider::new(cx).with_value(0.5).on_change(move |v, _, cx| {
                    let _ = ev3.update(cx, |this, cx| {
                        this.log = format!("V3 → {}%", (v * 100.0).round() as i32);
                        cx.notify();
                    });
                })
            }),
            log: String::new(),
        }
    }
}

fn slider_row(label: &str, slider: Entity<VolumeSlider>) -> impl IntoElement {
    HStack::new()
        .items_center()
        .gap(px(12.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(rgba(0x3F3F4650))
        .child(
            div()
                .w(px(120.0))
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(FG))
                .child(label.to_string()),
        )
        .child(div().flex_1().child(slider))
}

impl Render for SliderDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .flex()
            .justify_center()
            .pt(px(40.0))
            .child(
                VStack::new()
                    .gap(px(24.0))
                    .w(px(600.0))
                    // Horizontal sliders
                    .child(
                        VStack::new()
                            .bg(rgba(CARD))
                            .border_1()
                            .border_color(rgba(BORDER))
                            .rounded_lg()
                            .p(px(16.0))
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(PRIMARY))
                                    .mb(px(4.0))
                                    .child("HORIZONTAL VOLUME"),
                            )
                            .child(slider_row("System Audio", self.h_slider1.clone()))
                            .child(slider_row("Game Audio", self.h_slider2.clone()))
                            .child(slider_row("Microphone", self.h_slider3.clone())),
                    )
                    // Vertical sliders
                    .child(
                        VStack::new()
                            .bg(rgba(CARD))
                            .border_1()
                            .border_color(rgba(BORDER))
                            .rounded_lg()
                            .p(px(16.0))
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(PRIMARY))
                                    .mb(px(4.0))
                                    .child("VERTICAL VOLUME (MIXER STYLE)"),
                            )
                            .child(
                                HStack::new()
                                    .gap(px(16.0))
                                    .justify_center()
                                    .h(px(180.0))
                                    .child(
                                        VStack::new()
                                            .items_center()
                                            .h_full()
                                            .gap(px(4.0))
                                            .child(
                                                div().flex_1().w(px(40.0)).child(self.v_slider1.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgba(MUTED))
                                                    .child("System"),
                                            ),
                                    )
                                    .child(
                                        VStack::new()
                                            .items_center()
                                            .h_full()
                                            .gap(px(4.0))
                                            .child(
                                                div().flex_1().w(px(40.0)).child(self.v_slider2.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgba(MUTED))
                                                    .child("Game"),
                                            ),
                                    )
                                    .child(
                                        VStack::new()
                                            .items_center()
                                            .h_full()
                                            .gap(px(4.0))
                                            .child(
                                                div().flex_1().w(px(40.0)).child(self.v_slider3.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgba(MUTED))
                                                    .child("Mic"),
                                            ),
                                    ),
                            ),
                    )
                    // Log
                    .when(!self.log.is_empty(), |d| {
                        d.child(
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .bg(rgba(CARD))
                                .rounded(px(4.0))
                                .text_size(px(12.0))
                                .text_color(rgba(MUTED))
                                .child(self.log.clone()),
                        )
                    }),
            )
    }
}

fn main() {
    Application::new()
        .with_assets(Assets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        })
        .run(|cx| {
            adabraka_ui::init(cx);
            adabraka_ui::set_icon_base_path("assets/icons");
            install_theme(cx, Theme::dark());

            let bounds = Bounds::centered(None, size(px(700.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Volume Slider Test".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| SliderDemo::new(cx)),
            )
            .unwrap();
        });
}
