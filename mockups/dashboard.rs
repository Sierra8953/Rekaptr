//! Main dashboard mockup, built with the `rsx!` macro (`crates/rekaptr-rsx`).
//!
//! A static visual approximation of `src/ui/dashboard.rs`: video preview with a
//! recording overlay, transport + clip controls, a timeline, and the
//! "Recent Sessions" card gallery. No real state, video, or theme — placeholder
//! data and a local palette only. Real SVG icons are loaded from `assets/`.
//!
//! Run with:  cargo run --example dashboard_mockup

use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;
use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, relative, rgb, rgba, size, svg,
    AnyElement, App, Application, AssetSource, Background, Bounds, Context, FontWeight, Pixels,
    Render, SharedString, Window, WindowBounds, WindowOptions,
};
use rekaptr_rsx::rsx;

// ── palette ──────────────────────────────────────────────────────────────────
const BG_TOP: u32 = 0x0d0d10;
const BG_BOT: u32 = 0x070708;
const PANEL: u32 = 0x141417;
const CARD: u32 = 0x161619;
const CARD_HI: u32 = 0x1d1d22; // hover
const WELL: u32 = 0x0e0e11; // inset surfaces
const BORDER: u32 = 0x2a2a30;
const BORDER_HI: u32 = 0x3a3a44;
const HAIRLINE: u32 = 0xffffff_0d; // 1px white overlay border
const MUTED: u32 = 0x8b8b95;
const SUBTLE: u32 = 0x5c5c66;
const FG: u32 = 0xd4d4d8;
const FG_HI: u32 = 0xfafafa;
const PRIMARY: u32 = 0x8b5cf6;
const REC: u32 = 0xef4444;

/// A linear gradient between two packed `0xRRGGBBAA` colors.
fn lin(angle: f32, from: u32, to: u32) -> Background {
    linear_gradient(
        angle,
        linear_color_stop(rgba(from), 0.0),
        linear_color_stop(rgba(to), 1.0),
    )
}

/// A monochrome SVG icon from `assets/icons/`, tinted via `text_color`.
fn icon(name: &str, dim: f32, color: u32) -> impl IntoElement {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(dim))
        .text_color(rgb(color))
}

// ── placeholder data ─────────────────────────────────────────────────────────
struct SourceData {
    title: &'static str,
    subtitle: &'static str,
    recording: bool,
    selected: bool,
    captured: &'static str,
    clips: &'static str,
    last: &'static str,
}

fn sources() -> Vec<SourceData> {
    vec![
        SourceData { title: "Monitor",     subtitle: "Record entire desktop", recording: false, selected: false, captured: "12.4h",  clips: "7",  last: "2h ago" },
        SourceData { title: "Hades II",    subtitle: "Auto-record on launch", recording: true,  selected: true,  captured: "48.2h",  clips: "23", last: "now" },
        SourceData { title: "Factorio",    subtitle: "Manual capture",        recording: false, selected: false, captured: "112.6h", clips: "41", last: "3d ago" },
        SourceData { title: "ARC Raiders", subtitle: "Auto-record on launch", recording: false, selected: false, captured: "6.8h",   clips: "4",  last: "yesterday" },
    ]
}

// ── root ─────────────────────────────────────────────────────────────────────
struct DashboardMockup;

impl Render for DashboardMockup {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Size the preview from the live viewport so it scales with the window
        // (mirrors `dashboard.rs`, which derives video height the same way).
        let video_h = (window.viewport_size().height - px(430.0)).max(px(240.0));

        rsx! {
            <div size_full flex flex_col bg={lin(180.0, (BG_TOP << 8) | 0xff, (BG_BOT << 8) | 0xff)}
                 text_color={rgb(FG)}>
                {top_bar()}
                // `min_h: 0` lets this flex child shrink below its content
                // height so `overflow_y_scroll` actually scrolls instead of
                // the whole column overflowing the window.
                <div id={"dash-scroll"} flex_1 w_full min_h={px(0.0)} overflow_y_scroll>
                    <div w_full flex flex_col gap_6 px_8 pt_6 pb_10>
                        {video_preview(video_h)}
                        {controls_card()}
                        {timeline_bar()}
                        {sessions_section()}
                    </div>
                </div>
            </div>
        }
    }
}

fn top_bar() -> impl IntoElement {
    rsx! {
        <div h={px(56.0)} w_full flex_shrink_0 px_8 flex items_center justify_between bg={rgb(PANEL)}
             border_b_1 border_color={rgb(BORDER)}>
            <div flex items_center gap_3>
                <div size={px(26.0)} rounded_lg flex items_center justify_center
                     bg={lin(150.0, 0xa78bfaff, 0x7c5cf0ff)} text_color={rgb(0xffffff)}
                     text_xs font_weight={FontWeight::BLACK} shadow_md>{"R"}</div>
                <div text_lg font_weight={FontWeight::BOLD} text_color={rgb(FG_HI)}>{"Dashboard"}</div>
                <div px_2 py_1 rounded_md bg={rgb(WELL)} border_1 border_color={rgb(BORDER)}
                     text_xs text_color={rgb(MUTED)}>{"Hades II"}</div>
            </div>
            <div flex items_center gap_2>
                <div size={px(6.0)} rounded_full bg={rgb(REC)} />
                <div text_xs font_weight={FontWeight::MEDIUM} text_color={rgb(MUTED)}>{"rsx! mockup"}</div>
            </div>
        </div>
    }
}

// ── video preview ────────────────────────────────────────────────────────────
fn video_preview(height: Pixels) -> impl IntoElement {
    rsx! {
        <div relative w_full h={height} flex_shrink_0 rounded_xl overflow_hidden shadow_2xl
             border_1 border_color={rgba(HAIRLINE)}
             bg={lin(155.0, 0x1a1a24ff, 0x0a0a0eff)}>
            // centred play affordance
            <div absolute inset_0 flex items_center justify_center>
                <div size={px(72.0)} rounded_full flex items_center justify_center cursor_pointer
                     bg={rgba(0xffffff_14)} border_1 border_color={rgba(0xffffff_29)} shadow_lg
                     hover={|s| s.bg(rgba(0xffffff_24))}>
                    {icon("play", 26.0, FG_HI)}
                </div>
            </div>
            // bottom scrim + faux scrubber
            <div absolute bottom_0 left_0 right_0 h={px(96.0)}
                 bg={lin(180.0, 0x00000000, 0x000000cc)} />
            <div absolute bottom_0 left_0 right_0 h={px(3.0)} bg={rgba(0xffffff_1a)}>
                <div absolute top_0 bottom_0 left_0 w={relative(0.42)}
                     bg={lin(90.0, 0x8b5cf6ff, 0xa78bfaff)} />
            </div>
            // game logo bubble
            <div absolute top_4 left_4 size={px(34.0)} rounded_full flex items_center justify_center
                 bg={lin(160.0, (avatar_tint("Hades II") << 8) | 0xff, (avatar_tint("Hades II") << 8) | 0x88)}
                 border_1 border_color={rgba(0xffffff_2e)} text_sm font_weight={FontWeight::BOLD}
                 text_color={rgb(FG_HI)} shadow_md>{"H"}</div>
            // recording stats overlay
            <div absolute top_4 right_4 py_2 px_3 rounded_lg bg={rgba(0x0a0a0ce6)}
                 border_1 border_color={rgba(HAIRLINE)} shadow_lg>
                <div flex flex_col gap_1>
                    <div flex items_center gap_2>
                        <div relative size={px(8.0)} rounded_full bg={rgb(REC)}>
                            <div absolute inset={px(-3.0)} rounded_full border_1
                                 border_color={rgba(0xef4444_55)} />
                        </div>
                        <div text_xs font_weight={FontWeight::BOLD} text_color={rgb(REC)}>{"REC"}</div>
                        <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(FG_HI)}>{"12:48"}</div>
                    </div>
                    <div flex gap_3>
                        <div text_xs text_color={rgb(MUTED)}>{"24.8 Mbps"}</div>
                        <div text_xs text_color={rgb(MUTED)}>{"31.2 MB/s"}</div>
                    </div>
                    <div flex gap_3>
                        <div text_xs text_color={rgb(SUBTLE)}>{"Dropped 0"}</div>
                        <div text_xs text_color={rgb(SUBTLE)}>{"Segments 142"}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

// ── transport + clip controls ────────────────────────────────────────────────
fn controls_card() -> impl IntoElement {
    rsx! {
        <div w_full flex_shrink_0 bg={rgb(CARD)} border_1 border_color={rgb(BORDER)} rounded_xl
             shadow_lg p_4 flex flex_col gap_3>
            // row 1 — transport
            <div flex items_center justify_between>
                <div flex items_center gap_1 p_1 rounded_lg bg={rgb(WELL)}
                     border_1 border_color={rgb(BORDER)}>
                    // record
                    <div w={px(30.0)} h={px(30.0)} rounded_md flex items_center justify_center
                         cursor_pointer hover={|s| s.bg(rgba(0xffffff_0f))}>
                        <div size={px(13.0)} rounded_full bg={rgb(REC)} />
                    </div>
                    {divider()}
                    {icon_btn("skip-back", 30.0, FG)}
                    {play_btn()}
                    {icon_btn("skip-forward", 30.0, FG)}
                    {divider()}
                    <div px_2 py_1 mx_1 rounded_md bg={rgba(0xffffff_08)} text_sm
                         text_color={rgb(MUTED)}>{"3:21 / 1:12:40"}</div>
                </div>
                {icon_btn("settings", 32.0, MUTED)}
            </div>
            // row 2 — markers + clip controls
            <div flex items_center justify_between>
                <div flex items_center gap_2>
                    <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(SUBTLE)}
                         mr_1>{"MARKERS"}</div>
                    {..marker_btns()}
                </div>
                <div flex items_center gap_2>
                    {clip_pill("IN", "3:05")}
                    {clip_pill("OUT", "3:48")}
                    {divider()}
                    {save_btn()}
                </div>
            </div>
        </div>
    }
}

fn icon_btn(name: &str, dim: f32, fg: u32) -> impl IntoElement {
    rsx! {
        <div w={px(dim)} h={px(dim)} rounded_md flex items_center justify_center cursor_pointer
             hover={|s| s.bg(rgba(0xffffff_0f))}>
            {icon(name, 16.0, fg)}
        </div>
    }
}

fn play_btn() -> impl IntoElement {
    rsx! {
        <div w={px(36.0)} h={px(36.0)} rounded_lg flex items_center justify_center cursor_pointer
             bg={lin(180.0, 0x9d7bf5ff, 0x7c5cf0ff)} shadow_md
             hover={|s| s.bg(lin(180.0, 0xae90f8ff, 0x8d6ff3ff))}>
            {icon("play", 16.0, 0xffffff)}
        </div>
    }
}

fn divider() -> impl IntoElement {
    rsx! { <div w={px(1.0)} h={px(18.0)} mx_1 bg={rgb(BORDER)} /> }
}

fn marker_btns() -> Vec<AnyElement> {
    let colors = [0x3b82f6u32, 0xef4444, 0xf59e0b, 0xeab308];
    colors
        .iter()
        .map(|&c| {
            rsx! {
                <div w={px(30.0)} h={px(30.0)} rounded_lg flex items_center justify_center
                     cursor_pointer bg={rgba((c << 8) | 0x1f)} border_1
                     border_color={rgba((c << 8) | 0x33)}
                     hover={|s| s.bg(rgba((c << 8) | 0x33))}>
                    <div size={px(10.0)} rounded_full bg={rgb(c)} />
                </div>
            }
            .into_any_element()
        })
        .collect()
}

fn clip_pill(label: &'static str, time: &'static str) -> impl IntoElement {
    rsx! {
        <div flex items_center gap_1 px_2 h={px(28.0)} rounded_md bg={rgb(WELL)}
             border_1 border_color={rgb(BORDER)} cursor_pointer
             hover={|s| s.border_color(rgb(BORDER_HI))}>
            <div text_xs font_weight={FontWeight::BOLD} text_color={rgb(SUBTLE)}>{label}</div>
            <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(FG)}>{time}</div>
        </div>
    }
}

fn save_btn() -> impl IntoElement {
    rsx! {
        <div flex items_center gap_1 px_3 h={px(30.0)} rounded_lg cursor_pointer shadow_md
             bg={lin(180.0, 0x9d7bf5ff, 0x7c5cf0ff)}
             hover={|s| s.bg(lin(180.0, 0xae90f8ff, 0x8d6ff3ff))}>
            {icon("scissors", 13.0, 0xffffff)}
            <div text_xs font_weight={FontWeight::BOLD} text_color={rgb(0xffffff)}>{"SAVE CLIP"}</div>
        </div>
    }
}

// ── timeline ─────────────────────────────────────────────────────────────────
fn timeline_bar() -> impl IntoElement {
    rsx! {
        <div w_full flex_shrink_0 bg={rgb(CARD)} border_1 border_color={rgb(BORDER)} rounded_xl
             shadow_lg p_4 flex flex_col gap_3>
            <div flex items_center justify_between>
                <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(SUBTLE)}>{"TIMELINE"}</div>
                <div text_xs text_color={rgb(MUTED)}>{"1:12:40"}</div>
            </div>
            <div relative w_full h={px(40.0)}>
                // track
                <div absolute inset_0 rounded_lg overflow_hidden bg={rgb(WELL)}
                     border_1 border_color={rgb(BORDER)}>
                    // played region
                    <div absolute top_0 bottom_0 left_0 w={relative(0.42)}
                         bg={lin(90.0, 0x8b5cf633, 0xa78bfa44)} />
                    // clip in/out range
                    <div absolute top_0 bottom_0 left={relative(0.27)} w={relative(0.15)}
                         bg={rgba(0x8b5cf63a)} border_1 border_color={rgba(0x8b5cf680)} />
                </div>
                // marker ticks
                {marker_tick(0.16, 0x3b82f6)}
                {marker_tick(0.34, 0xef4444)}
                {marker_tick(0.55, 0xf59e0b)}
                // playhead + knob
                <div absolute top_0 bottom_0 left={relative(0.42)} w={px(2.0)} bg={rgb(FG_HI)}>
                    <div absolute top={px(-3.0)} left={px(-5.5)} size={px(13.0)} rounded_full
                         bg={rgb(FG_HI)} border_2 border_color={rgb(CARD)} shadow_md />
                </div>
            </div>
        </div>
    }
}

fn marker_tick(frac: f32, color: u32) -> impl IntoElement {
    rsx! {
        <div absolute top_0 bottom_0 left={relative(frac)} w={px(2.0)} bg={rgba((color << 8) | 0xcc)}>
            <div absolute top={px(-2.0)} left={px(-3.0)} size={px(8.0)} rounded_full bg={rgb(color)} />
        </div>
    }
}

// ── recent sessions gallery ──────────────────────────────────────────────────
fn sessions_section() -> impl IntoElement {
    let srcs = sources();
    let count = format!("{}", srcs.len());
    let cards: Vec<AnyElement> = std::iter::once(add_source_card().into_any_element())
        .chain(srcs.iter().map(|s| source_card(s).into_any_element()))
        .collect();
    rsx! {
        <div flex_shrink_0 w_full flex flex_col gap_4>
            <div flex items_center gap_2>
                <div text_xl font_weight={FontWeight::BOLD} text_color={rgb(FG_HI)}>{"Recent Sessions"}</div>
                <div px_2 py_1 rounded_md bg={rgb(WELL)} border_1 border_color={rgb(BORDER)}
                     text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(MUTED)}>{count}</div>
            </div>
            <div w_full flex flex_wrap gap_5>
                {..cards}
            </div>
        </div>
    }
}

fn add_source_card() -> impl IntoElement {
    rsx! {
        <div w={px(304.0)} h={px(188.0)} bg={rgb(CARD)} border_2 border_color={rgb(BORDER)}
             border_dashed rounded_2xl flex items_center justify_center cursor_pointer
             hover={|s| s.bg(rgb(CARD_HI)).border_color(rgb(PRIMARY))}>
            <div flex flex_col items_center gap_2>
                <div size={px(44.0)} rounded_full flex items_center justify_center
                     bg={rgb(WELL)} border_1 border_color={rgb(BORDER)}>
                    {icon("plus", 20.0, MUTED)}
                </div>
                <div text_sm text_color={rgb(MUTED)} font_weight={FontWeight::MEDIUM}>{"Add Source"}</div>
            </div>
        </div>
    }
}

fn source_card(s: &SourceData) -> impl IntoElement {
    let tint = avatar_tint(s.title);
    let border = if s.selected { PRIMARY } else { BORDER };
    let letter = s
        .title
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    rsx! {
        <div w={px(304.0)} h={px(188.0)} rounded_2xl bg={rgb(CARD)} border_2
             border_color={rgb(border)} overflow_hidden flex flex_col cursor_pointer shadow_lg
             hover={|s| s.border_color(rgb(BORDER_HI))}>
            // art body
            <div relative flex_1 overflow_hidden rounded_t_2xl
                 bg={lin(160.0, (tint << 8) | 0x33, (tint << 8) | 0x0a)}>
                <div absolute top_0 left_0 right_0 h={px(72.0)} bg={lin(180.0, 0x00000066, 0x00000000)} />
                <div absolute bottom_0 left_0 right_0 h={px(84.0)} bg={lin(180.0, 0x00000000, 0x000000a6)} />
                <div relative size_full px_4 py_4 flex flex_col justify_between>
                    <div flex items_start justify_between gap_3>
                        <div flex items_center gap_3 min_w={px(0.0)}>
                            {letter_avatar(letter, tint, s.recording)}
                            <div flex flex_col gap={px(2.0)} min_w={px(0.0)}>
                                <div text_base font_weight={FontWeight::SEMIBOLD}
                                     text_color={rgb(FG_HI)}>{s.title}</div>
                                <div text_xs text_color={rgba(0xffffff_a6)}>{s.subtitle}</div>
                            </div>
                        </div>
                        <div size={px(28.0)} rounded_full flex_shrink_0 flex items_center justify_center
                             bg={rgba(0x00000080)} border_1 border_color={rgba(0xffffff_24)}
                             cursor_pointer hover={|s| s.bg(rgba(0x000000b3))}>
                            {icon("settings", 14.0, FG_HI)}
                        </div>
                    </div>
                    <div flex items_center>
                        {status_chip(s.recording)}
                    </div>
                </div>
            </div>
            // stat strip
            <div h={px(54.0)} rounded_b_2xl border_t_1 border_color={rgb(BORDER)}
                 bg={rgb(WELL)} overflow_hidden flex>
                {stat_cell("CAPTURED", s.captured)}
                {stat_divider()}
                {stat_cell("CLIPS", s.clips)}
                {stat_divider()}
                {stat_cell("LAST", s.last)}
            </div>
        </div>
    }
}

fn letter_avatar(letter: String, tint: u32, recording: bool) -> impl IntoElement {
    rsx! {
        <div relative size={px(40.0)} rounded_xl flex_shrink_0 flex items_center justify_center
             bg={lin(160.0, (tint << 8) | 0xff, (tint << 8) | 0x99)} border_1
             border_color={rgba(0xffffff_24)} text_sm font_weight={FontWeight::BOLD}
             text_color={rgb(FG_HI)} shadow_md>
            {letter}
            <when {recording}>
                <div absolute inset={px(-3.0)} rounded_xl border_2 border_color={rgba(0xef4444_b3)} />
            </when>
        </div>
    }
}

fn status_chip(recording: bool) -> impl IntoElement {
    let (label, dot, fg, bg, br): (&'static str, u32, u32, u32, u32) = if recording {
        ("Recording", 0xef4444, 0xfecaca, 0xef4444_2e, 0xef4444_66)
    } else {
        ("Idle", 0x10b981, 0xa7f3d0, 0x10b981_24, 0x10b981_55)
    };
    rsx! {
        <div h={px(24.0)} px_2 rounded_full flex items_center gap_2
             bg={rgba(bg)} border_1 border_color={rgba(br)}>
            <div size={px(6.0)} rounded_full bg={rgb(dot)} />
            <div text_xs font_weight={FontWeight::SEMIBOLD} text_color={rgb(fg)}>{label}</div>
        </div>
    }
}

fn stat_cell(label: &'static str, value: &'static str) -> impl IntoElement {
    rsx! {
        <div flex_1 px_3 flex flex_col justify_center gap={px(3.0)}>
            <div text_sm font_weight={FontWeight::BOLD} text_color={rgb(FG_HI)}>{value}</div>
            <div text_xs font_weight={FontWeight::MEDIUM} text_color={rgb(SUBTLE)}>{label}</div>
        </div>
    }
}

fn stat_divider() -> impl IntoElement {
    rsx! { <div my_3 w={px(1.0)} bg={rgb(BORDER)} /> }
}

fn avatar_tint(title: &str) -> u32 {
    const PALETTE: &[u32] = &[0x6366f1, 0xec4899, 0xf59e0b, 0x10b981, 0x06b6d4, 0x8b5cf6];
    let idx = title.as_bytes().first().copied().unwrap_or(0) as usize % PALETTE.len();
    PALETTE[idx]
}

// ── asset source — loads `assets/icons/*.svg` from the repo at its dev path ──
struct MockupAssets;

impl AssetSource for MockupAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        Ok(Some(Cow::Owned(std::fs::read(base.join(path))?)))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

fn main() {
    Application::new()
        .with_assets(MockupAssets)
        .run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1100.0), px(940.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Rekaptr — Dashboard Mockup (rsx!)".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| DashboardMockup),
            )
            .unwrap();
            cx.activate(true);
        });
}
