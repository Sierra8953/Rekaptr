use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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

const THROTTLE_MS: u128 = 100;

// ── Inline Track Slider ───────────────────────────────────────────
// Compact horizontal slider for embedding in track headers

struct TrackSlider {
    value: f32,
    muted: bool,
    dragging: bool,
    bounds: Bounds<Pixels>,
    last_change_at: Instant,
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>>,
}

impl TrackSlider {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            value: 0.75,
            muted: false,
            dragging: false,
            bounds: Bounds::default(),
            last_change_at: Instant::now(),
            on_change: None,
        }
    }

    fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    #[allow(dead_code)]
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

impl Render for TrackSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.effective_value();
        let dragging = self.dragging;
        let muted = self.muted;
        let pct: SharedString = format!("{}%", (value * 100.0).round() as i32).into();

        let icon_name: &str = if muted || value == 0.0 {
            "volume-x"
        } else if value < 0.5 {
            "volume-1"
        } else {
            "volume-2"
        };

        div()
            .relative()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(22.0))
                    .child(
                        div()
                            .id("ts-mute")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(ACCENT)))
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
                                    .size(px(13.0))
                                    .color(if muted {
                                        rgba(MUTED).into()
                                    } else {
                                        rgba(FG).into()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("ts-track")
                            .relative()
                            .flex_1()
                            .h(px(22.0))
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
                                        let track_h = px(3.0);
                                        let track_y =
                                            bounds.top() + (bounds.size.height - track_h) / 2.0;
                                        let track_w = bounds.size.width;

                                        // Background track
                                        window.paint_quad(
                                            fill(
                                                Bounds::new(
                                                    point(bounds.left(), track_y),
                                                    size(track_w, track_h),
                                                ),
                                                gpui::hsla(0.0, 0.0, 0.25, 1.0),
                                            )
                                            .corner_radii(px(1.5)),
                                        );

                                        // Fill
                                        let fill_w = track_w * val;
                                        if fill_w > px(0.0) {
                                            window.paint_quad(
                                                fill(
                                                    Bounds::new(
                                                        point(bounds.left(), track_y),
                                                        size(fill_w, track_h),
                                                    ),
                                                    gpui::hsla(258.0 / 360.0, 0.9, 0.67, 1.0),
                                                )
                                                .corner_radii(px(1.5)),
                                            );
                                        }

                                        // Thumb
                                        let thumb_r = if dragging { px(6.0) } else { px(4.5) };
                                        let thumb_x = bounds.left() + fill_w;
                                        let thumb_y = track_y + track_h / 2.0;

                                        if dragging {
                                            let glow_r = thumb_r + px(3.0);
                                            window.paint_quad(
                                                fill(
                                                    Bounds::new(
                                                        point(thumb_x - glow_r, thumb_y - glow_r),
                                                        size(glow_r * 2.0, glow_r * 2.0),
                                                    ),
                                                    gpui::hsla(258.0 / 360.0, 0.9, 0.67, 0.2),
                                                )
                                                .corner_radii(glow_r),
                                            );
                                        }

                                        window.paint_quad(
                                            fill(
                                                Bounds::new(
                                                    point(thumb_x - thumb_r, thumb_y - thumb_r),
                                                    size(thumb_r * 2.0, thumb_r * 2.0),
                                                ),
                                                gpui::hsla(258.0 / 360.0, 0.9, 0.67, 1.0),
                                            )
                                            .corner_radii(thumb_r),
                                        );
                                    },
                                )
                                .size_full()
                            }),
                    )
                    .child(
                        div()
                            .min_w(px(30.0))
                            .text_color(rgba(MUTED))
                            .text_right()
                            .font_family("Consolas")
                            .text_size(px(10.0))
                            .child(pct),
                    ),
            )
            .when(dragging, |el| {
                el.child(
                    deferred(
                        div()
                            .id("ts-drag-overlay")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .cursor_pointer()
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, window, cx| {
                                    if event.pressed_button != Some(MouseButton::Left) {
                                        this.dragging = false;
                                        this.fire_immediate(window, cx);
                                        cx.notify();
                                        return;
                                    }
                                    this.update_from_mouse(event.position.x);
                                    cx.notify();
                                    this.fire_throttled(window, cx);
                                },
                            ))
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
    }
}

// ── Track Data ────────────────────────────────────────────────────

#[derive(Clone)]
struct MockTrack {
    name: &'static str,
    icon: &'static str,
    color: u32,
    is_video: bool,
    solo: bool,
}

// ── Main App ──────────────────────────────────────────────────────

struct TimelineMockup {
    tracks: Vec<MockTrack>,
    sliders: Vec<Entity<TrackSlider>>,
    playhead: f32,
    clip_in: f32,
    clip_out: f32,
    scrubbing: bool,
    timeline_bounds: Bounds<Pixels>,
}

impl TimelineMockup {
    fn new(cx: &mut Context<Self>) -> Self {
        let tracks = vec![
            MockTrack {
                name: "Video",
                icon: "video",
                color: PRIMARY,
                is_video: true,
                solo: false,
            },
            MockTrack {
                name: "System Audio",
                icon: "speaker",
                color: 0x22D3EEFF,
                is_video: false,
                solo: false,
            },
            MockTrack {
                name: "VALORANT",
                icon: "gamepad-2",
                color: 0xF472B6FF,
                is_video: false,
                solo: false,
            },
            MockTrack {
                name: "Microphone",
                icon: "mic",
                color: 0x4ADE80FF,
                is_video: false,
                solo: false,
            },
        ];

        let sliders = vec![
            cx.new(|cx| TrackSlider::new(cx).with_value(1.0)),
            cx.new(|cx| TrackSlider::new(cx).with_value(0.8)),
            cx.new(|cx| TrackSlider::new(cx).with_value(0.65)),
            cx.new(|cx| TrackSlider::new(cx).with_value(0.5)),
        ];

        Self {
            tracks,
            sliders,
            playhead: 0.45,
            clip_in: 0.25,
            clip_out: 0.72,
            scrubbing: false,
            timeline_bounds: Bounds::default(),
        }
    }

    fn format_time(secs: f64) -> String {
        let total = secs.max(0.0) as u64;
        let m = total / 60;
        let s = total % 60;
        format!("{}:{:02}", m, s)
    }
}

impl Render for TimelineMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let playhead = self.playhead;
        let clip_in = self.clip_in;
        let clip_out = self.clip_out;
        let duration = 312.0_f64;
        let position = playhead as f64 * duration;

        let time_current = Self::format_time(position);
        let time_total = Self::format_time(duration);

        div()
            .size_full()
            .bg(rgba(BG))
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .p(px(24.0))
            .font_family("Segoe UI")
            .child(
                div()
                    .w(px(1000.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    // Transport bar
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_family("Consolas")
                                            .text_color(rgba(FG))
                                            .child(format!("{} / {}", time_current, time_total)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(2.0))
                                            .child(transport_btn("skip-back", cx))
                                            .child(transport_btn("play", cx))
                                            .child(transport_btn("skip-forward", cx)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(marker_btn("IN", 0x22C55EFF))
                                    .child(marker_btn("OUT", 0xEF4444FF))
                                    .child(
                                        div()
                                            .h(px(20.0))
                                            .w(px(1.0))
                                            .bg(rgba(BORDER)),
                                    )
                                    .child(transport_btn("flag", cx))
                                    .child(transport_btn("crosshair", cx))
                                    .child(transport_btn("star", cx)),
                            ),
                    )
                    // Timeline card
                    .child(
                        div()
                            .w_full()
                            .bg(rgba(CARD))
                            .border_1()
                            .border_color(rgba(BORDER))
                            .rounded_lg()
                            .p(px(12.0))
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
                                            .children(
                                                self.tracks
                                                    .iter()
                                                    .zip(self.sliders.iter())
                                                    .map(|(track, slider)| {
                                                        render_track_header(track, slider.clone())
                                                    }),
                                            ),
                                    )
                                    // Track lanes
                                    .child(
                                        div()
                                            .id("tl-tracks")
                                            .flex_1()
                                            .relative()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(4.0))
                                                    .pt(px(22.0))
                                                    .children(
                                                        self.tracks.iter().map(|track| {
                                                            render_track_lane(
                                                                track, playhead,
                                                            )
                                                        }),
                                                    ),
                                            )
                                            // Canvas overlays
                                            .child({
                                                let entity = cx.entity().clone();
                                                canvas(
                                                    move |bounds, _, cx| {
                                                        entity.update(cx, |this, _| {
                                                            this.timeline_bounds = bounds;
                                                        });
                                                    },
                                                    move |bounds, _, window, _cx| {
                                                        let marker_h = px(20.0);
                                                        let top = bounds.top() + marker_h;
                                                        let height =
                                                            bounds.size.height - marker_h;
                                                        let width = bounds.size.width;
                                                        let left = bounds.left();

                                                        let to_x = |p: f32| left + width * p;

                                                        // Clip range
                                                        let x_start = to_x(clip_in);
                                                        let x_end = to_x(clip_out);
                                                        if x_end > x_start {
                                                            let primary_hsla = gpui::hsla(
                                                                258.0 / 360.0,
                                                                0.9,
                                                                0.67,
                                                                1.0,
                                                            );
                                                            window.paint_quad(fill(
                                                                Bounds::new(
                                                                    point(x_start, top),
                                                                    size(
                                                                        x_end - x_start,
                                                                        height,
                                                                    ),
                                                                ),
                                                                primary_hsla.opacity(0.06),
                                                            ));
                                                            window.paint_quad(fill(
                                                                Bounds::new(
                                                                    point(x_start, top),
                                                                    size(
                                                                        x_end - x_start,
                                                                        px(2.0),
                                                                    ),
                                                                ),
                                                                primary_hsla.opacity(0.25),
                                                            ));
                                                            window.paint_quad(fill(
                                                                Bounds::new(
                                                                    point(
                                                                        x_start,
                                                                        top + height - px(2.0),
                                                                    ),
                                                                    size(
                                                                        x_end - x_start,
                                                                        px(2.0),
                                                                    ),
                                                                ),
                                                                primary_hsla.opacity(0.25),
                                                            ));
                                                        }

                                                        // In marker
                                                        draw_bracket_marker(
                                                            window,
                                                            to_x(clip_in),
                                                            top,
                                                            height,
                                                            gpui::hsla(
                                                                142.0 / 360.0,
                                                                0.71,
                                                                0.45,
                                                                1.0,
                                                            ),
                                                            true,
                                                        );
                                                        // Out marker
                                                        draw_bracket_marker(
                                                            window,
                                                            to_x(clip_out),
                                                            top,
                                                            height,
                                                            gpui::hsla(
                                                                346.0 / 360.0,
                                                                0.84,
                                                                0.61,
                                                                1.0,
                                                            ),
                                                            false,
                                                        );

                                                        // Playhead
                                                        let ph_x = to_x(playhead);
                                                        window.paint_quad(fill(
                                                            Bounds::new(
                                                                point(ph_x - px(3.0), top),
                                                                size(px(6.0), height),
                                                            ),
                                                            gpui::white().opacity(0.05),
                                                        ));
                                                        window.paint_quad(fill(
                                                            Bounds::new(
                                                                point(ph_x - px(1.0), top),
                                                                size(px(2.0), height),
                                                            ),
                                                            gpui::white().opacity(0.9),
                                                        ));
                                                        // Triangle head
                                                        if let Ok(path) = {
                                                            let mut pb = PathBuilder::fill();
                                                            let hw = px(6.0);
                                                            let hh = px(8.0);
                                                            pb.move_to(point(ph_x - hw, top));
                                                            pb.line_to(point(ph_x + hw, top));
                                                            pb.line_to(point(ph_x, top + hh));
                                                            pb.close();
                                                            pb.build()
                                                        } {
                                                            window.paint_path(
                                                                path,
                                                                gpui::white(),
                                                            );
                                                        }
                                                        // Bottom cap
                                                        window.paint_quad(
                                                            fill(
                                                                Bounds::new(
                                                                    point(
                                                                        ph_x - px(3.0),
                                                                        top + height - px(3.0),
                                                                    ),
                                                                    size(px(6.0), px(3.0)),
                                                                ),
                                                                gpui::white().opacity(0.7),
                                                            )
                                                            .corner_radii(px(1.0)),
                                                        );
                                                    },
                                                )
                                                .absolute()
                                                .inset_0()
                                                .size_full()
                                            })
                                            // Interaction overlay
                                            .child(
                                                div()
                                                    .id("tl-interact")
                                                    .absolute()
                                                    .inset_0()
                                                    .size_full()
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            |this,
                                                             event: &MouseDownEvent,
                                                             _,
                                                             cx| {
                                                                this.scrubbing = true;
                                                                let width = this
                                                                    .timeline_bounds
                                                                    .size
                                                                    .width;
                                                                if width > px(0.0) {
                                                                    let rel = (event.position.x
                                                                        - this
                                                                            .timeline_bounds
                                                                            .left())
                                                                    .clamp(px(0.0), width);
                                                                    this.playhead = f32::from(rel)
                                                                        / f32::from(width);
                                                                }
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.scrubbing = false;
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .on_mouse_move(cx.listener(
                                                        |this, event: &MouseMoveEvent, _, cx| {
                                                            if this.scrubbing {
                                                                let width = this
                                                                    .timeline_bounds
                                                                    .size
                                                                    .width;
                                                                if width > px(0.0) {
                                                                    let rel = (event.position.x
                                                                        - this
                                                                            .timeline_bounds
                                                                            .left())
                                                                    .clamp(px(0.0), width);
                                                                    this.playhead =
                                                                        f32::from(rel)
                                                                            / f32::from(width);
                                                                }
                                                                cx.notify();
                                                            }
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

fn render_track_header(track: &MockTrack, slider: Entity<TrackSlider>) -> impl IntoElement {
    let h = if track.is_video { 50.0 } else { 50.0 };
    let color: Hsla = rgba(track.color).into();

    div()
        .w(px(220.0))
        .h(px(h))
        .px(px(10.0))
        .py(px(6.0))
        .bg(rgba(CARD))
        .rounded_md()
        .border_1()
        .border_color(rgba(BORDER))
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
                .child(
                    Icon::new(IconSource::Named(track.icon.into()))
                        .size(px(13.0))
                        .color(color),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgba(FG))
                        .child(track.name),
                )
                .when(!track.is_video, |this| {
                    this.child(div().flex_1()).child(
                        div()
                            .id(SharedString::from(format!("solo-{}", track.name)))
                            .text_size(px(9.0))
                            .font_weight(FontWeight::BOLD)
                            .px(px(4.0))
                            .py(px(1.0))
                            .rounded(px(2.0))
                            .cursor_pointer()
                            .text_color(if track.solo {
                                gpui::rgba(0xFFFFFFFF)
                            } else {
                                rgba(MUTED)
                            })
                            .when(track.solo, |d| d.bg(rgba(PRIMARY)))
                            .when(!track.solo, |d| {
                                d.hover(|s| s.bg(rgba(ACCENT)))
                            })
                            .child("S"),
                    )
                }),
        )
        .when(!track.is_video, |this| this.child(slider))
}

fn render_track_lane(track: &MockTrack, progress: f32) -> impl IntoElement {
    let color: Hsla = rgba(track.color).into();
    let h = if track.is_video { 50.0 } else { 50.0 };

    div()
        .h(px(h))
        .bg(rgba(BG))
        .rounded_md()
        .border_1()
        .border_color(rgba(BORDER))
        .relative()
        .overflow_hidden()
        // Progress fill
        .child(
            div()
                .absolute()
                .inset_0()
                .child(
                    div()
                        .h_full()
                        .w(relative(progress))
                        .bg(color.opacity(0.06)),
                ),
        )
        // Waveform / filmstrip decoration
        .when(track.is_video, |this| {
            this.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .px(px(3.0))
                    .py(px(4.0))
                    .opacity(0.12)
                    .children((0..16).map(|i| {
                        let shade = if i % 2 == 0 { 0.5 } else { 0.35 };
                        div()
                            .w(px(60.0))
                            .h_full()
                            .rounded(px(3.0))
                            .bg(gpui::hsla(0.0, 0.0, shade, 1.0))
                    })),
            )
        })
        .when(!track.is_video, |this| {
            this.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_end()
                    .px(px(2.0))
                    .py(px(4.0))
                    .opacity(0.15)
                    .children((0..80).map(|i| {
                        let h = ((i as f32 * 0.7).sin().abs() * 0.6 + 0.15) * 100.0;
                        div()
                            .flex_1()
                            .h(relative(h / 100.0))
                            .rounded(px(1.0))
                            .bg(color)
                    })),
            )
        })
}

fn transport_btn(icon: &str, _cx: &mut Context<TimelineMockup>) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("t-{icon}")))
        .flex()
        .items_center()
        .justify_center()
        .w(px(30.0))
        .h(px(28.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(ACCENT)))
        .child(
            Icon::new(IconSource::Named(icon.into()))
                .size(px(16.0))
                .color(rgba(FG).into()),
        )
}

fn marker_btn(label: &'static str, color: u32) -> impl IntoElement {
    let c: Hsla = rgba(color).into();
    div()
        .id(SharedString::from(format!("m-{label}")))
        .flex()
        .items_center()
        .justify_center()
        .px(px(8.0))
        .h(px(24.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .border_1()
        .border_color(c.opacity(0.5))
        .hover(|s| s.bg(c.opacity(0.15)))
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(c)
        .child(label)
}

fn draw_bracket_marker(
    window: &mut Window,
    marker_x: Pixels,
    top: Pixels,
    height: Pixels,
    color: Hsla,
    is_in: bool,
) {
    let bracket_w = px(8.0);
    let bracket_h = px(14.0);
    let t = px(2.0);

    window.paint_quad(fill(
        Bounds::new(point(marker_x - px(2.0), top), size(px(4.0), height)),
        color.opacity(0.08),
    ));

    window.paint_quad(
        fill(
            Bounds::new(point(marker_x - px(1.0), top), size(t, height)),
            color.opacity(0.8),
        )
        .corner_radii(px(1.0)),
    );

    if is_in {
        window.paint_quad(
            fill(
                Bounds::new(point(marker_x, top), size(bracket_w, t)),
                color,
            )
            .corner_radii(Corners {
                top_left: px(2.0),
                top_right: px(2.0),
                bottom_left: px(0.0),
                bottom_right: px(0.0),
            }),
        );
        window.paint_quad(
            fill(
                Bounds::new(point(marker_x, top), size(t, bracket_h)),
                color,
            )
            .corner_radii(px(1.0)),
        );
        window.paint_quad(
            fill(
                Bounds::new(
                    point(marker_x, top + height - t),
                    size(bracket_w, t),
                ),
                color,
            )
            .corner_radii(Corners {
                top_left: px(0.0),
                top_right: px(0.0),
                bottom_left: px(2.0),
                bottom_right: px(2.0),
            }),
        );
        window.paint_quad(
            fill(
                Bounds::new(
                    point(marker_x, top + height - bracket_h),
                    size(t, bracket_h),
                ),
                color,
            )
            .corner_radii(px(1.0)),
        );
    } else {
        window.paint_quad(
            fill(
                Bounds::new(point(marker_x - bracket_w, top), size(bracket_w, t)),
                color,
            )
            .corner_radii(Corners {
                top_left: px(2.0),
                top_right: px(2.0),
                bottom_left: px(0.0),
                bottom_right: px(0.0),
            }),
        );
        window.paint_quad(
            fill(
                Bounds::new(point(marker_x - t, top), size(t, bracket_h)),
                color,
            )
            .corner_radii(px(1.0)),
        );
        window.paint_quad(
            fill(
                Bounds::new(
                    point(marker_x - bracket_w, top + height - t),
                    size(bracket_w, t),
                ),
                color,
            )
            .corner_radii(Corners {
                top_left: px(0.0),
                top_right: px(0.0),
                bottom_left: px(2.0),
                bottom_right: px(2.0),
            }),
        );
        window.paint_quad(
            fill(
                Bounds::new(
                    point(marker_x - t, top + height - bracket_h),
                    size(t, bracket_h),
                ),
                color,
            )
            .corner_radii(px(1.0)),
        );
    }
}

fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1100.0), px(400.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Timeline Mixer Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(TimelineMockup::new),
        )
        .unwrap();
    });
}
