// Source-card redesign mockup — Variant A.
//
// Run with:  cargo run -p adabraka-gpui --example source_cards_mockup

use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, Context, FontWeight, ObjectFit, Render, Window, WindowBounds,
    WindowOptions, div, img, prelude::*, px, rgb, rgba, size,
};

// ---- palette ----------------------------------------------------------------
const BG: u32 = 0x0a0a0b;
const PANEL: u32 = 0x131316;
const PANEL_HI: u32 = 0x1a1a1f;
const CARD: u32 = 0x18181b;
const BORDER: u32 = 0x27272a;
const MUTED: u32 = 0x71717a;
const FG: u32 = 0xd4d4d8;
const FG_HI: u32 = 0xfafafa;
const ACCENT: u32 = 0x3b82f6;
const REC: u32 = 0xef4444;
const OK: u32 = 0x10b981;

struct SourceCardsMockup {
    hades_art: Option<PathBuf>,
    factorio_art: Option<PathBuf>,
    arc_raiders_art: Option<PathBuf>,
}

#[derive(Clone)]
struct SourceData {
    title: &'static str,
    subtitle: &'static str,
    is_recording: bool,
    is_selected: bool,
    hours: &'static str,
    clips: u32,
    last_clip: &'static str,
    art: Option<PathBuf>,
}

impl SourceCardsMockup {
    fn data(&self) -> [SourceData; 4] {
        [
            SourceData {
                title: "Monitor",
                subtitle: "Entire desktop",
                is_recording: false,
                is_selected: false,
                hours: "12.4h",
                clips: 7,
                last_clip: "2h ago",
                art: None,
            },
            SourceData {
                title: "Hades II",
                subtitle: "Auto-record on launch",
                is_recording: true,
                is_selected: true,
                hours: "48.2h",
                clips: 23,
                last_clip: "now",
                art: self.hades_art.clone(),
            },
            SourceData {
                title: "Factorio",
                subtitle: "Manual capture",
                is_recording: false,
                is_selected: false,
                hours: "112.6h",
                clips: 41,
                last_clip: "3d ago",
                art: self.factorio_art.clone(),
            },
            SourceData {
                title: "ARC Raiders",
                subtitle: "Auto-record on launch",
                is_recording: false,
                is_selected: false,
                hours: "6.8h",
                clips: 4,
                last_clip: "yesterday",
                art: self.arc_raiders_art.clone(),
            },
        ]
    }
}

impl Render for SourceCardsMockup {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let [monitor, hades, factorio, arc_raiders] = self.data();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .child(
                div()
                    .h(px(56.0))
                    .px_8()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(FG_HI))
                                    .child("Sources"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(rgb(CARD))
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("4"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child("Variant A — Stat-rich"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .px_8()
                    .py_8()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_6()
                            .child(card(monitor))
                            .child(card(hades))
                            .child(card(factorio))
                            .child(card(arc_raiders)),
                    ),
            )
    }
}

fn card(s: SourceData) -> impl IntoElement {
    let avatar_color = avatar_tint(s.title);
    let border_color = if s.is_selected { ACCENT } else { BORDER };

    div()
        .relative()
        .w(px(304.0))
        .h(px(188.0))
        .child(
            div()
                .relative()
                .size_full()
                .rounded_2xl()
                .bg(rgb(CARD))
                .border_2()
                .border_color(rgb(border_color))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(art_body(s.clone(), avatar_color))
                .child(stat_strip(s)),
        )
}

fn art_body(s: SourceData, avatar_color: u32) -> gpui::Div {
    let mut body = div()
        .relative()
        .flex_1()
        .overflow_hidden()
        .rounded_t_2xl()
        .bg(rgba((avatar_color << 8) | 0x14));

    // ── background artwork (when provided) ─────────────────────────────────
    if let Some(art) = s.art.clone() {
        body = body.child(
            img(art)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover),
        );
    }

    // ── gradient overlays ──────────────────────────────────────────────────
    body = body
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(70.0))
                .bg(rgba(0x000000_55)),
        )
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(80.0))
                .bg(rgba(0x000000_88)),
        );

    // ── content ────────────────────────────────────────────────────────────
    body.child(
        div()
            .relative()
            .size_full()
            .px_4()
            .py_4()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .min_w(px(0.0))
                            .child(letter_avatar(s.title, avatar_color, s.is_recording))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(FG_HI))
                                            .child(s.title),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0xd4d4d8))
                                            .child(s.subtitle),
                                    ),
                            ),
                    )
                    .child(round_btn("⚙")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(status_chip(s.is_recording)),
            ),
    )
}

fn stat_strip(s: SourceData) -> gpui::Div {
    div()
        .h(px(52.0))
        .rounded_b_2xl()
        .border_t_1()
        .border_color(rgb(BORDER))
        .bg(rgb(PANEL_HI))
        .overflow_hidden()
        .flex()
        .child(stat_cell("captured", s.hours))
        .child(stat_divider())
        .child(stat_cell("clips", &format!("{}", s.clips)))
        .child(stat_divider())
        .child(stat_cell("last", s.last_clip))
}

// ---- pieces -----------------------------------------------------------------

fn avatar_tint(title: &str) -> u32 {
    const PALETTE: &[u32] = &[
        0x6366f1, 0xec4899, 0xf59e0b, 0x10b981, 0x06b6d4, 0x8b5cf6,
    ];
    let idx = title.as_bytes().first().copied().unwrap_or(0) as usize % PALETTE.len();
    PALETTE[idx]
}

fn letter_avatar(title: &str, tint: u32, recording: bool) -> gpui::Div {
    let letter = title
        .chars()
        .next()
        .map(|c| c.to_uppercase().next().unwrap_or(c).to_string())
        .unwrap_or_else(|| "?".into());
    div()
        .relative()
        .size(px(38.0))
        .rounded_xl()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba((tint << 8) | 0x55))
        .border_1()
        .border_color(rgba(0xffffff_1a))
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(FG_HI))
        .child(letter)
        .when(recording, |this| {
            this.child(
                div()
                    .absolute()
                    .inset(px(-3.0))
                    .rounded_xl()
                    .border_1()
                    .border_color(rgba((REC << 8) | 0x99)),
            )
        })
}

fn round_btn(glyph: &'static str) -> gpui::Div {
    div()
        .size(px(28.0))
        .rounded_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x000000_55))
        .border_1()
        .border_color(rgba(0xffffff_1a))
        .text_xs()
        .text_color(rgb(FG_HI))
        .child(glyph)
}

fn status_chip(recording: bool) -> gpui::Div {
    let (label, dot, fg, bg, br) = if recording {
        ("Recording", REC, 0xfecaca, (REC << 8) | 0x33, (REC << 8) | 0x66)
    } else {
        ("Idle", OK, 0xa7f3d0, (OK << 8) | 0x22, (OK << 8) | 0x55)
    };
    div()
        .h(px(22.0))
        .px_2()
        .rounded_full()
        .flex()
        .items_center()
        .gap_2()
        .bg(rgba(bg))
        .border_1()
        .border_color(rgba(br))
        .child(div().size(px(6.0)).rounded_full().bg(rgb(dot)))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(fg))
                .child(label),
        )
}

fn stat_cell(label: &'static str, value: &str) -> gpui::Div {
    div()
        .flex_1()
        .px_3()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(FG_HI))
                .child(value.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(label),
        )
}

fn stat_divider() -> gpui::Div {
    div().my_3().w(px(1.0)).bg(rgb(BORDER))
}

fn fetch_art(url: &str, name: &str) -> Option<PathBuf> {
    let cache_dir = std::env::temp_dir().join("rekaptr_mockup_art");
    let _ = std::fs::create_dir_all(&cache_dir);
    let raw: PathBuf = cache_dir.join(format!("raw_{name}"));
    let blurred: PathBuf = cache_dir.join(name);

    if !blurred.exists() {
        if !raw.exists() {
            let status = std::process::Command::new("curl")
                .args(["-sSL", "-o"])
                .arg(&raw)
                .arg(url)
                .status()
                .ok()?;
            if !status.success() {
                let _ = std::fs::remove_file(&raw);
                return None;
            }
        }
        // Pre-blur once so the cards get a soft, art-bg feel without needing
        // a runtime shader. Sigma 4.0 ≈ slight haze; bumps to 8+ get painterly.
        let img = image::open(&raw).ok()?;
        let blurred_img = img.blur(4.0);
        blurred_img.save(&blurred).ok()?;
    }

    blurred.exists().then_some(blurred)
}

fn main() {
    let hades_art = fetch_art(
        "https://steamcdn-a.akamaihd.net/steam/apps/1145350/library_hero.jpg",
        "hades_ii_hero.jpg",
    );
    let factorio_art = fetch_art(
        "https://steamcdn-a.akamaihd.net/steam/apps/427520/library_hero.jpg",
        "factorio_hero.jpg",
    );
    let arc_raiders_art = fetch_art(
        "https://steamcdn-a.akamaihd.net/steam/apps/1808500/library_hero.jpg",
        "arc_raiders_hero.jpg",
    );

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1420.0), px(420.0)), cx);
        let hades_art = hades_art.clone();
        let factorio_art = factorio_art.clone();
        let arc_raiders_art = arc_raiders_art.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Rekaptr — Source Card Redesign".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| SourceCardsMockup {
                    hades_art,
                    factorio_art,
                    arc_raiders_art,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
