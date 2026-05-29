// Add-source modal redesign mockup.
//
// Current modal forces the user through three tabs of encoder/audio/advanced
// knobs even when all they want is "pick window + go with defaults". This
// redesign collapses that into a single pane:
//   1. Pick the window (searchable, with "Detected" chip for the focused game)
//   2. Name + thumbnail preview of what you're adding
//   3. Settings default to "inherit global settings", with a compact summary
//      pill and an "Override defaults" toggle that reveals encoder / output /
//      quality / retention controls when the user actually wants to customize
//   4. Auto-record checkbox
//
// Self-contained: no real window enumeration, no config writes. All mocked.

use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
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

// ── Theme ───────────────────────────────────────────────────────────
const BG: u32 = 0x09090BFF;
const SURFACE: u32 = 0x121215FF;
const CARD: u32 = 0x18181BFF;
const CARD_HOVER: u32 = 0x22222AFF;
const BORDER: u32 = 0x2A2A30FF;
const BORDER_STRONG: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const PRIMARY_DIM_A25: u32 = 0x5B3FA840;
const SUCCESS: u32 = 0x22C55EFF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const FG_SUBTLE: u32 = 0x71717AFF;

// ── Mock data ───────────────────────────────────────────────────────
#[derive(Clone)]
struct MockWindow {
    hwnd: u64,
    title: String,
    process: String,
    resolution: String,
    thumb_tint: u32,
    is_game: bool,   // pretend we've detected this one as a game
    is_focused: bool,
}

fn mock_windows() -> Vec<MockWindow> {
    vec![
        MockWindow { hwnd: 1, title: "Elden Ring".into(),           process: "eldenring.exe".into(),    resolution: "2560×1440".into(), thumb_tint: 0x8B6F3FFF, is_game: true,  is_focused: true  },
        MockWindow { hwnd: 2, title: "Counter-Strike 2".into(),     process: "cs2.exe".into(),          resolution: "1920×1080".into(), thumb_tint: 0x6E7F66FF, is_game: true,  is_focused: false },
        MockWindow { hwnd: 3, title: "Helldivers 2".into(),         process: "helldivers2.exe".into(),  resolution: "2560×1440".into(), thumb_tint: 0x3F5B8BFF, is_game: true,  is_focused: false },
        MockWindow { hwnd: 4, title: "Baldur's Gate 3".into(),      process: "bg3.exe".into(),          resolution: "3440×1440".into(), thumb_tint: 0x6B3A5BFF, is_game: true,  is_focused: false },
        MockWindow { hwnd: 5, title: "Visual Studio Code".into(),   process: "code.exe".into(),         resolution: "1920×1080".into(), thumb_tint: 0x1E3A5FFF, is_game: false, is_focused: false },
        MockWindow { hwnd: 6, title: "Discord".into(),              process: "discord.exe".into(),      resolution: "1920×1080".into(), thumb_tint: 0x3F4A8BFF, is_game: false, is_focused: false },
        MockWindow { hwnd: 7, title: "Firefox — Rekaptr roadmap".into(), process: "firefox.exe".into(), resolution: "1920×1080".into(), thumb_tint: 0x6B3F3AFF, is_game: false, is_focused: false },
    ]
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Rc {
    Cqp,
    Vbr,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TrackSource {
    System,
    Mic,
    App,
}

impl TrackSource {
    fn label(self) -> &'static str {
        match self {
            TrackSource::System => "System",
            TrackSource::Mic => "Mic",
            TrackSource::App => "App",
        }
    }
}

#[derive(Clone)]
struct MockTrack {
    name: &'static str,
    enabled: bool,
    source: TrackSource,
    device: String,
    app_count: usize,
}

// ── Workspace ───────────────────────────────────────────────────────
struct AddSourceMockup {
    search: Entity<InputState>,
    title: Entity<InputState>,
    windows: Vec<MockWindow>,
    selected_hwnd: Option<u64>,
    auto_record: bool,
    show_overrides: bool,

    // Override form (only meaningful when show_overrides = true)
    encoder: String,
    resolution: String,
    fps: u32,
    rate_control: Rc,
    cq: i32,
    bitrate: i32,
    retention_minutes: i32,

    // Audio routing
    audio_tracks: Vec<MockTrack>,
}

impl AddSourceMockup {
    fn new(cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(cx));
        let title = cx.new(|cx| InputState::new(cx));
        Self {
            search,
            title,
            windows: mock_windows(),
            selected_hwnd: Some(1), // default to the detected game
            auto_record: true,
            show_overrides: false,
            encoder: "HEVC".into(),
            resolution: "1920x1080".into(),
            fps: 60,
            rate_control: Rc::Cqp,
            cq: 23,
            bitrate: 16000,
            retention_minutes: 10,
            audio_tracks: vec![
                MockTrack { name: "Track 1", enabled: true,  source: TrackSource::System, device: "Speakers (Realtek)".into(),       app_count: 0 },
                MockTrack { name: "Track 2", enabled: true,  source: TrackSource::Mic,    device: "Shure MV7".into(),                app_count: 0 },
                MockTrack { name: "Track 3", enabled: false, source: TrackSource::App,    device: String::new(),                     app_count: 2 },
            ],
        }
    }

    fn filtered(&self, cx: &App) -> Vec<&MockWindow> {
        let q = self.search.read(cx).content().to_lowercase();
        self.windows
            .iter()
            .filter(|w| {
                q.is_empty()
                    || w.title.to_lowercase().contains(&q)
                    || w.process.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn selected(&self) -> Option<&MockWindow> {
        self.selected_hwnd
            .and_then(|h| self.windows.iter().find(|w| w.hwnd == h))
    }
}

// ── Render root ─────────────────────────────────────────────────────
impl Render for AddSourceMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .relative()
            .child(render_underlay())
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000CC))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.render_modal(cx)),
            )
    }
}

fn render_underlay() -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .bg(rgba(SURFACE))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_sm()
                .text_color(rgba(FG_SUBTLE))
                .child("(app behind the modal)"),
        )
}

impl AddSourceMockup {
    fn render_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(720.0))
            .max_h(px(860.0))
            .bg(rgba(CARD))
            .rounded_xl()
            .border_1()
            .border_color(rgba(BORDER))
            .shadow_xl()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.render_header())
            .child(
                div()
                    .id("add-src-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .p_6()
                            .gap_6()
                            .child(self.render_source_section(cx))
                            .child(self.render_details_section(cx))
                            .child(self.render_settings_section(cx))
                            .child(self.render_audio_section(cx))
                            .child(self.render_auto_record(cx)),
                    ),
            )
            .child(self.render_footer(cx))
    }

    fn render_header(&self) -> impl IntoElement {
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
                            .size(px(28.0))
                            .rounded_md()
                            .bg(rgba(PRIMARY_DIM_A25))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named("plus".into()))
                                    .size(px(16.0))
                                    .color(rgba(PRIMARY).into()),
                            ),
                    )
                    .child(
                        VStack::new()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Add game capture"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child("Pick a window and we'll do the rest."),
                            ),
                    ),
            )
            .child(
                Button::new("close", "")
                    .icon(IconSource::Named("x".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
    }

    // ── Section 1: Source picker ────────────────────────────────────
    fn render_source_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered(cx);
        VStack::new()
            .gap_2()
            .child(section_label("SOURCE"))
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(
                        Input::new(&self.search).placeholder("Search windows..."),
                    ))
                    .child(
                        Button::new("refresh", "")
                            .icon(IconSource::Named("rotate-cw".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
            .child(
                div()
                    .id("win-list")
                    .w_full()
                    .h(px(220.0))
                    .overflow_y_scroll()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgba(BORDER))
                    .bg(rgba(SURFACE))
                    .child(
                        VStack::new()
                            .p_1()
                            .gap_0p5()
                            .children(filtered.into_iter().map(|w| self.render_window_row(w, cx))),
                    ),
            )
    }

    fn render_window_row(&self, w: &MockWindow, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_hwnd == Some(w.hwnd);
        let hwnd = w.hwnd;
        let title = w.title.clone();
        let process = w.process.clone();
        let resolution = w.resolution.clone();
        let thumb_tint = w.thumb_tint;
        let is_game = w.is_game;
        let is_focused = w.is_focused;

        div()
            .id(SharedString::from(format!("win-{}", w.hwnd)))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .bg(if selected { rgba(PRIMARY_DIM_A25) } else { rgba(0x00000000) })
            .hover(|s| s.bg(rgba(CARD_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.selected_hwnd = Some(hwnd);
                    cx.notify();
                }),
            )
            // Thumb
            .child(
                div()
                    .w(px(40.0))
                    .h(px(28.0))
                    .rounded_sm()
                    .bg(rgba(thumb_tint))
                    .relative()
                    .overflow_hidden()
                    .child(div().absolute().inset_0().bg(rgba(0x00000033))),
            )
            .child(
                VStack::new()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgba(FG))
                                    .child(title),
                            )
                            .when(is_focused, |this| {
                                this.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgba(0x22C55E30))
                                        .border_1()
                                        .border_color(rgba(SUCCESS))
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgba(SUCCESS))
                                        .child("Detected"),
                                )
                            })
                            .when(is_game && !is_focused, |this| {
                                this.child(
                                    Icon::new(IconSource::Named("gamepad-2".into()))
                                        .size(px(12.0))
                                        .color(rgba(FG_SUBTLE).into()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(format!("{} · {}", process, resolution)),
                    ),
            )
            .when(selected, |this| {
                this.child(
                    Icon::new(IconSource::Named("check".into()))
                        .size(px(16.0))
                        .color(rgba(PRIMARY).into()),
                )
            })
    }

    // ── Section 2: Details ──────────────────────────────────────────
    fn render_details_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sel = match self.selected() {
            Some(w) => w.clone(),
            None => {
                return VStack::new()
                    .gap_2()
                    .child(section_label("DETAILS"))
                    .child(
                        div()
                            .p_6()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgba(BORDER))
                            .bg(rgba(SURFACE))
                            .text_center()
                            .text_sm()
                            .text_color(rgba(FG_SUBTLE))
                            .child("Select a window above to preview."),
                    )
                    .into_any_element();
            }
        };

        VStack::new()
            .gap_2()
            .child(section_label("DETAILS"))
            .child(
                HStack::new()
                    .gap_4()
                    .items_center()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgba(BORDER))
                    .bg(rgba(SURFACE))
                    .child(
                        div()
                            .w(px(56.0))
                            .h(px(56.0))
                            .rounded_md()
                            .bg(rgba(sel.thumb_tint))
                            .relative()
                            .overflow_hidden()
                            .child(div().absolute().inset_0().bg(rgba(0x00000033))),
                    )
                    .child(
                        VStack::new()
                            .flex_1()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgba(FG_SUBTLE))
                                    .child("TITLE"),
                            )
                            .child(
                                Input::new(&self.title)
                                    .placeholder(SharedString::from(sel.title.clone())),
                            )
                            .child(
                                HStack::new()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child(sel.process.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child("·"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child(sel.resolution.clone()),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("artwork", "Artwork")
                            .icon(IconSource::Named("star".into()))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm),
                    ),
            )
            .into_any_element()
    }

    // ── Section 3: Settings ─────────────────────────────────────────
    fn render_settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = if self.show_overrides {
            self.summary_from_overrides()
        } else {
            "1920×1080 · 60 fps · HEVC · CQ 23 · 10 min".to_string()
        };

        let mut body = VStack::new()
            .gap_2()
            .child(
                HStack::new()
                    .items_center()
                    .child(section_label("SETTINGS"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(if self.show_overrides { "Overriding globals" } else { "Inheriting globals" }),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgba(BORDER))
                    .bg(rgba(SURFACE))
                    .child(
                        Icon::new(IconSource::Named(
                            if self.show_overrides { "sliders-horizontal" } else { "check-circle" }
                                .into(),
                        ))
                        .size(px(16.0))
                        .color(rgba(FG_MUTED).into()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgba(FG))
                            .child(summary),
                    )
                    .child(
                        Button::new(
                            "toggle-override",
                            if self.show_overrides { "Use defaults" } else { "Override defaults" },
                        )
                        .variant(if self.show_overrides { ButtonVariant::Ghost } else { ButtonVariant::Outline })
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_overrides = !this.show_overrides;
                            cx.notify();
                        })),
                    ),
            );

        if self.show_overrides {
            body = body.child(self.render_override_form(cx));
        }
        body
    }

    fn summary_from_overrides(&self) -> String {
        let quality = if self.rate_control == Rc::Cqp {
            format!("CQ {}", self.cq)
        } else {
            format!("{} kbps", self.bitrate)
        };
        format!(
            "{} · {} fps · {} · {} · {} min",
            self.resolution.replace('x', "×"),
            self.fps,
            self.encoder,
            quality,
            self.retention_minutes,
        )
    }

    fn render_override_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .p_4()
            .rounded_lg()
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(BORDER))
            .child(field_row(
                "Encoder",
                segmented_row_enc(cx, &self.encoder, &["HEVC", "AV1", "H.264"]),
            ))
            .child(field_row(
                "Resolution",
                segmented_row_res(
                    cx,
                    &self.resolution,
                    &["3840x2160", "2560x1440", "1920x1080", "1280x720"],
                ),
            ))
            .child(field_row(
                "Frame rate",
                segmented_row_fps(cx, self.fps, &[30, 60, 120, 144]),
            ))
            .child(field_row(
                "Rate control",
                HStack::new()
                    .gap_2()
                    .child(
                        Button::new("rc-cqp", "CQP")
                            .variant(if self.rate_control == Rc::Cqp {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.rate_control = Rc::Cqp;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("rc-vbr", "VBR")
                            .variant(if self.rate_control == Rc::Vbr {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.rate_control = Rc::Vbr;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            ))
            .child(
                if self.rate_control == Rc::Cqp {
                    field_row(
                        "Quality (CQ)",
                        stepper_row(cx, "cq", self.cq, 0, 51, 1, |this, v| this.cq = v),
                    )
                } else {
                    field_row(
                        "Bitrate",
                        stepper_row_with_suffix(
                            cx,
                            "br",
                            self.bitrate,
                            1000,
                            100_000,
                            1000,
                            "kbps",
                            |this, v| this.bitrate = v,
                        ),
                    )
                },
            )
            .child(field_row(
                "Retention",
                stepper_row_with_suffix(
                    cx,
                    "ret",
                    self.retention_minutes,
                    1,
                    600,
                    1,
                    "min",
                    |this, v| this.retention_minutes = v,
                ),
            ))
    }

    // ── Section 4: Audio routing ────────────────────────────────────
    fn render_audio_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_count = self.audio_tracks.iter().filter(|t| t.enabled).count();
        let total = self.audio_tracks.len();

        let mut list = VStack::new()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(BORDER));
        for (i, track) in self.audio_tracks.iter().enumerate() {
            list = list.child(self.render_audio_track_row(i, track, cx));
        }

        VStack::new()
            .gap_2()
            .child(
                HStack::new()
                    .items_center()
                    .child(section_label("AUDIO TRACKS"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(format!("{} of {} active", active_count, total)),
                    ),
            )
            .child(list)
    }

    fn render_audio_track_row(
        &self,
        idx: usize,
        track: &MockTrack,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = track.enabled;
        let track_name = track.name;
        let device = track.device.clone();
        let source = track.source;
        let app_count = track.app_count;

        HStack::new()
            .gap_3()
            .items_center()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgba(CARD))
            .border_1()
            .border_color(rgba(BORDER))
            // Enable toggle
            .child(
                div()
                    .id(SharedString::from(format!("at-en-{}", idx)))
                    .w(px(28.0))
                    .h(px(16.0))
                    .rounded_full()
                    .relative()
                    .cursor_pointer()
                    .bg(rgba(if enabled { PRIMARY } else { BORDER_STRONG }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.audio_tracks[idx].enabled = !this.audio_tracks[idx].enabled;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .left(if enabled { px(14.0) } else { px(2.0) })
                            .size(px(12.0))
                            .rounded_full()
                            .bg(rgba(FG)),
                    ),
            )
            .child(
                div()
                    .w(px(56.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if enabled { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(track_name),
            )
            // Source-type segmented mini control
            .child(track_source_pill(cx, idx, "sys", TrackSource::System, source))
            .child(track_source_pill(cx, idx, "mic", TrackSource::Mic, source))
            .child(track_source_pill(cx, idx, "app", TrackSource::App, source))
            .child(div().flex_1())
            // Right-side detail
            .child(match source {
                TrackSource::System | TrackSource::Mic => div()
                    .text_xs()
                    .text_color(rgba(FG_SUBTLE))
                    .child(if device.is_empty() { "Default".to_string() } else { device })
                    .into_any_element(),
                TrackSource::App => Button::new(
                    SharedString::from(format!("at-apps-{}", idx)),
                    if app_count == 0 {
                        "Configure apps".to_string()
                    } else {
                        format!("{} apps", app_count)
                    },
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .icon(IconSource::Named("chevron-right".into()))
                .into_any_element(),
            })
    }

    fn render_auto_record(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("auto-rec")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(rgba(BORDER))
            .bg(rgba(SURFACE))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.auto_record = !this.auto_record;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(36.0))
                    .h(px(20.0))
                    .rounded_full()
                    .relative()
                    .bg(rgba(if self.auto_record { PRIMARY } else { BORDER_STRONG }))
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .left(if self.auto_record { px(18.0) } else { px(2.0) })
                            .size(px(16.0))
                            .rounded_full()
                            .bg(rgba(FG)),
                    ),
            )
            .child(
                VStack::new()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Auto-record when detected"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child("Start the buffer automatically whenever this window becomes focused."),
                    ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_sel = self.selected_hwnd.is_some();
        HStack::new()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(FG_SUBTLE))
                    .child(if has_sel {
                        "Ready to add."
                    } else {
                        "Pick a window to continue."
                    }),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(
                        Button::new("cancel", "Cancel")
                            .variant(ButtonVariant::Ghost),
                    )
                    .child(
                        Button::new("add", "Add game source")
                            .icon(IconSource::Named("plus".into()))
                            .variant(ButtonVariant::Default),
                    ),
            )
    }
}

fn track_source_pill(
    cx: &mut Context<AddSourceMockup>,
    idx: usize,
    id_suffix: &'static str,
    source: TrackSource,
    current: TrackSource,
) -> impl IntoElement {
    let active = source == current;
    div()
        .id(SharedString::from(format!("ts-{}-{}", idx, id_suffix)))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .cursor_pointer()
        .bg(if active { rgba(PRIMARY_DIM_A25) } else { rgba(0x00000000) })
        .border_1()
        .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
        .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
        .hover(|s| s.text_color(rgba(FG)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.audio_tracks[idx].source = source;
                cx.notify();
            }),
        )
        .child(source.label())
}

// ── Shared primitives ───────────────────────────────────────────────
fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgba(FG_SUBTLE))
        .child(text.to_string())
}

fn field_row(label: &str, control: AnyElement) -> impl IntoElement {
    HStack::new()
        .gap_4()
        .items_center()
        .child(
            div()
                .w(px(120.0))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(FG_MUTED))
                .child(label.to_string()),
        )
        .child(div().flex_1().child(control))
}

// Encoder segmented (3 items, string-value)
fn segmented_row_enc(
    cx: &mut Context<AddSourceMockup>,
    current: &str,
    options: &[&'static str],
) -> AnyElement {
    let current = current.to_string();
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(rgba(CARD))
        .border_1()
        .border_color(rgba(BORDER))
        .p(px(2.0))
        .gap(px(2.0));
    for (i, opt) in options.iter().enumerate() {
        let active = *opt == current;
        let opt = opt.to_string();
        let opt_for_click = opt.clone();
        group = group.child(
            div()
                .id(SharedString::from(format!("enc-{}", i)))
                .px_3()
                .py_1()
                .rounded_sm()
                .text_xs()
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .bg(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
                .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                .hover(|s| s.text_color(rgba(FG)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.encoder = opt_for_click.clone();
                        cx.notify();
                    }),
                )
                .child(opt),
        );
    }
    group.into_any_element()
}

fn segmented_row_res(
    cx: &mut Context<AddSourceMockup>,
    current: &str,
    options: &[&'static str],
) -> AnyElement {
    let current = current.to_string();
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(rgba(CARD))
        .border_1()
        .border_color(rgba(BORDER))
        .p(px(2.0))
        .gap(px(2.0));
    for (i, opt) in options.iter().enumerate() {
        let active = *opt == current;
        let opt_owned = opt.to_string();
        let display = match *opt {
            "3840x2160" => "4K",
            "2560x1440" => "1440p",
            "1920x1080" => "1080p",
            "1280x720" => "720p",
            _ => opt,
        };
        group = group.child(
            div()
                .id(SharedString::from(format!("res-{}", i)))
                .px_3()
                .py_1()
                .rounded_sm()
                .text_xs()
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .bg(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
                .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                .hover(|s| s.text_color(rgba(FG)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.resolution = opt_owned.clone();
                        cx.notify();
                    }),
                )
                .child(display.to_string()),
        );
    }
    group.into_any_element()
}

fn segmented_row_fps(
    cx: &mut Context<AddSourceMockup>,
    current: u32,
    options: &[u32],
) -> AnyElement {
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(rgba(CARD))
        .border_1()
        .border_color(rgba(BORDER))
        .p(px(2.0))
        .gap(px(2.0));
    for (i, opt) in options.iter().copied().enumerate() {
        let active = opt == current;
        group = group.child(
            div()
                .id(SharedString::from(format!("fps-{}", i)))
                .px_3()
                .py_1()
                .rounded_sm()
                .text_xs()
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .bg(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
                .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                .hover(|s| s.text_color(rgba(FG)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.fps = opt;
                        cx.notify();
                    }),
                )
                .child(format!("{}", opt)),
        );
    }
    group.into_any_element()
}

fn stepper_row(
    cx: &mut Context<AddSourceMockup>,
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: impl Fn(&mut AddSourceMockup, i32) + 'static + Send + Sync + Clone,
) -> AnyElement {
    stepper_row_with_suffix(cx, id_prefix, value, min, max, step, "", on_change)
}

fn stepper_row_with_suffix(
    cx: &mut Context<AddSourceMockup>,
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    suffix: &'static str,
    on_change: impl Fn(&mut AddSourceMockup, i32) + 'static + Send + Sync + Clone,
) -> AnyElement {
    let on_dec = on_change.clone();
    let on_inc = on_change;
    HStack::new()
        .gap_2()
        .items_center()
        .child(
            Button::new(SharedString::from(format!("{}-dec", id_prefix)), "-")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_dec(this, (value - step).max(min));
                    cx.notify();
                })),
        )
        .child(
            div()
                .min_w(px(72.0))
                .text_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(if suffix.is_empty() {
                    format!("{}", value)
                } else {
                    format!("{} {}", value, suffix)
                }),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", id_prefix)), "+")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_inc(this, (value + step).min(max));
                    cx.notify();
                })),
        )
        .into_any_element()
}

// ── main ────────────────────────────────────────────────────────────
fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1100.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Add Source Redesign Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(AddSourceMockup::new),
        )
        .unwrap();
    });
}
