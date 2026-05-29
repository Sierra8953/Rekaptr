// Clip-export dialog redesign mockup.
//
// Direction: the current export modal is a narrow 400px stack of radios and
// steppers. Redesign as a wider, preview-driven dialog that answers the three
// questions a user actually has in order:
//   1. What am I exporting? (thumbnail, title, range, estimated size)
//   2. How? (Instant Copy vs Re-encode — mode cards, not radios)
//   3. Where? (destination folder)
// Adds a dedicated progress state with ETA + cancel, and a success state with
// "open folder" action. A state toggle at the top lets you
// preview all three.
//
// Self-contained: no real file I/O, no ffmpeg. All data is mocked.

use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
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

// ── Theme ───────────────────────────────────────────────────────────
const BG: u32 = 0x09090BFF;
const SURFACE: u32 = 0x121215FF;
const CARD: u32 = 0x18181BFF;
const BORDER: u32 = 0x2A2A30FF;
const BORDER_STRONG: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const PRIMARY_DIM_A25: u32 = 0x5B3FA840;
const SUCCESS: u32 = 0x22C55EFF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const FG_SUBTLE: u32 = 0x71717AFF;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Configure,
    Exporting,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Instant,
    Reencode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoder {
    Hevc,
    Av1,
    H264,
}

impl Encoder {
    fn label(self) -> &'static str {
        match self {
            Encoder::Hevc => "HEVC",
            Encoder::Av1 => "AV1",
            Encoder::H264 => "H.264",
        }
    }
    fn detail(self) -> &'static str {
        match self {
            Encoder::Hevc => "Best quality per bit. Plays on most devices.",
            Encoder::Av1 => "Smallest files. Newer players only.",
            Encoder::H264 => "Maximum compatibility. Larger files.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quality {
    Small,
    Balanced,
    Max,
}

impl Quality {
    fn label(self) -> &'static str {
        match self {
            Quality::Small => "Smaller",
            Quality::Balanced => "Balanced",
            Quality::Max => "Max quality",
        }
    }
    fn bitrate_kbps(self) -> u32 {
        match self {
            Quality::Small => 8000,
            Quality::Balanced => 20000,
            Quality::Max => 50000,
        }
    }
}

struct ExportMockup {
    stage: Stage,
    progress: f32,

    // Clip info
    title: Entity<InputState>,
    game: String,
    clip_start: f32,
    clip_end: f32,

    // Export config
    mode: Mode,
    encoder: Encoder,
    quality: Quality,
    container: String,
    include_mic: bool,
    include_system: bool,

    // Destination
    destination: String,
}

impl ExportMockup {
    fn new(cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| InputState::new(cx));
        Self {
            stage: Stage::Configure,
            progress: 0.0,
            title,
            game: "Counter-Strike 2".into(),
            clip_start: 42.0,
            clip_end: 86.5,
            mode: Mode::Instant,
            encoder: Encoder::Hevc,
            quality: Quality::Balanced,
            container: "mp4".into(),
            include_mic: true,
            include_system: true,
            destination: "C:\\Users\\you\\Videos\\Rekaptr\\Clips\\Counter-Strike 2".into(),
        }
    }

    fn duration(&self) -> f32 {
        (self.clip_end - self.clip_start).max(0.0)
    }

    fn estimated_size_mb(&self) -> f32 {
        match self.mode {
            Mode::Instant => self.duration() * 18.0 / 8.0, // pretend source is ~18 Mbps
            Mode::Reencode => self.duration() * self.quality.bitrate_kbps() as f32 / 8000.0,
        }
    }
}

// ── Render ──────────────────────────────────────────────────────────
impl Render for ExportMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .relative()
            .child(self.render_fake_underlay())
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
            .child(self.render_stage_toggle(cx))
    }
}

impl ExportMockup {
    // Soft blurred-looking fill behind the overlay, so the dialog reads as a modal.
    fn render_fake_underlay(&self) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(rgba(SURFACE))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(rgba(FG_SUBTLE))
                    .text_sm()
                    .child("(app behind the modal)"),
            )
    }

    // Top-right preview toggle so you can step through the three stages.
    fn render_stage_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let cur = self.stage;
        let chip = |id: &'static str, label: &'static str, stage: Stage, cur: Stage, cx: &mut Context<Self>| {
            let active = stage == cur;
            div()
                .id(id)
                .px_3()
                .py_1()
                .text_xs()
                .rounded_sm()
                .cursor_pointer()
                .bg(if active { rgba(PRIMARY) } else { rgba(0x00000000) })
                .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .hover(|s| s.text_color(rgba(FG)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.stage = stage;
                        if stage == Stage::Exporting { this.progress = 0.38; }
                        cx.notify();
                    }),
                )
                .child(label)
        };
        div()
            .absolute()
            .top(px(16.0))
            .right(px(16.0))
            .flex()
            .flex_row()
            .p(px(2.0))
            .gap(px(2.0))
            .rounded_md()
            .bg(rgba(CARD))
            .border_1()
            .border_color(rgba(BORDER))
            .child(chip("st-cfg", "Configure", Stage::Configure, cur, cx))
            .child(chip("st-exp", "Exporting", Stage::Exporting, cur, cx))
            .child(chip("st-done", "Done", Stage::Done, cur, cx))
    }

    fn render_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(720.0))
            .max_h(px(820.0))
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
            .child(match self.stage {
                Stage::Configure => self.render_configure(cx).into_any_element(),
                Stage::Exporting => self.render_exporting(cx).into_any_element(),
                Stage::Done => self.render_done(cx).into_any_element(),
            })
    }

    // ── Header (shared) ─────────────────────────────────────────────
    fn render_header(&self) -> impl IntoElement {
        let label = match self.stage {
            Stage::Configure => "Export clip",
            Stage::Exporting => "Exporting…",
            Stage::Done => "Clip exported",
        };
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
                                Icon::new(IconSource::Named(
                                    match self.stage {
                                        Stage::Configure => "scissors",
                                        Stage::Exporting => "loader-2",
                                        Stage::Done => "check",
                                    }
                                    .into(),
                                ))
                                .size(px(16.0))
                                .color(rgba(PRIMARY).into()),
                            ),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label),
                    ),
            )
            .child(
                Button::new("modal-close", "")
                    .icon(IconSource::Named("x".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
    }

    // ── Stage: Configure ────────────────────────────────────────────
    fn render_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .flex_1()
            .child(
                div()
                    .id("cfg-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .p_6()
                            .gap_6()
                            .child(self.render_clip_preview())
                            .child(self.render_mode_cards(cx))
                            .when(self.mode == Mode::Reencode, |this| {
                                this.child(self.render_reencode_panel(cx))
                            })
                            .child(self.render_audio_panel(cx))
                            .child(self.render_destination(cx)),
                    ),
            )
            .child(self.render_footer(cx))
    }

    fn render_clip_preview(&self) -> impl IntoElement {
        HStack::new()
            .gap_4()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgba(BORDER))
            .bg(rgba(SURFACE))
            .child(
                // Fake thumbnail with gradient + duration badge.
                div()
                    .w(px(200.0))
                    .h(px(112.0))
                    .rounded_md()
                    .bg(rgba(0x6E7F66FF))
                    .relative()
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(rgba(0x00000033)),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .size(px(40.0))
                                    .rounded_full()
                                    .bg(rgba(0x00000099))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named("play".into()))
                                            .size(px(18.0))
                                            .color(rgba(FG).into()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(6.0))
                            .right(px(6.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgba(0x000000CC))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .child(fmt_duration(self.duration())),
                    ),
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
                            .child("CLIP TITLE"),
                    )
                    .child(Input::new(&self.title).placeholder("Clutch 1v4 on Mirage"))
                    .child(
                        HStack::new()
                            .pt_2()
                            .gap_4()
                            .child(kv("Source", &self.game))
                            .child(kv("Range", &format!(
                                "{} → {}",
                                fmt_clock(self.clip_start),
                                fmt_clock(self.clip_end),
                            )))
                            .child(kv("Estimated size", &format!("{:.1} MB", self.estimated_size_mb()))),
                    ),
            )
    }

    fn render_mode_cards(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_2()
            .child(section_label("MODE"))
            .child(
                HStack::new()
                    .gap_2()
                    .child(
                        Button::new("mc-instant", "Instant copy")
                            .variant(if self.mode == Mode::Instant {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::Instant;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mc-reencode", "Re-encode")
                            .variant(if self.mode == Mode::Reencode {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = Mode::Reencode;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_reencode_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_5()
            .p_5()
            .rounded_lg()
            .bg(rgba(SURFACE))
            .border_1()
            .border_color(rgba(BORDER))
            .child(
                VStack::new()
                    .gap_2()
                    .child(section_label("ENCODER"))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.enc_chip(Encoder::Hevc, cx))
                            .child(self.enc_chip(Encoder::Av1, cx))
                            .child(self.enc_chip(Encoder::H264, cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(self.encoder.detail()),
                    ),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_end()
                            .child(section_label("QUALITY"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_MUTED))
                                    .child(format!("{} kbps", self.quality.bitrate_kbps())),
                            ),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.qty_chip(Quality::Small, cx))
                            .child(self.qty_chip(Quality::Balanced, cx))
                            .child(self.qty_chip(Quality::Max, cx)),
                    )
                    .child(quality_slider_mock(self.quality)),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(section_label("CONTAINER"))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.container_chip("mp4", cx))
                            .child(self.container_chip("mov", cx))
                            .child(self.container_chip("mkv", cx))
                            .child(self.container_chip("webm", cx)),
                    ),
            )
    }

    fn enc_chip(&self, enc: Encoder, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.encoder == enc;
        div()
            .id(SharedString::from(format!("enc-{}", enc.label())))
            .px_4()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
            .bg(if active { rgba(PRIMARY_DIM_A25) } else { rgba(CARD) })
            .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
            .font_weight(FontWeight::SEMIBOLD)
            .text_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(BORDER_STRONG)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.encoder = enc;
                    cx.notify();
                }),
            )
            .child(enc.label())
    }

    fn qty_chip(&self, q: Quality, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.quality == q;
        div()
            .id(SharedString::from(format!("q-{}", q.label())))
            .flex_1()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
            .bg(if active { rgba(PRIMARY_DIM_A25) } else { rgba(CARD) })
            .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
            .text_center()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .text_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(BORDER_STRONG)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.quality = q;
                    cx.notify();
                }),
            )
            .child(q.label())
    }

    fn container_chip(&self, ext: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.container == ext;
        div()
            .id(SharedString::from(format!("ct-{}", ext)))
            .px_3()
            .py_1()
            .rounded_sm()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .bg(if active { rgba(PRIMARY) } else { rgba(CARD) })
            .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
            .border_1()
            .border_color(if active { rgba(PRIMARY) } else { rgba(BORDER) })
            .cursor_pointer()
            .hover(|s| s.text_color(rgba(FG)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.container = ext.to_string();
                    cx.notify();
                }),
            )
            .child(ext.to_uppercase())
    }

    fn render_audio_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_2()
            .child(section_label("AUDIO TRACKS"))
            .child(
                HStack::new()
                    .gap_5()
                    .child(audio_track_row(
                        cx,
                        "at-sys",
                        "volume-2",
                        "System",
                        "Game + Discord",
                        self.include_system,
                        Arc::new(|this: &mut ExportMockup, cx| {
                            this.include_system = !this.include_system;
                            cx.notify();
                        }),
                    ))
                    .child(audio_track_row(
                        cx,
                        "at-mic",
                        "mic",
                        "Microphone",
                        "Shure MV7",
                        self.include_mic,
                        Arc::new(|this: &mut ExportMockup, cx| {
                            this.include_mic = !this.include_mic;
                            cx.notify();
                        }),
                    )),
            )
    }

    fn render_destination(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_2()
            .child(section_label("DESTINATION"))
            .child(
                HStack::new()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .bg(rgba(SURFACE))
                    .border_1()
                    .border_color(rgba(BORDER))
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("folder".into()))
                            .size(px(16.0))
                            .color(rgba(FG_MUTED).into()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(rgba(FG_MUTED))
                            .child(self.destination.clone()),
                    )
                    .child(
                        Button::new("pick-dest", "Change")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgba(BORDER))
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_4()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("hard-drive".into()))
                            .size(px(14.0))
                            .color(rgba(FG_SUBTLE).into()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_MUTED))
                            .child(format!(
                                "{} · {:.1} MB estimated",
                                fmt_duration(self.duration()),
                                self.estimated_size_mb(),
                            )),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(
                        Button::new("cancel", "Cancel")
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    )
                    .child(
                        Button::new("export", "Export clip")
                            .icon(IconSource::Named("download".into()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stage = Stage::Exporting;
                                this.progress = 0.0;
                                cx.notify();
                            })),
                    ),
            )
    }

    // ── Stage: Exporting ────────────────────────────────────────────
    fn render_exporting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pct = (self.progress * 100.0).round() as i32;
        VStack::new()
            .flex_1()
            .p_8()
            .gap_6()
            .items_center()
            .child(
                div()
                    .pt_4()
                    .child(
                        div()
                            .size(px(72.0))
                            .rounded_full()
                            .bg(rgba(PRIMARY_DIM_A25))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Spinner::new().size(SpinnerSize::Xl)),
                    ),
            )
            .child(
                VStack::new()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Rendering your clip"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(match self.mode {
                                Mode::Instant => "Stream-copying segments. Just a moment…",
                                Mode::Reencode => "Re-encoding with NVENC. About 14s remaining.",
                            }),
                    ),
            )
            .child(
                VStack::new()
                    .w(px(480.0))
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .h(px(10.0))
                            .rounded_full()
                            .bg(rgba(SURFACE))
                            .border_1()
                            .border_color(rgba(BORDER))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(self.progress))
                                    .bg(rgba(PRIMARY))
                                    .rounded_full(),
                            ),
                    )
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_MUTED))
                                    .child(format!("{}%", pct)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child(format!("{} / {}", fmt_duration(self.duration() * self.progress), fmt_duration(self.duration()))),
                            ),
                    ),
            )
            .child(
                HStack::new()
                    .pt_4()
                    .gap_3()
                    .items_center()
                    .child(Button::new("cancel-exp", "Cancel").variant(ButtonVariant::Outline)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.stage = Stage::Configure; cx.notify();
                        })))
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconSource::Named("minimize-2".into()))
                                    .size(px(14.0))
                                    .color(rgba(FG_SUBTLE).into()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child("You can close this — the export keeps running in the background."),
                            ),
                    ),
            )
    }

    // ── Stage: Done ─────────────────────────────────────────────────
    fn render_done(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filename = format!("clutch-1v4-on-mirage.{}", self.container);
        VStack::new()
            .flex_1()
            .p_8()
            .gap_6()
            .items_center()
            .child(
                div()
                    .size(px(72.0))
                    .rounded_full()
                    .bg(rgba(0x22C55E40))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named("check".into()))
                            .size(px(32.0))
                            .color(rgba(SUCCESS).into()),
                    ),
            )
            .child(
                VStack::new()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Clip saved"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(format!(
                                "{} · {:.1} MB",
                                fmt_duration(self.duration()),
                                self.estimated_size_mb()
                            )),
                    ),
            )
            .child(
                div()
                    .w(px(520.0))
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(rgba(SURFACE))
                    .border_1()
                    .border_color(rgba(BORDER))
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                Icon::new(IconSource::Named("file-video".into()))
                                    .size(px(18.0))
                                    .color(rgba(FG_MUTED).into()),
                            )
                            .child(
                                VStack::new()
                                    .flex_1()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(filename),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child(self.destination.clone()),
                                    ),
                            )
                            .child(
                                Button::new("copy-path", "")
                                    .icon(IconSource::Named("copy".into()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm),
                            ),
                    ),
            )
            .child(
                HStack::new()
                    .pt_2()
                    .gap_3()
                    .child(
                        Button::new("done-reveal", "Show in folder")
                            .icon(IconSource::Named("folder-open".into()))
                            .variant(ButtonVariant::Outline),
                    )
                    .child(
                        Button::new("done-close", "Done")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stage = Stage::Configure;
                                cx.notify();
                            })),
                    ),
            )
    }
}

// ── Small shared pieces ─────────────────────────────────────────────
fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgba(FG_SUBTLE))
        .child(text.to_string())
}

fn kv(label: &str, value: &str) -> impl IntoElement {
    VStack::new()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .text_color(rgba(FG_SUBTLE))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(FG))
                .child(value.to_string()),
        )
}

fn fmt_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn fmt_clock(secs: f32) -> String {
    let total = secs.max(0.0) as u32;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 { format!("{}:{:02}:{:02}", h, m, s) } else { format!("{}:{:02}", m, s) }
}

fn quality_slider_mock(q: Quality) -> impl IntoElement {
    let position = match q {
        Quality::Small => 0.15,
        Quality::Balanced => 0.5,
        Quality::Max => 0.88,
    };
    div()
        .w_full()
        .h(px(6.0))
        .rounded_full()
        .bg(rgba(SURFACE))
        .border_1()
        .border_color(rgba(BORDER))
        .relative()
        .child(
            div()
                .absolute()
                .inset_0()
                .h_full()
                .w(relative(position))
                .bg(rgba(PRIMARY))
                .rounded_full(),
        )
        .child(
            div()
                .absolute()
                .top(px(-4.0))
                .left(relative(position))
                .size(px(14.0))
                .rounded_full()
                .bg(rgba(FG))
                .border_2()
                .border_color(rgba(PRIMARY)),
        )
}

fn audio_track_row(
    cx: &mut Context<ExportMockup>,
    id: &'static str,
    icon: &'static str,
    name: &str,
    detail: &str,
    enabled: bool,
    on_toggle: Arc<dyn Fn(&mut ExportMockup, &mut Context<ExportMockup>) + Send + Sync + 'static>,
) -> impl IntoElement {
    let name = name.to_string();
    let detail = detail.to_string();

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .py_1()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                on_toggle(this, cx);
            }),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(16.0))
                .rounded_full()
                .relative()
                .bg(if enabled { rgba(PRIMARY) } else { rgba(BORDER_STRONG) })
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
            Icon::new(IconSource::Named(icon.into()))
                .size(px(14.0))
                .color(if enabled { rgba(FG_MUTED).into() } else { rgba(FG_SUBTLE).into() }),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(if enabled { rgba(FG) } else { rgba(FG_MUTED) })
                .child(name),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgba(FG_SUBTLE))
                .child(detail),
        )
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
                    title: Some("Export Redesign Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ExportMockup::new),
        )
        .unwrap();
    });
}
