// Redesign of the per-source "Source Settings" popup (the modal that opens
// from the gear button on an already-added game source — currently
// `render_advanced_settings_dialog`).
//
// Same scope as today: a centered modal over a dimmed backdrop, Video / Audio
// / Advanced tabs, and a Delete / Cancel / Save footer. This is purely a visual
// refresh — cleaner header with source identity, segmented pickers instead of
// button rows, real toggle switches, and tidier steppers.
//
// Self-contained: no real config I/O. All data is mocked.

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

// ── Theme (matches settings_redesign_mockup.rs) ─────────────────────
const BG: u32 = 0x09090BFF;
const SURFACE: u32 = 0x121215FF;
const CARD: u32 = 0x18181BFF;
const CARD_HOVER: u32 = 0x22222AFF;
const BORDER: u32 = 0x2A2A30FF;
const BORDER_STRONG: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const PRIMARY_DIM_A25: u32 = 0x5B3FA840;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const FG_SUBTLE: u32 = 0x71717AFF;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Video,
    Audio,
    Advanced,
}

#[derive(Clone)]
struct Track {
    name: String,
    source: String, // System | Mic | App
    detail: String,
    enabled: bool,
}

// ── Mock popup state ────────────────────────────────────────────────
struct SourceSettingsMockup {
    source_name: String,
    accent: u32,
    glyph: &'static str,
    tab: Tab,

    // Video
    encoder: String, // HEVC | AV1 | H.264
    rate_control: u32, // 0 = CQP, 1 = VBR
    resolution: String,
    fps: u32,
    cq: i32,
    bitrate: i32,
    retention: i32,

    // Audio
    tracks: Vec<Track>,

    // Advanced
    auto_record: bool,
    preset: String,
    zero_latency: bool,
    lookahead: bool,
    spatial_aq: bool,
    temporal_aq: bool,
    gop: i32,
}

impl SourceSettingsMockup {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            source_name: "Counter-Strike 2".into(),
            accent: 0xF59E0BFF,
            glyph: "crosshair",
            tab: Tab::Video,
            encoder: "HEVC".into(),
            rate_control: 0,
            resolution: "1920x1080".into(),
            fps: 60,
            cq: 21,
            bitrate: 24000,
            retention: 10,
            tracks: vec![
                Track { name: "Game".into(), source: "App".into(), detail: "cs2.exe".into(), enabled: true },
                Track { name: "Mic".into(), source: "Mic".into(), detail: "Shure MV7".into(), enabled: true },
                Track { name: "Desktop".into(), source: "System".into(), detail: "Speakers (Realtek)".into(), enabled: false },
            ],
            auto_record: true,
            preset: "p5".into(),
            zero_latency: false,
            lookahead: true,
            spatial_aq: true,
            temporal_aq: true,
            gop: 60,
        }
    }
}

// ── Render: backdrop + modal ────────────────────────────────────────
impl Render for SourceSettingsMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Dimmed app backdrop so the modal reads in context.
        div()
            .id("backdrop")
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000_cc)),
            )
            .child(
                div()
                    .relative()
                    .w(px(680.0))
                    .max_h(relative(0.9))
                    .bg(rgba(CARD))
                    .border_1()
                    .border_color(rgba(BORDER))
                    .rounded_2xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_header(cx))
                    .child(self.render_tabs(cx))
                    .child(
                        div()
                            .id("tab-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_6()
                            .py_5()
                            .child(self.render_body(cx)),
                    )
                    .child(self.render_footer(cx)),
            )
    }
}

impl SourceSettingsMockup {
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .px_6()
            .py_5()
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
                            .size(px(44.0))
                            .rounded_xl()
                            .bg(rgba(self.accent & 0xFFFFFF66))
                            .border_1()
                            .border_color(rgba(self.accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named(self.glyph.into()))
                                    .size(px(22.0))
                                    .color(rgba(self.accent).into()),
                            ),
                    )
                    .child(
                        VStack::new()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.source_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_MUTED))
                                    .child("Source settings"),
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

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .px_6()
            .pt_4()
            .gap_1()
            .child(self.tab_btn(Tab::Video, "Video", "video", cx))
            .child(self.tab_btn(Tab::Audio, "Audio", "volume-2", cx))
            .child(self.tab_btn(Tab::Advanced, "Advanced", "sliders-horizontal", cx))
    }

    fn tab_btn(&self, tab: Tab, label: &'static str, icon: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.tab == tab;
        div()
            .id(SharedString::from(format!("tab-{}", label)))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_t_lg()
            .cursor_pointer()
            .bg(if active { rgba(CARD_HOVER) } else { rgba(0x00000000) })
            .border_b_2()
            .border_color(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.tab = tab;
                    cx.notify();
                }),
            )
            .child(
                Icon::new(IconSource::Named(icon.into()))
                    .size(px(15.0))
                    .color(if active { rgba(PRIMARY).into() } else { rgba(FG_MUTED).into() }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(label),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.tab {
            Tab::Video => self.body_video(cx).into_any_element(),
            Tab::Audio => self.body_audio(cx).into_any_element(),
            Tab::Advanced => self.body_advanced(cx).into_any_element(),
        }
    }

    fn body_video(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut quality = VStack::new().child(segmented_row(
            cx, "rc", "Rate control",
            if self.rate_control == 0 { "CQP" } else { "VBR" },
            &["CQP", "VBR"],
            |this, v| this.rate_control = if v == "VBR" { 1 } else { 0 },
        ));
        if self.rate_control == 0 {
            quality = quality.child(stepper_row(cx, "cq", "Quality (CQ)",
                Some("Lower is sharper. 18–24 is the sweet spot."),
                self.cq, 0, 51, 1, |this, v| this.cq = v));
        } else {
            quality = quality.child(stepper_row(cx, "br", "Bitrate (kbps)", None,
                self.bitrate, 1000, 100_000, 1000, |this, v| this.bitrate = v));
        }

        VStack::new()
            .gap_5()
            .child(card("Output", VStack::new()
                .child(encoder_row(cx, &self.encoder))
                .child(segmented_row(cx, "res", "Resolution", &self.resolution,
                    &["3840x2160", "2560x1440", "1920x1080", "1280x720"],
                    |this, v| this.resolution = v))
                .child(segmented_row(cx, "fps", "Frame rate", &format!("{}", self.fps),
                    &["30", "60", "120", "144"],
                    |this, v| { if let Ok(n) = v.parse() { this.fps = n; } }))))
            .child(card("Quality", quality))
            .child(card("Replay buffer", VStack::new().child(stepper_row(
                cx, "ret", "Retention", Some("minutes kept in memory"),
                self.retention, 1, 600, 1, |this, v| this.retention = v))))
    }

    fn body_audio(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = VStack::new().gap_2();
        for (i, t) in self.tracks.iter().enumerate() {
            list = list.child(self.track_row(i, t, cx));
        }
        card("Audio tracks", list)
    }

    fn track_row(&self, idx: usize, t: &Track, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = t.enabled;
        HStack::new()
            .gap_3()
            .items_center()
            .px_3()
            .py_2p5()
            .rounded_lg()
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(BORDER))
            .child(mini_toggle(cx, idx, enabled))
            .child(
                div()
                    .w(px(64.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if enabled { rgba(FG) } else { rgba(FG_SUBTLE) })
                    .child(t.name.clone()),
            )
            .child(track_pill(cx, idx, "sys", "System", &t.source))
            .child(track_pill(cx, idx, "mic", "Mic", &t.source))
            .child(track_pill(cx, idx, "app", "App", &t.source))
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(rgba(FG_SUBTLE))
                    .text_right()
                    .child(t.detail.clone()),
            )
    }

    fn body_advanced(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_5()
            .child(card("Automation", VStack::new().child(toggle_row(
                cx, "auto", "Auto-record when detected",
                Some("Start the buffer automatically when this game is focused."),
                self.auto_record, |this| this.auto_record = !this.auto_record))))
            .child(card("Encoder preset", VStack::new().child(segmented_row(
                cx, "pre", "Preset", &self.preset.to_uppercase(),
                &["P1", "P4", "P5", "P7"],
                |this, v| this.preset = v.to_lowercase()))))
            .child(card("Quality suite", VStack::new()
                .child(toggle_row(cx, "zl", "Zero latency",
                    Some("Disables B-frames; minimizes latency."),
                    self.zero_latency, |this| this.zero_latency = !this.zero_latency))
                .child(toggle_row(cx, "la", "Lookahead",
                    Some("Better compression at a small latency cost."),
                    self.lookahead, |this| this.lookahead = !this.lookahead))
                .child(toggle_row(cx, "saq", "Spatial AQ",
                    Some("Redistributes bitrate to low-detail areas."),
                    self.spatial_aq, |this| this.spatial_aq = !this.spatial_aq))
                .child(toggle_row(cx, "taq", "Temporal AQ",
                    Some("Improves quality in complex motion."),
                    self.temporal_aq, |this| this.temporal_aq = !this.temporal_aq))))
            .child(card("Keyframes", VStack::new().child(stepper_row(
                cx, "gop", "GOP size", Some("Keyframe interval in frames."),
                self.gop, 1, 1000, 10, |this, v| this.gop = v))))
    }

    fn render_footer(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                Button::new("delete", "Delete source")
                    .icon(IconSource::Named("trash-2".into()))
                    .variant(ButtonVariant::Destructive)
                    .size(ButtonSize::Sm),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(Button::new("cancel", "Cancel").variant(ButtonVariant::Ghost))
                    .child(
                        Button::new("save", "Save changes")
                            .icon(IconSource::Named("check".into())),
                    ),
            )
    }
}

// ── Generic card ────────────────────────────────────────────────────
fn card(title: &str, body: impl IntoElement) -> impl IntoElement {
    VStack::new()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(rgba(BORDER))
        .bg(rgba(SURFACE))
        .child(
            div()
                .px_5()
                .pt_4()
                .pb_2()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba(FG_MUTED))
                .child(title.to_string()),
        )
        .child(div().px_5().pb_4().child(body))
}

fn row(label: &str, description: Option<&str>, control: AnyElement) -> impl IntoElement {
    let desc_owned: Option<SharedString> = description.map(|d| SharedString::from(d.to_string()));
    let mut left = VStack::new().flex_1().gap_0p5().child(
        div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgba(FG))
            .child(label.to_string()),
    );
    if let Some(d) = desc_owned {
        left = left.child(div().text_xs().text_color(rgba(FG_SUBTLE)).child(d));
    }
    HStack::new()
        .w_full()
        .py_2p5()
        .gap_4()
        .items_center()
        .justify_between()
        .child(left)
        .child(div().child(control))
}

// ── Controls ────────────────────────────────────────────────────────
fn toggle_row(
    cx: &mut Context<SourceSettingsMockup>,
    id: &'static str,
    label: &str,
    description: Option<&str>,
    value: bool,
    on_toggle: impl Fn(&mut SourceSettingsMockup) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_toggle = Arc::new(on_toggle);
    let sw = div()
        .id(id)
        .w(px(40.0))
        .h(px(22.0))
        .rounded_full()
        .relative()
        .cursor_pointer()
        .bg(if value { rgba(PRIMARY) } else { rgba(BORDER_STRONG) })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            on_toggle(this);
            cx.notify();
        }))
        .child(
            div()
                .absolute()
                .top(px(2.0))
                .left(if value { px(20.0) } else { px(2.0) })
                .size(px(18.0))
                .rounded_full()
                .bg(rgba(FG)),
        );
    row(label, description, sw.into_any_element())
}

fn segmented_row(
    cx: &mut Context<SourceSettingsMockup>,
    id_prefix: &'static str,
    label: &str,
    current: &str,
    options: &[&'static str],
    on_pick: impl Fn(&mut SourceSettingsMockup, String) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_pick = Arc::new(on_pick);
    let current_owned = current.to_string();
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(rgba(BG))
        .border_1()
        .border_color(rgba(BORDER))
        .p(px(2.0))
        .gap(px(2.0));
    for (i, opt) in options.iter().enumerate() {
        let active = *opt == current_owned;
        let opt_string = opt.to_string();
        let on_pick = on_pick.clone();
        group = group.child(
            div()
                .id(SharedString::from(format!("{}-{}", id_prefix, i)))
                .px_3()
                .py_1()
                .rounded_sm()
                .text_xs()
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .bg(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
                .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                .hover(|s| s.text_color(rgba(FG)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    on_pick(this, opt_string.clone());
                    cx.notify();
                }))
                .child(opt.to_string()),
        );
    }
    row(label, None, group.into_any_element())
}

fn stepper_row(
    cx: &mut Context<SourceSettingsMockup>,
    id_prefix: &'static str,
    label: &str,
    description: Option<&str>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: impl Fn(&mut SourceSettingsMockup, i32) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_change = Arc::new(on_change);
    let on_dec = on_change.clone();
    let on_inc = on_change;
    let ctl = HStack::new()
        .gap_2()
        .items_center()
        .child(
            Button::new(SharedString::from(format!("{}-dec", id_prefix)), "")
                .icon(IconSource::Named("minus".into()))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
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
                .child(format!("{}", value)),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", id_prefix)), "")
                .icon(IconSource::Named("plus".into()))
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_inc(this, (value + step).min(max));
                    cx.notify();
                })),
        );
    row(label, description, ctl.into_any_element())
}

fn encoder_row(cx: &mut Context<SourceSettingsMockup>, current: &str) -> impl IntoElement {
    let encoders = ["HEVC", "AV1", "H.264"];
    let buttons = HStack::new().gap_2().children(encoders.iter().map(|name| {
        let active = *name == current;
        let name_str = name.to_string();
        Button::new(SharedString::from(format!("enc-{}", name)), *name)
            .variant(if active { ButtonVariant::Default } else { ButtonVariant::Outline })
            .size(ButtonSize::Sm)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.encoder = name_str.clone();
                cx.notify();
            }))
    }));
    row("Encoder", None, buttons.into_any_element())
}

fn mini_toggle(cx: &mut Context<SourceSettingsMockup>, idx: usize, enabled: bool) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("trk-{}", idx)))
        .w(px(28.0))
        .h(px(16.0))
        .rounded_full()
        .relative()
        .cursor_pointer()
        .bg(if enabled { rgba(PRIMARY) } else { rgba(BORDER_STRONG) })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.tracks[idx].enabled = !this.tracks[idx].enabled;
            cx.notify();
        }))
        .child(
            div()
                .absolute()
                .top(px(2.0))
                .left(if enabled { px(14.0) } else { px(2.0) })
                .size(px(12.0))
                .rounded_full()
                .bg(rgba(FG)),
        )
}

fn track_pill(
    cx: &mut Context<SourceSettingsMockup>,
    idx: usize,
    id_suffix: &'static str,
    label: &'static str,
    current: &str,
) -> impl IntoElement {
    let active = current == label;
    div()
        .id(SharedString::from(format!("tp-{}-{}", idx, id_suffix)))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .cursor_pointer()
        .bg(if active { rgba(PRIMARY_DIM_A25) } else { rgba(CARD) })
        .border_1()
        .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
        .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
        .hover(|s| s.text_color(rgba(FG)))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.tracks[idx].source = label.to_string();
            cx.notify();
        }))
        .child(label)
}

// ── main ────────────────────────────────────────────────────────────
fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1100.0), px(820.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Source Settings Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(SourceSettingsMockup::new),
        )
        .unwrap();
    });
}
