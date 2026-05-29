// Quick Cut Editor (draft editor) mockup.
//
// Mocks the NLE described in roadmap/FEATURE_DRAFT_EDITOR.md:
//   - Project title + Save / Export in the top bar
//   - Source browser on the left (segments you can drag onto the timeline)
//   - Big preview pane on the right with transport
//   - Two-track timeline below: Primary (Layer 0) + Overlay (Layer 1),
//     with ruler, playhead, zoom slider, and rounded clip blocks with trim
//     handles
//   - Inspector strip revealed when a clip is selected — volume slider,
//     fade in/out toggles, delete
//   - "Smart cut" pill next to Export when the project reduces to a single
//     trimmed clip on Primary (stream-copy fast path)
//
// Self-contained: no real GES, no real video. All data is mocked.

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

// ── Mocked data ─────────────────────────────────────────────────────
#[derive(Clone)]
struct SourceAsset {
    id: u64,
    title: String,
    game: String,
    duration: f32,
    thumb_tint: u32,
}

#[derive(Clone)]
struct DraftClip {
    id: u64,
    asset_id: u64,
    title: String,
    thumb_tint: u32,
    trim_start: f32,  // seconds into the source asset
    trim_end: f32,    // seconds into the source asset
    timeline_pos: f32, // seconds on the editor timeline
    volume: f32,      // 0..1
    fade_in: bool,
    fade_out: bool,
    layer: u8,        // 0 = Primary, 1 = Overlay
}

impl DraftClip {
    fn duration(&self) -> f32 {
        (self.trim_end - self.trim_start).max(0.0)
    }
    fn end(&self) -> f32 {
        self.timeline_pos + self.duration()
    }
}

fn mock_sources() -> Vec<SourceAsset> {
    vec![
        SourceAsset { id: 1, title: "Malenia phase 2 clear".into(), game: "Elden Ring".into(), duration: 38.5, thumb_tint: 0x8B6F3FFF },
        SourceAsset { id: 2, title: "Clutch 1v4 on Mirage".into(), game: "Counter-Strike 2".into(), duration: 24.2, thumb_tint: 0x6E7F66FF },
        SourceAsset { id: 3, title: "Extraction under fire".into(), game: "Helldivers 2".into(), duration: 54.8, thumb_tint: 0x3F5B8BFF },
        SourceAsset { id: 4, title: "Ninja defuse".into(), game: "Counter-Strike 2".into(), duration: 11.4, thumb_tint: 0x6E7F66FF },
        SourceAsset { id: 5, title: "Mohg solo".into(), game: "Elden Ring".into(), duration: 92.0, thumb_tint: 0x8B6F3FFF },
        SourceAsset { id: 6, title: "Automaton base assault".into(), game: "Helldivers 2".into(), duration: 44.3, thumb_tint: 0x3F5B8BFF },
    ]
}

// ── Workspace ───────────────────────────────────────────────────────
struct DraftEditor {
    project_title: Entity<InputState>,
    sources: Vec<SourceAsset>,
    clips: Vec<DraftClip>,
    selected_clip: Option<u64>,
    playhead: f32,
    playing: bool,
    zoom_px_per_sec: f32,
    inspector_fade_in: bool,
    inspector_fade_out: bool,
}

impl DraftEditor {
    fn new(cx: &mut Context<Self>) -> Self {
        let project_title = cx.new(|cx| InputState::new(cx));

        // Seed the timeline with a couple of clips + one overlay.
        let clips = vec![
            DraftClip {
                id: 101, asset_id: 2, title: "Clutch 1v4 on Mirage".into(), thumb_tint: 0x6E7F66FF,
                trim_start: 3.0, trim_end: 22.0, timeline_pos: 0.0,
                volume: 1.0, fade_in: true, fade_out: false, layer: 0,
            },
            DraftClip {
                id: 102, asset_id: 4, title: "Ninja defuse".into(), thumb_tint: 0x6E7F66FF,
                trim_start: 1.5, trim_end: 11.4, timeline_pos: 19.0,
                volume: 0.85, fade_in: false, fade_out: true, layer: 0,
            },
            DraftClip {
                id: 103, asset_id: 1, title: "Malenia reaction".into(), thumb_tint: 0x8B6F3FFF,
                trim_start: 0.0, trim_end: 6.0, timeline_pos: 14.0,
                volume: 0.0, fade_in: false, fade_out: false, layer: 1,
            },
        ];

        Self {
            project_title,
            sources: mock_sources(),
            clips,
            selected_clip: Some(101),
            playhead: 6.0,
            playing: false,
            zoom_px_per_sec: 24.0,
            inspector_fade_in: true,
            inspector_fade_out: false,
        }
    }

    fn total_duration(&self) -> f32 {
        self.clips
            .iter()
            .filter(|c| c.layer == 0)
            .map(|c| c.end())
            .fold(0.0_f32, f32::max)
            .max(30.0)
    }

    fn selected(&self) -> Option<&DraftClip> {
        self.selected_clip
            .and_then(|id| self.clips.iter().find(|c| c.id == id))
    }

    fn smart_cut_eligible(&self) -> bool {
        let primary: Vec<&DraftClip> = self.clips.iter().filter(|c| c.layer == 0).collect();
        let overlays = self.clips.iter().any(|c| c.layer == 1);
        primary.len() == 1 && !overlays
    }
}

// ── Render root ─────────────────────────────────────────────────────
impl Render for DraftEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .child(render_app_sidebar())
            .child(
                VStack::new()
                    .flex_1()
                    .h_full()
                    .child(self.render_top_bar(cx))
                    .child(
                        HStack::new()
                            .flex_1()
                            .h_0()
                            .child(self.render_source_browser(cx))
                            .child(self.render_preview_pane(cx)),
                    )
                    .child(self.render_timeline_panel(cx))
                    .when(self.selected().is_some(), |this| this.child(self.render_inspector(cx))),
            )
    }
}

// ── App rail ────────────────────────────────────────────────────────
fn render_app_sidebar() -> impl IntoElement {
    VStack::new()
        .w(px(72.0))
        .h_full()
        .bg(rgba(CARD))
        .border_r_1()
        .border_color(rgba(BORDER))
        .pt(px(12.0))
        .px(px(8.0))
        .gap_2()
        .child(app_nav_item("nav-dash", "layout-dashboard", false))
        .child(app_nav_item("nav-clips", "video", true))
        .child(app_nav_item("nav-settings", "settings", false))
}

fn app_nav_item(id: &'static str, icon_name: &'static str, active: bool) -> impl IntoElement {
    div()
        .id(id)
        .w_full()
        .h(px(56.0))
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            div()
                .size(px(48.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .bg(if active { rgba(CARD_HOVER) } else { rgba(0x00000000) })
                .hover(|s| s.bg(rgba(CARD_HOVER)))
                .child(
                    Icon::new(IconSource::Named(icon_name.into()))
                        .size(px(22.0))
                        .color(if active { rgba(FG).into() } else { rgba(FG_MUTED).into() }),
                ),
        )
        .when(active, |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(16.0))
                    .w(px(3.0))
                    .h(px(24.0))
                    .rounded_r_sm()
                    .bg(rgba(PRIMARY)),
            )
        })
}

// ── Top bar ─────────────────────────────────────────────────────────
impl DraftEditor {
    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let smart = self.smart_cut_eligible();

        HStack::new()
            .h(px(56.0))
            .px_4()
            .gap_4()
            .items_center()
            .border_b_1()
            .border_color(rgba(BORDER))
            .child(
                Button::new("back", "")
                    .icon(IconSource::Named("chevron-left".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("scissors".into()))
                            .size(px(16.0))
                            .color(rgba(PRIMARY).into()),
                    )
                    .child(
                        div()
                            .w(px(260.0))
                            .child(
                                Input::new(&self.project_title)
                                    .placeholder("Untitled draft"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(format!(
                                "· {} clips · {}",
                                self.clips.iter().filter(|c| c.layer == 0).count(),
                                fmt_duration(self.total_duration()),
                            )),
                    ),
            )
            .child(div().flex_1())
            .when(smart, |this| {
                this.child(
                    HStack::new()
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_1()
                        .rounded_full()
                        .bg(rgba(0x22C55E30))
                        .border_1()
                        .border_color(rgba(SUCCESS))
                        .child(
                            Icon::new(IconSource::Named("zap".into()))
                                .size(px(12.0))
                                .color(rgba(SUCCESS).into()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgba(SUCCESS))
                                .child("Smart cut eligible"),
                        ),
                )
            })
            .child(
                Button::new("save-draft", "Save draft")
                    .icon(IconSource::Named("save".into()))
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm),
            )
            .child(
                Button::new("export", "Export")
                    .icon(IconSource::Named("download".into()))
                    .variant(ButtonVariant::Default)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|_, _, _, _| {})),
            )
    }
}

// ── Source browser ──────────────────────────────────────────────────
impl DraftEditor {
    fn render_source_browser(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .w(px(280.0))
            .h_full()
            .bg(rgba(SURFACE))
            .border_r_1()
            .border_color(rgba(BORDER))
            .child(
                HStack::new()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgba(BORDER))
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgba(FG_SUBTLE))
                            .child("SOURCES"),
                    )
                    .child(
                        Button::new("add-source", "")
                            .icon(IconSource::Named("plus".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
            .child(
                div()
                    .id("src-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .p_3()
                            .gap_2()
                            .children(self.sources.iter().map(|s| self.render_source_card(s, cx))),
                    ),
            )
    }

    fn render_source_card(&self, asset: &SourceAsset, cx: &mut Context<Self>) -> impl IntoElement {
        let asset_clone = asset.clone();
        div()
            .id(SharedString::from(format!("src-{}", asset.id)))
            .flex()
            .flex_row()
            .gap_3()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgba(BORDER))
            .bg(rgba(CARD))
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(BORDER_STRONG)).bg(rgba(CARD_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    // Drop the clip at the current end of the primary track.
                    let end = this
                        .clips
                        .iter()
                        .filter(|c| c.layer == 0)
                        .map(|c| c.end())
                        .fold(0.0_f32, f32::max);
                    let new_id = this.clips.iter().map(|c| c.id).max().unwrap_or(100) + 1;
                    this.clips.push(DraftClip {
                        id: new_id,
                        asset_id: asset_clone.id,
                        title: asset_clone.title.clone(),
                        thumb_tint: asset_clone.thumb_tint,
                        trim_start: 0.0,
                        trim_end: asset_clone.duration,
                        timeline_pos: end,
                        volume: 1.0,
                        fade_in: false,
                        fade_out: false,
                        layer: 0,
                    });
                    this.selected_clip = Some(new_id);
                    cx.notify();
                }),
            )
            // Thumb
            .child(
                div()
                    .w(px(72.0))
                    .h(px(48.0))
                    .rounded_sm()
                    .bg(rgba(asset.thumb_tint))
                    .relative()
                    .overflow_hidden()
                    .child(div().absolute().inset_0().bg(rgba(0x00000033)))
                    .child(
                        div()
                            .absolute()
                            .bottom(px(3.0))
                            .right(px(3.0))
                            .px_1()
                            .rounded_sm()
                            .bg(rgba(0x000000CC))
                            .text_color(rgba(FG))
                            .text_xs()
                            .child(fmt_duration(asset.duration)),
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
                            .text_color(rgba(FG))
                            .child(asset.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(asset.game.clone()),
                    ),
            )
    }
}

// ── Preview pane + transport ────────────────────────────────────────
impl DraftEditor {
    fn render_preview_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let clip_at_head = self
            .clips
            .iter()
            .filter(|c| c.layer == 0)
            .find(|c| self.playhead >= c.timeline_pos && self.playhead < c.end());
        let tint = clip_at_head
            .map(|c| c.thumb_tint)
            .unwrap_or(0x1F1F24FF);
        let title = clip_at_head
            .map(|c| c.title.clone())
            .unwrap_or_else(|| "—".to_string());
        let total = self.total_duration();

        VStack::new()
            .flex_1()
            .h_full()
            .bg(rgba(BG))
            .child(
                // Preview surface
                div()
                    .flex_1()
                    .relative()
                    .bg(rgba(tint))
                    .overflow_hidden()
                    .child(div().absolute().inset_0().bg(rgba(0x00000066)))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .size(px(72.0))
                                    .rounded_full()
                                    .bg(rgba(0x00000099))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named(
                                            if self.playing { "pause" } else { "play" }.into(),
                                        ))
                                        .size(px(28.0))
                                        .color(rgba(FG).into()),
                                    ),
                            ),
                    )
                    // Top-left: clip-at-playhead label
                    .child(
                        div()
                            .absolute()
                            .top(px(12.0))
                            .left(px(12.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgba(0x000000AA))
                            .text_xs()
                            .text_color(rgba(FG))
                            .child(title),
                    )
                    // Bottom-right: time
                    .child(
                        div()
                            .absolute()
                            .bottom(px(12.0))
                            .right(px(12.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgba(0x000000AA))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgba(FG))
                            .child(format!("{} / {}", fmt_clock(self.playhead), fmt_clock(total))),
                    ),
            )
            .child(self.render_transport(cx))
    }

    fn render_transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .h(px(48.0))
            .px_4()
            .gap_3()
            .items_center()
            .border_t_1()
            .border_color(rgba(BORDER))
            .bg(rgba(SURFACE))
            .child(
                Button::new("skip-back", "")
                    .icon(IconSource::Named("skip-back".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.playhead = 0.0;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("play-pause", "")
                    .icon(IconSource::Named(
                        if self.playing { "pause" } else { "play" }.into(),
                    ))
                    .variant(ButtonVariant::Default)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.playing = !this.playing;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("skip-fwd", "")
                    .icon(IconSource::Named("skip-forward".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
            .child(div().flex_1())
            .child(
                Button::new("split", "Split")
                    .icon(IconSource::Named("scissors".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
            .child(
                Button::new("undo", "")
                    .icon(IconSource::Named("rotate-ccw".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
            .child(
                Button::new("redo", "")
                    .icon(IconSource::Named("rotate-cw".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm),
            )
    }
}

// ── Timeline panel ──────────────────────────────────────────────────
impl DraftEditor {
    fn render_timeline_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.total_duration();
        let timeline_px_width = (total * self.zoom_px_per_sec).max(600.0);

        VStack::new()
            .h(px(260.0))
            .bg(rgba(SURFACE))
            .border_t_1()
            .border_color(rgba(BORDER))
            .child(self.render_timeline_header(cx))
            .child(
                div()
                    .id("tl-scroll")
                    .flex_1()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .w(px(timeline_px_width))
                            .h_full()
                            .relative()
                            .child(self.render_ruler(timeline_px_width, cx))
                            .child(self.render_track(1, "Overlay", 32.0, timeline_px_width, cx))
                            .child(self.render_track(0, "Primary", 64.0, timeline_px_width, cx))
                            .child(self.render_playhead(timeline_px_width)),
                    ),
            )
    }

    fn render_timeline_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .h(px(36.0))
            .px_4()
            .gap_4()
            .items_center()
            .border_b_1()
            .border_color(rgba(BORDER))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(FG_SUBTLE))
                    .child("TIMELINE"),
            )
            .when_some(self.selected(), |this, clip| {
                let clip = clip.clone();
                this.child(
                    HStack::new()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgba(FG_MUTED))
                                .child(clip.title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgba(FG_SUBTLE))
                                .child(format!(
                                    "· {} → {} · {}",
                                    fmt_clock(clip.trim_start),
                                    fmt_clock(clip.trim_end),
                                    fmt_duration(clip.duration()),
                                )),
                        ),
                )
            })
            .child(div().flex_1())
            .child(self.render_zoom_slider(cx))
    }

    fn render_zoom_slider(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Maps zoom range 6..80 px/s to a 0..1 position.
        let min = 6.0;
        let max = 80.0;
        let pos = ((self.zoom_px_per_sec - min) / (max - min)).clamp(0.0, 1.0);
        HStack::new()
            .gap_2()
            .items_center()
            .child(
                Icon::new(IconSource::Named("zoom-out".into()))
                    .size(px(12.0))
                    .color(rgba(FG_SUBTLE).into()),
            )
            .child(
                // Track
                div()
                    .id("zoom-track")
                    .w(px(120.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(rgba(BORDER))
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .h_full()
                            .w(relative(pos))
                            .bg(rgba(PRIMARY))
                            .rounded_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(-4.0))
                            .left(relative(pos))
                            .size(px(12.0))
                            .rounded_full()
                            .bg(rgba(FG))
                            .border_2()
                            .border_color(rgba(PRIMARY)),
                    ),
            )
            .child(
                Button::new("zoom-in-tick", "")
                    .icon(IconSource::Named("plus".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_px_per_sec = (this.zoom_px_per_sec * 1.3).min(80.0);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("zoom-out-tick", "")
                    .icon(IconSource::Named("minus".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_px_per_sec = (this.zoom_px_per_sec / 1.3).max(6.0);
                        cx.notify();
                    })),
            )
    }

    fn render_ruler(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let total_secs = (width / self.zoom_px_per_sec) as i32;
        let tick_step = if self.zoom_px_per_sec > 40.0 { 1 } else if self.zoom_px_per_sec > 16.0 { 2 } else { 5 };
        let label_step = tick_step.max(5);

        let mut ruler = div()
            .id("ruler")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .w(px(width))
            .h(px(28.0))
            .bg(rgba(CARD))
            .border_b_1()
            .border_color(rgba(BORDER))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    // Seek by click x — approximate, rel-to-scroll viewport.
                    let x = ev.position.x.0;
                    this.playhead = (x / this.zoom_px_per_sec).max(0.0);
                    cx.notify();
                }),
            );

        let mut t = 0i32;
        while t <= total_secs {
            let x = t as f32 * self.zoom_px_per_sec;
            let is_major = t % label_step == 0;
            ruler = ruler.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(if is_major { 10.0 } else { 16.0 }))
                    .w(px(1.0))
                    .h(px(if is_major { 12.0 } else { 6.0 }))
                    .bg(rgba(if is_major { BORDER_STRONG } else { BORDER })),
            );
            if is_major {
                ruler = ruler.child(
                    div()
                        .absolute()
                        .left(px(x + 4.0))
                        .top(px(4.0))
                        .text_xs()
                        .text_color(rgba(FG_SUBTLE))
                        .child(fmt_clock(t as f32)),
                );
            }
            t += tick_step;
        }
        ruler
    }

    fn render_track(
        &self,
        layer: u8,
        label: &str,
        height: f32,
        width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let top = if layer == 1 { 32.0 } else { 72.0 };
        let mut track = div()
            .absolute()
            .top(px(top))
            .left(px(0.0))
            .w(px(width))
            .h(px(height))
            .bg(rgba(if layer == 0 { 0x151519FF } else { 0x1A1A1FFF }))
            .border_b_1()
            .border_color(rgba(BORDER))
            // Track label (sticky at x=0 ideally, but here a small caption).
            .child(
                div()
                    .absolute()
                    .left(px(6.0))
                    .top(px(4.0))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(FG_SUBTLE))
                    .child(label.to_string()),
            );

        for clip in self.clips.iter().filter(|c| c.layer == layer) {
            track = track.child(self.render_clip_block(clip, height, cx));
        }
        track
    }

    fn render_clip_block(
        &self,
        clip: &DraftClip,
        track_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let x = clip.timeline_pos * self.zoom_px_per_sec;
        let w = (clip.duration() * self.zoom_px_per_sec).max(20.0);
        let selected = self.selected_clip == Some(clip.id);
        let clip_id = clip.id;
        let tint = clip.thumb_tint;

        let block_h = track_height - 8.0;

        div()
            .id(SharedString::from(format!("clip-{}", clip.id)))
            .absolute()
            .left(px(x))
            .top(px(4.0))
            .w(px(w))
            .h(px(block_h))
            .rounded_md()
            .border_1()
            .border_color(rgba(if selected { PRIMARY } else { BORDER_STRONG }))
            .bg(rgba(CARD))
            .overflow_hidden()
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(PRIMARY)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.selected_clip = Some(clip_id);
                    if let Some(c) = this.clips.iter().find(|c| c.id == clip_id) {
                        this.inspector_fade_in = c.fade_in;
                        this.inspector_fade_out = c.fade_out;
                    }
                    cx.notify();
                }),
            )
            // Colored top band
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .h(px(4.0))
                    .bg(rgba(tint)),
            )
            // Body
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .pt(px(6.0))
                    .px_2()
                    .child(
                        VStack::new()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgba(FG))
                                    .child(clip.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child(fmt_duration(clip.duration())),
                            ),
                    ),
            )
            // Fade-in hint (left wedge)
            .when(clip.fade_in, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(0.0))
                        .w(px(20.0))
                        .h_full()
                        .bg(rgba(0x00000088)),
                )
            })
            // Fade-out hint (right wedge)
            .when(clip.fade_out, |this| {
                this.child(
                    div()
                        .absolute()
                        .right(px(0.0))
                        .top(px(0.0))
                        .w(px(20.0))
                        .h_full()
                        .bg(rgba(0x00000088)),
                )
            })
            // Left trim handle
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(4.0))
                    .h_full()
                    .bg(rgba(if selected { PRIMARY } else { BORDER_STRONG })),
            )
            // Right trim handle
            .child(
                div()
                    .absolute()
                    .right(px(0.0))
                    .top(px(0.0))
                    .w(px(4.0))
                    .h_full()
                    .bg(rgba(if selected { PRIMARY } else { BORDER_STRONG })),
            )
    }

    fn render_playhead(&self, width: f32) -> impl IntoElement {
        let x = (self.playhead * self.zoom_px_per_sec).min(width);
        div()
            .absolute()
            .top(px(0.0))
            .left(px(x))
            .w(px(2.0))
            .h_full()
            .bg(rgba(PRIMARY))
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(-5.0))
                    .w(px(12.0))
                    .h(px(12.0))
                    .rounded_sm()
                    .bg(rgba(PRIMARY)),
            )
    }
}

// ── Inspector ───────────────────────────────────────────────────────
impl DraftEditor {
    fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let clip = match self.selected() {
            Some(c) => c.clone(),
            None => return div().into_any_element(),
        };
        let clip_id = clip.id;

        HStack::new()
            .h(px(72.0))
            .px_6()
            .gap_8()
            .items_center()
            .border_t_1()
            .border_color(rgba(BORDER))
            .bg(rgba(CARD))
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .size(px(40.0))
                            .rounded_sm()
                            .bg(rgba(clip.thumb_tint)),
                    )
                    .child(
                        VStack::new()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(clip.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child(format!(
                                        "Layer {} · {}",
                                        if clip.layer == 0 { "Primary" } else { "Overlay" },
                                        fmt_duration(clip.duration()),
                                    )),
                            ),
                    ),
            )
            .child(volume_control(cx, clip_id, clip.volume))
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("play".into()))
                            .size(px(12.0))
                            .color(rgba(FG_SUBTLE).into()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_MUTED))
                            .child("Fade"),
                    )
                    .child(fade_toggle(cx, "fi", "In", clip_id, clip.fade_in, true))
                    .child(fade_toggle(cx, "fo", "Out", clip_id, clip.fade_out, false)),
            )
            .child(div().flex_1())
            .child(
                Button::new("del-clip", "")
                    .icon(IconSource::Named("trash".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.clips.retain(|c| c.id != clip_id);
                        this.selected_clip = None;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

fn volume_control(
    cx: &mut Context<DraftEditor>,
    clip_id: u64,
    volume: f32,
) -> impl IntoElement {
    let v = volume.clamp(0.0, 1.0);
    HStack::new()
        .gap_3()
        .items_center()
        .child(
            Icon::new(IconSource::Named(if v < 0.01 { "volume-x" } else if v < 0.5 { "volume-1" } else { "volume-2" }.into()))
                .size(px(14.0))
                .color(rgba(FG_MUTED).into()),
        )
        .child(
            // Step-based volume via - / + buttons for a click-only mockup.
            HStack::new()
                .gap_1()
                .items_center()
                .child(
                    Button::new(SharedString::from(format!("vol-dec-{}", clip_id)), "")
                        .icon(IconSource::Named("minus".into()))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(c) = this.clips.iter_mut().find(|c| c.id == clip_id) {
                                c.volume = (c.volume - 0.1).max(0.0);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .w(px(110.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(rgba(SURFACE))
                        .border_1()
                        .border_color(rgba(BORDER))
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .left(px(0.0))
                                .top(px(0.0))
                                .h_full()
                                .w(relative(v))
                                .bg(rgba(PRIMARY))
                                .rounded_l_full(),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(-4.0))
                                .left(relative(v))
                                .size(px(12.0))
                                .rounded_full()
                                .bg(rgba(FG))
                                .border_2()
                                .border_color(rgba(PRIMARY)),
                        ),
                )
                .child(
                    Button::new(SharedString::from(format!("vol-inc-{}", clip_id)), "")
                        .icon(IconSource::Named("plus".into()))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(c) = this.clips.iter_mut().find(|c| c.id == clip_id) {
                                c.volume = (c.volume + 0.1).min(1.0);
                            }
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .w(px(44.0))
                .text_right()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba(FG_MUTED))
                .child(format!("{}%", (v * 100.0).round() as i32)),
        )
}

fn fade_toggle(
    cx: &mut Context<DraftEditor>,
    prefix: &'static str,
    label: &'static str,
    clip_id: u64,
    enabled: bool,
    is_fade_in: bool,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("{}-{}", prefix, clip_id)))
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if enabled { rgba(PRIMARY) } else { rgba(BORDER) })
        .bg(if enabled { rgba(PRIMARY_DIM_A25) } else { rgba(SURFACE) })
        .text_color(if enabled { rgba(FG) } else { rgba(FG_MUTED) })
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                if let Some(c) = this.clips.iter_mut().find(|c| c.id == clip_id) {
                    if is_fade_in { c.fade_in = !c.fade_in; } else { c.fade_out = !c.fade_out; }
                }
                cx.notify();
            }),
        )
        .child(label)
}

// ── Helpers ─────────────────────────────────────────────────────────
fn fmt_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn fmt_clock(secs: f32) -> String {
    let total = secs.max(0.0);
    let whole = total as u32;
    let frames = ((total - whole as f32) * 60.0).round() as u32;
    let m = whole / 60;
    let s = whole % 60;
    format!("{:02}:{:02}:{:02}", m, s, frames)
}

// ── main ────────────────────────────────────────────────────────────
fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1500.0), px(950.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Quick Cut Editor Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(DraftEditor::new),
        )
        .unwrap();
    });
}
