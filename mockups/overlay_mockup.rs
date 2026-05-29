// Mockup for the in-game overlay.
//
// This is a Steam-style overlay: a surface the player *summons* (e.g. Shift+Tab)
// which dims the game behind it and presents a large centered capture preview
// plus quick actions — not a passive always-on HUD shown while actively playing.
//
// The real overlay would render as a separate transparent, always-on-top window
// layered over the game (NOT via DLL injection / graphics-API hooking — that
// trips anti-cheat). When summoned it becomes interactive and dims the frame;
// when dismissed the game resumes.
//
// Self-contained: no real capture, hotkeys, or window flags. A faux "game"
// backdrop stands in for the live frame. The overlay's own buttons are
// interactive so you can preview each state.

use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::layout::{HStack, VStack};
use adabraka_ui::prelude::*;
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

// ── Theme (matches the other mockups) ───────────────────────────────
const PRIMARY: u32 = 0x8B5CF6FF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const REC: u32 = 0xEF4444FF;
const AMBER: u32 = 0xF59E0BFF;
const GOOD: u32 = 0x22C55EFF;

const BORDER: u32 = 0x2A2A30FF;

// The overlay surface: a frosted dark panel floating over the dimmed game.
const SURFACE: u32 = 0x111114F2; // ~95% opaque
const SURFACE_2: u32 = 0x1A1A1FFF;
const PANEL_BORDER: u32 = 0xFFFFFF1F;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Marker {
    None,
    Flag,
    Kill,
    Death,
    Highlight,
}

impl Marker {
    fn label(self) -> &'static str {
        match self {
            Marker::None => "",
            Marker::Flag => "Flag",
            Marker::Kill => "Kill",
            Marker::Death => "Death",
            Marker::Highlight => "Highlight",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Marker::None => "flag",
            Marker::Flag => "flag",
            Marker::Kill => "crosshair",
            Marker::Death => "skull",
            Marker::Highlight => "star",
        }
    }
    fn color(self) -> u32 {
        match self {
            Marker::Highlight => AMBER,
            Marker::Kill => GOOD,
            Marker::Death => REC,
            _ => PRIMARY,
        }
    }
}

struct OverlayMockup {
    recording: bool,
    mic_muted: bool,
    saved: bool,
    last_marker: Marker,
}

impl OverlayMockup {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            recording: true,
            mic_muted: false,
            saved: false,
            last_marker: Marker::None,
        }
    }
}

impl Render for OverlayMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("root")
            .size_full()
            .text_color(rgba(FG))
            .relative()
            // The game keeps rendering behind the overlay…
            .child(self.faux_game())
            // …but the summoned overlay dims it and captures focus.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000C2))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.overlay_surface(cx)),
            )
    }
}

impl OverlayMockup {
    // A stand-in for the live game frame the overlay floats above.
    fn faux_game(&self) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x101826FF))
            .child(div().absolute().inset_0().bg(rgba(0x1E293BFF)).opacity(0.55))
            .child(
                div()
                    .absolute()
                    .bottom(px(24.0))
                    .left(px(24.0))
                    .text_color(rgba(0xFFFFFF40))
                    .text_sm()
                    .child("100 ♥   50 ◆   ammo 24/96"),
            )
    }

    // The centered Steam-style overlay surface.
    fn overlay_surface(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(1000.0))
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .rounded_2xl()
            .shadow_xl()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.header())
            .child(
                div()
                    .px_6()
                    .py_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.preview())
                    .child(self.action_bar(cx)),
            )
            .child(self.footer())
    }

    fn header(&self) -> impl IntoElement {
        HStack::new()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .size(px(30.0))
                            .rounded_lg()
                            .bg(rgba(PRIMARY))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(rgba(FG))
                                    .font_weight(FontWeight::BOLD)
                                    .child("R"),
                            ),
                    )
                    .child(div().text_lg().font_weight(FontWeight::BOLD).child("Rekaptr"))
                    .child(self.recording_badge())
                    .child(self.mic_badge()),
            )
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgba(0xFFFFFF14))
                            .border_1()
                            .border_color(rgba(PANEL_BORDER))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Shift+Tab"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_MUTED))
                            .child("to resume game"),
                    ),
            )
    }

    fn recording_badge(&self) -> impl IntoElement {
        let (dot, label, tint) = if self.recording {
            (REC, "REC 12:34", REC)
        } else {
            (AMBER, "BUFFERING", AMBER)
        };
        HStack::new()
            .gap_2()
            .items_center()
            .px_2p5()
            .py_1()
            .rounded_full()
            .bg(rgba(0xFFFFFF0F))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .child(div().size(px(8.0)).rounded_full().bg(rgba(dot)))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgba(tint))
                    .child(label),
            )
    }

    fn mic_badge(&self) -> impl IntoElement {
        let (icon, tint, label) = if self.mic_muted {
            ("volume-x", REC, "Mic muted")
        } else {
            ("mic", GOOD, "Mic live")
        };
        HStack::new()
            .gap_1p5()
            .items_center()
            .px_2p5()
            .py_1()
            .rounded_full()
            .bg(rgba(0xFFFFFF0F))
            .border_1()
            .border_color(rgba(PANEL_BORDER))
            .child(
                Icon::new(IconSource::Named(icon.into()))
                    .size(px(13.0))
                    .color(rgba(tint).into()),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgba(FG_MUTED))
                    .child(label),
            )
    }

    // Large centered capture/replay preview — the focal point of the overlay.
    fn preview(&self) -> impl IntoElement {
        // Faux 16:9 frame standing in for the live capture / replay buffer.
        let frame = div()
            .w_full()
            .h(px(495.0))
            .rounded_xl()
            .overflow_hidden()
            .bg(rgba(0x101826FF))
            .border_1()
            .border_color(rgba(BORDER))
            .relative()
            .child(div().absolute().inset_0().bg(rgba(0x1E293BFF)).opacity(0.55))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(120.0))
                    .bg(rgba(0x000000AA)),
            )
            // Live pip top-left of the feed.
            .child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .left(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(0x000000AA))
                    .child(div().size(px(8.0)).rounded_full().bg(rgba(REC)))
                    .child(
                        div()
                            .text_color(rgba(FG))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child("LIVE · 1080p60"),
                    ),
            )
            // Centered play affordance to read as "replay".
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px(64.0))
                            .rounded_full()
                            .bg(rgba(0x000000AA))
                            .border_1()
                            .border_color(rgba(PANEL_BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named("play".into()))
                                    .size(px(26.0))
                                    .color(rgba(FG).into()),
                            ),
                    ),
            );

        VStack::new()
            .gap_2()
            .child(frame)
            .child(
                HStack::new()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Instant replay · last 30s"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_MUTED))
                            .child("Counter-Strike 2 · 1920×1080 @ 60"),
                    ),
            )
    }

    fn action_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let save_label = if self.saved { "Replay saved ✓" } else { "Save instant replay" };

        let markers = HStack::new()
            .gap_2()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(FG_MUTED))
                    .child("MARK"),
            )
            .child(self.marker_btn(Marker::Flag, cx))
            .child(self.marker_btn(Marker::Kill, cx))
            .child(self.marker_btn(Marker::Death, cx))
            .child(self.marker_btn(Marker::Highlight, cx));

        HStack::new()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("save", save_label)
                            .icon(IconSource::Named("save".into()))
                            .variant(if self.saved { ButtonVariant::Outline } else { ButtonVariant::Default })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.saved = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("rec", if self.recording { "Stop recording" } else { "Start recording" })
                            .icon(IconSource::Named(if self.recording { "square" } else { "circle-dot" }.into()))
                            .variant(ButtonVariant::Outline)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.recording = !this.recording;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mic", if self.mic_muted { "Unmute mic" } else { "Mute mic" })
                            .icon(IconSource::Named(if self.mic_muted { "volume-x" } else { "mic" }.into()))
                            .variant(ButtonVariant::Outline)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mic_muted = !this.mic_muted;
                                cx.notify();
                            })),
                    ),
            )
            .child(markers)
    }

    fn marker_btn(&self, m: Marker, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.last_marker == m;
        div()
            .id(SharedString::from(format!("mark-{}", m.label())))
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { rgba(m.color() & 0xFFFFFF33) } else { rgba(SURFACE_2) })
            .border_1()
            .border_color(if active { rgba(m.color()) } else { rgba(BORDER) })
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.last_marker = m;
                cx.notify();
            }))
            .child(
                Icon::new(IconSource::Named(m.icon().into()))
                    .size(px(14.0))
                    .color(if active { rgba(m.color()).into() } else { rgba(FG_MUTED).into() }),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(m.label()),
            )
    }

    fn footer(&self) -> impl IntoElement {
        let pair = |key: &str, action: &str| {
            HStack::new()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xFFFFFF14))
                        .border_1()
                        .border_color(rgba(PANEL_BORDER))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(key.to_string()),
                )
                .child(div().text_xs().text_color(rgba(FG_MUTED)).child(action.to_string()))
        };
        HStack::new()
            .px_6()
            .py_3()
            .gap_5()
            .items_center()
            .border_t_1()
            .border_color(rgba(BORDER))
            .child(pair("F9", "Record"))
            .child(pair("F10", "Save replay"))
            .child(pair("F8", "Mark highlight"))
            .child(div().flex_1())
            .child(pair("Esc", "Resume game"))
    }
}

fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Overlay Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(OverlayMockup::new),
        )
        .unwrap();
    });
}
