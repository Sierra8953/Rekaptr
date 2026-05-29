use adabraka_ui::components::icon::Icon;
use adabraka_ui::gpui_ext::FluentBuilder;
use gpui::*;
use std::path::PathBuf;

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
const DESTRUCTIVE: u32 = 0xEF4444FF;

struct ControlBarMockup {
    is_recording: bool,
    is_playing: bool,
    position_secs: f64,
    duration_secs: f64,
    clip_in: Option<f64>,
    clip_out: Option<f64>,
}

impl ControlBarMockup {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            is_recording: false,
            is_playing: false,
            position_secs: 47.0,
            duration_secs: 312.0,
            clip_in: None,
            clip_out: None,
        }
    }

    fn format_time(secs: f64) -> String {
        let total = secs.max(0.0) as u64;
        let m = total / 60;
        let s = total % 60;
        format!("{}:{:02}", m, s)
    }

    fn icon_btn(
        id: &'static str,
        icon_name: &'static str,
        icon_size: f32,
        color: Hsla,
        on_click: impl Fn(&mut Self, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(ACCENT)))
            .active(|s| s.bg(rgba(BORDER)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
                cx.notify();
            }))
            .child(Icon::new(icon_name).size(px(icon_size)).color(color))
    }

    fn divider() -> impl IntoElement {
        div().w(px(1.0)).h(px(18.0)).bg(rgba(BORDER))
    }
}

impl Render for ControlBarMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_recording = self.is_recording;
        let is_playing = self.is_playing;

        let pos_text = Self::format_time(self.position_secs);
        let dur_text = Self::format_time(self.duration_secs);
        let time_display: SharedString = format!("{} / {}", pos_text, dur_text).into();

        let has_clip_range = self.clip_in.is_some();

        let clip_in_text: SharedString = self.clip_in
            .map(|t| Self::format_time(t))
            .unwrap_or_else(|| "--:--".to_string())
            .into();
        let clip_out_text: SharedString = self.clip_out
            .map(|t| Self::format_time(t))
            .unwrap_or_else(|| "--:--".to_string())
            .into();

        let muted_color: Hsla = rgba(MUTED).into();
        let fg_color: Hsla = rgba(FG).into();

        div()
            .size_full()
            .bg(rgba(BG))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(24.0))
            // Label
            .child(
                div()
                    .text_color(rgba(MUTED))
                    .text_xs()
                    .child("Control Bar Redesign — controls only"),
            )
            // ── The control bar ──
            .child(
                div()
                    .w(px(700.0))
                    .bg(rgba(CARD))
                    .border_1()
                    .border_color(rgba(BORDER))
                    .rounded(px(12.0))
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // Row 1: Transport + Time
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            // Left: transport cluster
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    // Record
                                    .child(
                                        div()
                                            .id("btn-rec")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(32.0))
                                            .h(px(32.0))
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .when(!is_recording, |el| {
                                                el.hover(|s| s.bg(hsla(0.0, 0.7, 0.5, 0.15)))
                                            })
                                            .when(is_recording, |el| {
                                                el.bg(hsla(0.0, 0.7, 0.5, 0.2))
                                                    .border_1()
                                                    .border_color(hsla(0.0, 0.7, 0.5, 0.4))
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_recording = !this.is_recording;
                                                cx.notify();
                                            }))
                                            .child(if is_recording {
                                                Icon::new("square")
                                                    .size(px(14.0))
                                                    .color(rgba(DESTRUCTIVE).into())
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .w(px(12.0))
                                                    .h(px(12.0))
                                                    .rounded_full()
                                                    .bg(rgba(DESTRUCTIVE))
                                                    .into_any_element()
                                            }),
                                    )
                                    .child(Self::divider())
                                    // Skip back
                                    .child(Self::icon_btn(
                                        "btn-back", "skip-back", 16.0, muted_color,
                                        |this, _, _| {
                                            this.position_secs = (this.position_secs - 10.0).max(0.0);
                                        },
                                        cx,
                                    ))
                                    // Play/Pause — primary accent
                                    .child(
                                        div()
                                            .id("btn-play")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded(px(8.0))
                                            .bg(rgba(PRIMARY))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0)))
                                            .active(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.53, 1.0)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.is_playing = !this.is_playing;
                                                cx.notify();
                                            }))
                                            .child(
                                                Icon::new(if is_playing { "pause" } else { "play" })
                                                    .size(px(18.0))
                                                    .color(fg_color),
                                            ),
                                    )
                                    // Skip forward
                                    .child(Self::icon_btn(
                                        "btn-fwd", "skip-forward", 16.0, muted_color,
                                        |this, _, _| {
                                            this.position_secs = (this.position_secs + 30.0).min(this.duration_secs);
                                        },
                                        cx,
                                    ))
                                    .child(Self::divider())
                                    // Time
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_family("Consolas")
                                            .text_color(rgba(MUTED))
                                            .child(time_display),
                                    ),
                            )
                            // Right: refresh
                            .child(
                                Self::icon_btn(
                                    "btn-refresh", "rotate-cw", 14.0, muted_color,
                                    |_, _, _| {},
                                    cx,
                                ),
                            ),
                    )
                    // Row 2: Markers + Clip controls
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            // Left: markers
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(MUTED))
                                            .mr(px(4.0))
                                            .child("Markers"),
                                    )
                                    .child(Self::render_marker_pill(
                                        "mk-flag", "flag",
                                        hsla(210.0 / 360.0, 0.78, 0.60, 1.0), cx,
                                    ))
                                    .child(Self::render_marker_pill(
                                        "mk-kill", "crosshair",
                                        hsla(0.0, 0.7, 0.55, 1.0), cx,
                                    ))
                                    .child(Self::render_marker_pill(
                                        "mk-death", "skull",
                                        hsla(30.0 / 360.0, 0.9, 0.55, 1.0), cx,
                                    ))
                                    .child(Self::render_marker_pill(
                                        "mk-star", "star",
                                        hsla(50.0 / 360.0, 0.9, 0.55, 1.0), cx,
                                    )),
                            )
                            // Right: clip controls
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    // IN
                                    .child(
                                        div()
                                            .id("btn-in")
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .px(px(8.0))
                                            .h(px(26.0))
                                            .rounded(px(5.0))
                                            .cursor_pointer()
                                            .bg(rgba(ACCENT))
                                            .hover(|s| s.bg(rgba(BORDER)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.clip_in = Some(this.position_secs);
                                                cx.notify();
                                            }))
                                            .child(Icon::new("chevron-right").size(px(12.0)).color(muted_color))
                                            .child(
                                                div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgba(MUTED)).child("IN"),
                                            )
                                            .child(
                                                div().text_xs().font_family("Consolas")
                                                    .text_color(rgba(MUTED)).ml(px(2.0))
                                                    .child(clip_in_text),
                                            ),
                                    )
                                    // OUT
                                    .child(
                                        div()
                                            .id("btn-out")
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .px(px(8.0))
                                            .h(px(26.0))
                                            .rounded(px(5.0))
                                            .cursor_pointer()
                                            .bg(rgba(ACCENT))
                                            .hover(|s| s.bg(rgba(BORDER)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.clip_out = Some(this.position_secs);
                                                cx.notify();
                                            }))
                                            .child(Icon::new("chevron-left").size(px(12.0)).color(muted_color))
                                            .child(
                                                div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgba(MUTED)).child("OUT"),
                                            )
                                            .child(
                                                div().text_xs().font_family("Consolas")
                                                    .text_color(rgba(MUTED)).ml(px(2.0))
                                                    .child(clip_out_text),
                                            ),
                                    )
                                    .child(Self::divider())
                                    // Save clip
                                    .child(
                                        div()
                                            .id("btn-save")
                                            .flex()
                                            .items_center()
                                            .gap(px(5.0))
                                            .px(px(10.0))
                                            .h(px(28.0))
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .when(has_clip_range, |el| {
                                                el.bg(rgba(PRIMARY))
                                                    .hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0)))
                                            })
                                            .when(!has_clip_range, |el| {
                                                el.bg(rgba(ACCENT))
                                                    .hover(|s| s.bg(rgba(BORDER)))
                                            })
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                this.clip_in = None;
                                                this.clip_out = None;
                                                cx.notify();
                                            }))
                                            .child(
                                                Icon::new("scissors").size(px(13.0))
                                                    .color(if has_clip_range { fg_color } else { muted_color }),
                                            )
                                            .child(
                                                div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if has_clip_range { rgba(FG) } else { rgba(MUTED) })
                                                    .child("SAVE"),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl ControlBarMockup {
    fn render_marker_pill(
        id: &'static str,
        icon_name: &'static str,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .bg(color.opacity(0.1))
            .hover(|s| s.bg(color.opacity(0.25)))
            .active(|s| s.bg(color.opacity(0.35)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, cx| {
                cx.notify();
            }))
            .child(Icon::new(icon_name).size(px(14.0)).color(color))
    }
}

fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.0), px(200.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Control Bar Redesign".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ControlBarMockup::new),
        )
        .unwrap();
    });
}
