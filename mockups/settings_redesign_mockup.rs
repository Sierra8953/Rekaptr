// Settings page redesign mockup.
//
// Direction: replace the current flat horizontal tab strip with a two-pane
// "Preferences" layout — a categorized left nav (sections: General, Capture,
// Storage, System) + a scrollable content panel of grouped cards. Each card
// has a title, optional description, and a stack of labelled setting rows
// with rich controls (toggles, segmented selectors, steppers, pill pickers,
// level meters). Adds an always-visible search field, a breadcrumb header,
// and a sticky "unsaved changes" footer.
//
// Self-contained: no real config I/O, no real audio/video. All data is mocked.

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

// ── Theme (matches clips_redesign_mockup.rs) ────────────────────────
const BG: u32 = 0x09090BFF;
const SURFACE: u32 = 0x121215FF;
const CARD: u32 = 0x18181BFF;
const CARD_HOVER: u32 = 0x22222AFF;
const BORDER: u32 = 0x2A2A30FF;
const BORDER_STRONG: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const PRIMARY_DIM: u32 = 0x5B3FA8FF;
const WARN: u32 = 0xFBBF24FF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const FG_SUBTLE: u32 = 0x71717AFF;

// ── Nav model ───────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Behavior,
    Appearance,
    Startup,
    Video,
    Audio,
    Hotkeys,
    Storage,
    Export,
    Performance,
    About,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Behavior => "Behavior",
            Section::Appearance => "Appearance",
            Section::Startup => "Startup",
            Section::Video => "Video",
            Section::Audio => "Audio",
            Section::Hotkeys => "Hotkeys",
            Section::Storage => "Storage",
            Section::Export => "Export",
            Section::Performance => "Performance",
            Section::About => "About",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Section::Behavior => "sliders-horizontal",
            Section::Appearance => "palette",
            Section::Startup => "power",
            Section::Video => "video",
            Section::Audio => "mic",
            Section::Hotkeys => "keyboard",
            Section::Storage => "hard-drive",
            Section::Export => "scissors",
            Section::Performance => "cpu",
            Section::About => "info",
        }
    }
    fn breadcrumb(self) -> &'static str {
        match self {
            Section::Behavior | Section::Appearance | Section::Startup => "General",
            Section::Video | Section::Audio | Section::Hotkeys => "Capture",
            Section::Storage | Section::Export => "Storage",
            Section::Performance | Section::About => "System",
        }
    }
}

struct NavGroup {
    title: &'static str,
    items: &'static [Section],
}

const NAV: &[NavGroup] = &[
    NavGroup { title: "GENERAL", items: &[Section::Behavior, Section::Appearance, Section::Startup] },
    NavGroup { title: "CAPTURE", items: &[Section::Video, Section::Audio, Section::Hotkeys] },
    NavGroup { title: "STORAGE", items: &[Section::Storage, Section::Export] },
    NavGroup { title: "SYSTEM", items: &[Section::Performance, Section::About] },
];

// ── Workspace ───────────────────────────────────────────────────────
struct SettingsMockup {
    search: Entity<InputState>,
    section: Section,
    dirty: bool,

    // Behavior
    minimize_to_tray: bool,
    confirm_delete: bool,
    show_notifications: bool,
    language: String,

    // Appearance
    theme: String,
    accent_hue: u32,
    compact_mode: bool,

    // Startup
    launch_on_boot: bool,
    start_minimized: bool,
    auto_check_updates: bool,

    // Video
    encoder: String,
    resolution: String,
    fps: u32,
    rate_control: u32,
    cq: i32,
    bitrate: i32,
    preset: String,
    retention_minutes: i32,
    show_advanced_video: bool,
    gop_size: i32,
    bframes: i32,
    zero_latency: bool,
    lookahead: bool,
    lookahead_frames: i32,
    spatial_aq: bool,
    temporal_aq: bool,

    // Audio
    mic_device: String,
    mic_gain_db: f32,
    mic_force_mono: bool,
    mic_monitoring: bool,
    noise_suppression: bool,
    noise_gate: bool,
    noise_gate_threshold: f32,
    compressor_enabled: bool,
    compressor_threshold: f32,
    compressor_ratio: f32,
    limiter_enabled: bool,
    limiter_threshold: f32,
    system_level: f32,
    mic_level: f32,

    // Hotkeys
    hk_toggle_record: String,
    hk_save_clip: String,
    hk_toggle_mic: String,
    hk_push_to_talk: String,
    hk_marker_flag: String,
    hk_marker_kill: String,
    hk_marker_highlight: String,
    listening_slot: Option<u8>,

    // Storage
    clips_gb: f32,
    sessions_gb: f32,
    buffer_gb: f32,
    auto_delete: bool,
    auto_delete_days: i32,

    // Export
    export_format: String,
    export_quality: u32,

    // Performance
    hardware_decode: bool,
    prefer_gpu: bool,
    max_threads: i32,
}

impl SettingsMockup {
    fn new(cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(cx));
        Self {
            search,
            section: Section::Behavior,
            dirty: false,
            minimize_to_tray: true,
            confirm_delete: true,
            show_notifications: true,
            language: "English (US)".into(),
            theme: "Dark".into(),
            accent_hue: 262,
            compact_mode: false,
            launch_on_boot: false,
            start_minimized: true,
            auto_check_updates: true,
            encoder: "HEVC".into(),
            resolution: "1920x1080".into(),
            fps: 60,
            rate_control: 0,
            cq: 23,
            bitrate: 16000,
            preset: "p4".into(),
            retention_minutes: 10,
            show_advanced_video: false,
            gop_size: 60,
            bframes: 2,
            zero_latency: false,
            lookahead: true,
            lookahead_frames: 8,
            spatial_aq: true,
            temporal_aq: true,
            mic_device: "Shure MV7".into(),
            mic_gain_db: 6.0,
            mic_force_mono: false,
            mic_monitoring: false,
            noise_suppression: true,
            noise_gate: false,
            noise_gate_threshold: -40.0,
            compressor_enabled: false,
            compressor_threshold: -18.0,
            compressor_ratio: 3.0,
            limiter_enabled: true,
            limiter_threshold: -3.0,
            system_level: 0.62,
            mic_level: 0.34,
            hk_toggle_record: "F9".into(),
            hk_save_clip: "F10".into(),
            hk_toggle_mic: "Ctrl + Shift + M".into(),
            hk_push_to_talk: "Mouse4".into(),
            hk_marker_flag: "F6".into(),
            hk_marker_kill: "F8".into(),
            hk_marker_highlight: "F7".into(),
            listening_slot: None,
            clips_gb: 18.4,
            sessions_gb: 64.2,
            buffer_gb: 40.0,
            auto_delete: true,
            auto_delete_days: 30,
            export_format: "mp4".into(),
            export_quality: 2,
            hardware_decode: true,
            prefer_gpu: true,
            max_threads: 8,
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

// ── Render ──────────────────────────────────────────────────────────
impl Render for SettingsMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .child(render_app_sidebar())
            .child(self.render_nav_rail(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .relative()
                    .child(
                        VStack::new()
                            .size_full()
                            .child(self.render_top_bar())
                            .child(
                                div()
                                    .id("settings-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(
                                        div()
                                            .max_w(px(880.0))
                                            .mx_auto()
                                            .px_10()
                                            .pt_8()
                                            .pb_32()
                                            .child(self.render_section(cx)),
                                    ),
                            ),
                    )
                    .when(self.dirty, |this| this.child(self.render_save_bar(cx))),
            )
    }
}

// ── Outer app rail ──────────────────────────────────────────────────
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
        .child(app_nav_item("nav-clips", "video", false))
        .child(app_nav_item("nav-settings", "settings", true))
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

// ── Inner categorized rail ──────────────────────────────────────────
impl SettingsMockup {
    fn render_nav_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.read(cx).content().to_lowercase();

        let mut rail = VStack::new()
            .w(px(260.0))
            .h_full()
            .bg(rgba(SURFACE))
            .border_r_1()
            .border_color(rgba(BORDER))
            .pt_6()
            .pb_4()
            .px_3()
            .gap_1()
            .child(
                div().px_3().pb_4().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .child("Preferences"),
                ),
            )
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .child(Input::new(&self.search).placeholder("Search settings...")),
            );

        for group in NAV {
            let matches: Vec<Section> = group
                .items
                .iter()
                .copied()
                .filter(|s| query.is_empty() || s.label().to_lowercase().contains(&query))
                .collect();
            if matches.is_empty() {
                continue;
            }
            rail = rail
                .child(
                    div()
                        .px_3()
                        .pt_5()
                        .pb_2()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgba(FG_SUBTLE))
                        .child(group.title),
                )
                .children(matches.into_iter().map(|s| self.rail_item(s, cx)));
        }
        rail
    }

    fn rail_item(&self, section: Section, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.section == section;
        div()
            .id(SharedString::from(format!("nav-{}", section.label())))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .h(px(36.0))
            .px_3()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { rgba(CARD_HOVER) } else { rgba(0x00000000) })
            .hover(|s| s.bg(rgba(CARD)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.section = section;
                    this.listening_slot = None;
                    cx.notify();
                }),
            )
            .child(
                Icon::new(IconSource::Named(section.icon().into()))
                    .size(px(16.0))
                    .color(if active {
                        rgba(PRIMARY).into()
                    } else {
                        rgba(FG_MUTED).into()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(section.label()),
            )
            .when(active, |this| {
                this.child(
                    Icon::new(IconSource::Named("chevron-right".into()))
                        .size(px(14.0))
                        .color(rgba(FG_SUBTLE).into()),
                )
            })
    }

    // ── Top bar ─────────────────────────────────────────────────────
    fn render_top_bar(&self) -> impl IntoElement {
        HStack::new()
            .px_10()
            .py_5()
            .border_b_1()
            .border_color(rgba(BORDER))
            .justify_between()
            .items_center()
            .child(
                VStack::new()
                    .gap_1()
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(div().text_xs().text_color(rgba(FG_SUBTLE)).child(self.section.breadcrumb()))
                            .child(
                                Icon::new(IconSource::Named("chevron-right".into()))
                                    .size(px(12.0))
                                    .color(rgba(FG_SUBTLE).into()),
                            )
                            .child(div().text_xs().text_color(rgba(FG_MUTED)).child(self.section.label())),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child(self.section.label()),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .when(self.dirty, |this| {
                        this.child(
                            HStack::new()
                                .gap_2()
                                .items_center()
                                .child(div().size(px(8.0)).rounded_full().bg(rgba(WARN)))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgba(FG_MUTED))
                                        .child("Unsaved changes"),
                                ),
                        )
                    })
                    .child(
                        Button::new("docs", "")
                            .icon(IconSource::Named("book-open".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
    }

    // ── Sticky save bar ─────────────────────────────────────────────
    fn render_save_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .px_10()
            .py_4()
            .border_t_1()
            .border_color(rgba(BORDER))
            .bg(rgba(SURFACE))
            .child(
                HStack::new()
                    .max_w(px(880.0))
                    .mx_auto()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgba(FG_MUTED))
                            .child("You have unsaved changes."),
                    )
                    .child(
                        HStack::new()
                            .gap_3()
                            .child(
                                Button::new("revert", "Revert")
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dirty = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("save", "Save changes")
                                    .icon(IconSource::Named("check".into()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dirty = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    // ── Section dispatcher ──────────────────────────────────────────
    fn render_section(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.section {
            Section::Behavior => self.section_behavior(cx).into_any_element(),
            Section::Appearance => self.section_appearance(cx).into_any_element(),
            Section::Startup => self.section_startup(cx).into_any_element(),
            Section::Video => self.section_video(cx).into_any_element(),
            Section::Audio => self.section_audio(cx).into_any_element(),
            Section::Hotkeys => self.section_hotkeys(cx).into_any_element(),
            Section::Storage => self.section_storage(cx).into_any_element(),
            Section::Export => self.section_export(cx).into_any_element(),
            Section::Performance => self.section_performance(cx).into_any_element(),
            Section::About => self.section_about().into_any_element(),
        }
    }
}

// ── Section content ─────────────────────────────────────────────────
impl SettingsMockup {
    fn section_behavior(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(settings_card(
                "General behavior",
                Some("How Rekaptr runs in the background."),
                VStack::new()
                    .child(toggle_row(cx, "beh-tray", "Minimize to tray",
                        Some("Keep Rekaptr running in the system tray when the window is closed."),
                        self.minimize_to_tray,
                        |this, _| { this.minimize_to_tray = !this.minimize_to_tray; this.mark_dirty(); }))
                    .child(toggle_row(cx, "beh-del", "Confirm before deleting clips",
                        None, self.confirm_delete,
                        |this, _| { this.confirm_delete = !this.confirm_delete; this.mark_dirty(); }))
                    .child(toggle_row(cx, "beh-not", "Show desktop notifications",
                        Some("Toasts for saved clips, markers, and errors."),
                        self.show_notifications,
                        |this, _| { this.show_notifications = !this.show_notifications; this.mark_dirty(); })),
            ))
            .child(settings_card(
                "Language & region", None,
                VStack::new().child(segmented_row(
                    cx, "lang", "Language", &self.language,
                    &["English (US)", "English (UK)", "日本語", "Deutsch"],
                    |this, v, _| { this.language = v; this.mark_dirty(); },
                )),
            ))
    }

    fn section_appearance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(settings_card(
                "Theme",
                Some("Rekaptr always uses a dark base. Accent applies throughout the UI."),
                VStack::new()
                    .child(segmented_row(cx, "thm", "Base theme", &self.theme,
                        &["Dark", "Midnight", "OLED"],
                        |this, v, _| { this.theme = v; this.mark_dirty(); }))
                    .child(accent_picker_row(cx, self.accent_hue))
                    .child(toggle_row(cx, "compact", "Compact mode",
                        Some("Reduce padding throughout the app."),
                        self.compact_mode,
                        |this, _| { this.compact_mode = !this.compact_mode; this.mark_dirty(); })),
            ))
    }

    fn section_startup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(settings_card(
                "Startup", None,
                VStack::new()
                    .child(toggle_row(cx, "boot", "Launch on system startup",
                        None, self.launch_on_boot,
                        |this, _| { this.launch_on_boot = !this.launch_on_boot; this.mark_dirty(); }))
                    .child(toggle_row(cx, "min", "Start minimized",
                        Some("Open directly to the tray."),
                        self.start_minimized,
                        |this, _| { this.start_minimized = !this.start_minimized; this.mark_dirty(); })),
            ))
            .child(settings_card(
                "Updates", None,
                VStack::new()
                    .child(toggle_row(cx, "upd", "Check for updates automatically",
                        Some("Checked daily. Never installs without asking."),
                        self.auto_check_updates,
                        |this, _| { this.auto_check_updates = !this.auto_check_updates; this.mark_dirty(); }))
                    .child(row("Current version",
                        Some("You're on the latest release."),
                        div().text_sm().text_color(rgba(FG_MUTED)).child("0.9.3 (build 412)").into_any_element()))
                    .child(row("", None,
                        Button::new("check-now", "Check now")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .into_any_element())),
            ))
    }

    fn section_video(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(settings_card(
                "Encoder",
                Some("Hardware-accelerated encoders are recommended. Rekaptr auto-detected NVENC."),
                encoder_grid(cx, &self.encoder),
            ))
            .child(settings_card(
                "Output", None,
                VStack::new()
                    .child(segmented_row(cx, "res", "Resolution", &self.resolution,
                        &["3840x2160", "2560x1440", "1920x1080", "1280x720"],
                        |this, v, _| { this.resolution = v; this.mark_dirty(); }))
                    .child(segmented_row(cx, "fps", "Frame rate", &format!("{}", self.fps),
                        &["30", "60", "120", "144"],
                        |this, v, _| {
                            if let Ok(n) = v.parse::<u32>() { this.fps = n; this.mark_dirty(); }
                        })),
            ))
            .child(settings_card(
                "Quality",
                Some("CQP holds quality steady; VBR caps bitrate."),
                {
                    let mut body = VStack::new();
                    body = body.child(segmented_row(cx, "rc", "Rate control",
                        if self.rate_control == 0 { "CQP" } else { "VBR" },
                        &["CQP", "VBR"],
                        |this, v, _| { this.rate_control = if v == "VBR" { 1 } else { 0 }; this.mark_dirty(); }));
                    if self.rate_control == 0 {
                        body = body.child(stepper_row(cx, "cq", "Constant quality",
                            Some("Lower is better. 18–24 is the sweet spot for HEVC."),
                            self.cq, 0, 51, 1,
                            |this, v| { this.cq = v; this.mark_dirty(); }));
                    } else {
                        body = body.child(stepper_row(cx, "br", "Bitrate (kbps)", None,
                            self.bitrate, 1000, 100_000, 1000,
                            |this, v| { this.bitrate = v; this.mark_dirty(); }));
                    }
                    body.child(segmented_row(cx, "pre", "Preset", &self.preset.to_uppercase(),
                        &["P1", "P4", "P7"],
                        |this, v, _| { this.preset = v.to_lowercase(); this.mark_dirty(); }))
                },
            ))
            .child(settings_card(
                "Replay buffer", Some("How much past gameplay is kept in memory for instant replay."),
                VStack::new().child(stepper_row(cx, "ret", "Retention", Some("minutes"),
                    self.retention_minutes, 1, 120, 1,
                    |this, v| { this.retention_minutes = v; this.mark_dirty(); })),
            ))
            .child(self.render_advanced_video(cx))
    }

    fn render_advanced_video(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show = self.show_advanced_video;
        let header = div()
            .id("adv-video-header")
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .px_6()
            .pt_5()
            .pb_3()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.show_advanced_video = !this.show_advanced_video;
                cx.notify();
            }))
            .child(
                VStack::new()
                    .gap_1()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Advanced encoder settings"))
                    .child(div().text_xs().text_color(rgba(FG_SUBTLE)).child("NVENC tuning — change only if you know what you're doing."))
            )
            .child(
                Icon::new(IconSource::Named(
                    if show { "chevron-down" } else { "chevron-right" }.into(),
                ))
                .size(px(16.0))
                .color(rgba(FG_MUTED).into()),
            );

        let mut card = VStack::new()
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(rgba(BORDER))
            .bg(rgba(CARD))
            .child(header);

        if show {
            let mut body = VStack::new()
                .child(stepper_row(cx, "gop", "GOP size", Some("Keyframe interval in frames."),
                    self.gop_size, 0, 600, 10,
                    |this, v| { this.gop_size = v; this.mark_dirty(); }))
                .child(stepper_row(cx, "bf", "B-Frames", None,
                    self.bframes, 0, 4, 1,
                    |this, v| { this.bframes = v; this.mark_dirty(); }))
                .child(toggle_row(cx, "zl", "Zero Latency",
                    Some("Disables B-frames and minimizes latency."),
                    self.zero_latency,
                    |this, _| { this.zero_latency = !this.zero_latency; this.mark_dirty(); }))
                .child(toggle_row(cx, "la", "Lookahead",
                    Some("Improves compression at some latency cost."),
                    self.lookahead,
                    |this, _| { this.lookahead = !this.lookahead; this.mark_dirty(); }));
            if self.lookahead {
                body = body.child(stepper_row(cx, "laf", "Lookahead frames", None,
                    self.lookahead_frames, 0, 32, 1,
                    |this, v| { this.lookahead_frames = v; this.mark_dirty(); }));
            }
            body = body
                .child(toggle_row(cx, "saq", "Spatial AQ",
                    Some("Redistributes bitrate to low-detail areas."),
                    self.spatial_aq,
                    |this, _| { this.spatial_aq = !this.spatial_aq; this.mark_dirty(); }))
                .child(toggle_row(cx, "taq", "Temporal AQ",
                    Some("Improves quality in complex motion."),
                    self.temporal_aq,
                    |this, _| { this.temporal_aq = !this.temporal_aq; this.mark_dirty(); }));
            card = card.child(div().px_6().pb_5().child(body));
        }
        card
    }

    fn section_audio(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mic_card = {
            let mut body = VStack::new()
                .child(segmented_row(cx, "mic-dev", "Input device", &self.mic_device,
                    &["Shure MV7", "Default", "Blue Yeti", "HyperX QuadCast"],
                    |this, v, _| { this.mic_device = v; this.mark_dirty(); }))
                .child(toggle_row(cx, "mono", "Force mono",
                    Some("Downmix to mono before encoding."),
                    self.mic_force_mono,
                    |this, _| { this.mic_force_mono = !this.mic_force_mono; this.mark_dirty(); }))
                .child(stepper_row_f32(cx, "gain", "Gain",
                    Some("Digital gain applied before encoding."),
                    self.mic_gain_db, -20.0, 20.0, 0.5, "dB",
                    |this, v| { this.mic_gain_db = v; this.mark_dirty(); }))
                .child(row(
                    "Monitor mic",
                    Some(if self.mic_monitoring { "Listening to live mic with processing applied." } else { "Test your mic with current settings." }),
                    Button::new(
                        "monitor-mic",
                        if self.mic_monitoring { "Stop" } else { "Monitor" },
                    )
                    .variant(if self.mic_monitoring {
                        ButtonVariant::Destructive
                    } else {
                        ButtonVariant::Outline
                    })
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mic_monitoring = !this.mic_monitoring;
                        cx.notify();
                    }))
                    .into_any_element(),
                ));
            body
        };

        let mut processing = VStack::new()
            .child(toggle_row(cx, "nr", "Noise suppression (RNNoise)",
                Some("Reduces hiss, fans, keyboard. Requires mic restart."),
                self.noise_suppression,
                |this, _| { this.noise_suppression = !this.noise_suppression; this.mark_dirty(); }))
            .child(toggle_row(cx, "gate", "Noise gate",
                Some("Mute the mic below a threshold."),
                self.noise_gate,
                |this, _| { this.noise_gate = !this.noise_gate; this.mark_dirty(); }));
        if self.noise_gate {
            processing = processing.child(stepper_row_f32(cx, "gt", "Gate threshold", None,
                self.noise_gate_threshold, -80.0, 0.0, 1.0, "dB",
                |this, v| { this.noise_gate_threshold = v; this.mark_dirty(); }));
        }

        let mut compressor = VStack::new()
            .child(toggle_row(cx, "comp", "Enable compressor",
                Some("Evens out loud vs quiet speech."),
                self.compressor_enabled,
                |this, _| { this.compressor_enabled = !this.compressor_enabled; this.mark_dirty(); }));
        if self.compressor_enabled {
            compressor = compressor
                .child(stepper_row_f32(cx, "ct", "Threshold", None,
                    self.compressor_threshold, -60.0, 0.0, 1.0, "dB",
                    |this, v| { this.compressor_threshold = v; this.mark_dirty(); }))
                .child(stepper_row_f32(cx, "cr", "Ratio", None,
                    self.compressor_ratio, 1.0, 20.0, 0.5, ":1",
                    |this, v| { this.compressor_ratio = v; this.mark_dirty(); }));
        }

        let mut limiter = VStack::new()
            .child(toggle_row(cx, "lim", "Enable limiter",
                Some("Hard ceiling to prevent clipping."),
                self.limiter_enabled,
                |this, _| { this.limiter_enabled = !this.limiter_enabled; this.mark_dirty(); }));
        if self.limiter_enabled {
            limiter = limiter.child(stepper_row_f32(cx, "lt", "Threshold", None,
                self.limiter_threshold, -30.0, 0.0, 0.5, "dB",
                |this, v| { this.limiter_threshold = v; this.mark_dirty(); }));
        }

        VStack::new()
            .gap_6()
            .child(settings_card(
                "Levels",
                Some("Live monitor of the currently active sources."),
                VStack::new()
                    .gap_4()
                    .child(meter_row("System output", "Speakers (Realtek)", self.system_level))
                    .child(meter_row("Microphone", &self.mic_device, self.mic_level)),
            ))
            .child(settings_card("Microphone", None, mic_card))
            .child(settings_card("Processing & FX", None, processing))
            .child(settings_card("Compressor", None, compressor))
            .child(settings_card("Limiter", None, limiter))
    }

    fn section_hotkeys(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let capture: Vec<(u8, &'static str, String)> = vec![
            (0, "Toggle recording", self.hk_toggle_record.clone()),
            (1, "Save instant replay", self.hk_save_clip.clone()),
        ];
        let mic: Vec<(u8, &'static str, String)> = vec![
            (2, "Toggle mic mute", self.hk_toggle_mic.clone()),
            (3, "Push-to-talk", self.hk_push_to_talk.clone()),
        ];
        let markers: Vec<(u8, &'static str, String)> = vec![
            (4, "Mark flag", self.hk_marker_flag.clone()),
            (5, "Mark kill", self.hk_marker_kill.clone()),
            (6, "Mark highlight", self.hk_marker_highlight.clone()),
        ];
        let listening = self.listening_slot;

        let render_group = |items: Vec<(u8, &'static str, String)>,
                            cx: &mut Context<Self>|
         -> VStack {
            VStack::new().children(items.into_iter().map(move |(slot, label, combo)| {
                hotkey_row(cx, slot, label, combo, listening == Some(slot))
            }))
        };

        VStack::new()
            .gap_6()
            .child(settings_card(
                "Capture",
                Some("Active system-wide. Click a binding and press the new combination. Esc cancels."),
                render_group(capture, cx),
            ))
            .child(settings_card("Microphone", None, render_group(mic, cx)))
            .child(settings_card(
                "Markers",
                Some("Tag moments during recording for quick lookup later."),
                render_group(markers, cx),
            ))
    }

    fn section_storage(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.clips_gb + self.sessions_gb + self.buffer_gb;
        VStack::new()
            .gap_6()
            .child(settings_card(
                "Disk usage",
                Some("Where your captured video is stored."),
                VStack::new()
                    .gap_4()
                    .child(storage_bar(self.clips_gb, self.sessions_gb, self.buffer_gb))
                    .child(
                        HStack::new()
                            .gap_6()
                            .items_center()
                            .child(legend_swatch(PRIMARY, "Clips", format!("{:.1} GB", self.clips_gb)))
                            .child(legend_swatch(0x6366F1FF, "Sessions", format!("{:.1} GB", self.sessions_gb)))
                            .child(legend_swatch(BORDER_STRONG, "Buffer", format!("{:.1} GB", self.buffer_gb)))
                            .child(div().flex_1())
                            .child(div().text_sm().text_color(rgba(FG_MUTED)).child(format!("Total: {:.1} GB", total))),
                    ),
            ))
            .child(settings_card(
                "Retention", None,
                {
                    let mut body = VStack::new()
                        .child(row("Storage location",
                            Some("C:\\Users\\you\\Videos\\Rekaptr"),
                            Button::new("browse", "Browse…")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .into_any_element()))
                        .child(stepper_row_f32(cx, "buf", "Instant-replay buffer",
                            Some("Memory reserved for the always-on rolling buffer."),
                            self.buffer_gb, 1.0, 128.0, 1.0, "GB",
                            |this, v| { this.buffer_gb = v; this.mark_dirty(); }))
                        .child(toggle_row(cx, "auto-del", "Auto-delete old clips",
                            None, self.auto_delete,
                            |this, _| { this.auto_delete = !this.auto_delete; this.mark_dirty(); }));
                    if self.auto_delete {
                        body = body.child(stepper_row(cx, "days", "Delete clips older than",
                            Some("Favorited clips are always kept."),
                            self.auto_delete_days, 1, 365, 1,
                            |this, v| { this.auto_delete_days = v; this.mark_dirty(); }));
                    }
                    body
                },
            ))
    }

    fn section_export(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new().gap_6().child(settings_card(
            "Export defaults",
            Some("Applied to every clip export unless overridden."),
            VStack::new()
                .child(segmented_row(cx, "fmt", "Container", &self.export_format,
                    &["mp4", "mov", "mkv", "webm"],
                    |this, v, _| { this.export_format = v; this.mark_dirty(); }))
                .child(segmented_row(cx, "eq", "Quality",
                    match self.export_quality { 0 => "Low", 1 => "Medium", _ => "High" },
                    &["Low", "Medium", "High"],
                    |this, v, _| {
                        this.export_quality = match v.as_str() { "Low" => 0, "Medium" => 1, _ => 2 };
                        this.mark_dirty();
                    })),
        ))
    }

    fn section_performance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(settings_card(
                "Acceleration", None,
                VStack::new()
                    .child(toggle_row(cx, "hwd", "Hardware video decoding",
                        Some("Use NVDEC / D3D11 for preview and timeline scrubbing."),
                        self.hardware_decode,
                        |this, _| { this.hardware_decode = !this.hardware_decode; this.mark_dirty(); }))
                    .child(toggle_row(cx, "pgu", "Prefer GPU encoder when available",
                        None, self.prefer_gpu,
                        |this, _| { this.prefer_gpu = !this.prefer_gpu; this.mark_dirty(); }))
                    .child(stepper_row(cx, "threads", "Max worker threads",
                        Some("Used for segment muxing and thumbnail generation."),
                        self.max_threads, 1, 32, 1,
                        |this, v| { this.max_threads = v; this.mark_dirty(); })),
            ))
            .child(settings_card(
                "Diagnostics", None,
                VStack::new()
                    .child(row("Open log folder", None,
                        Button::new("logs", "Open")
                            .icon(IconSource::Named("folder".into()))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .into_any_element()))
                    .child(row("Run pipeline self-test",
                        Some("Verifies encoder, audio capture, and storage throughput."),
                        Button::new("selftest", "Run test")
                            .icon(IconSource::Named("activity".into()))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .into_any_element())),
            ))
    }

    fn section_about(&self) -> impl IntoElement {
        VStack::new()
            .gap_6()
            .child(
                div()
                    .w_full()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgba(BORDER))
                    .bg(rgba(CARD))
                    .p_8()
                    .child(
                        HStack::new()
                            .gap_6()
                            .items_center()
                            .child(
                                div()
                                    .size(px(72.0))
                                    .rounded_xl()
                                    .bg(rgba(PRIMARY_DIM))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named("circle-play".into()))
                                            .size(px(40.0))
                                            .color(rgba(FG).into()),
                                    ),
                            )
                            .child(
                                VStack::new()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Rekaptr"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgba(FG_MUTED))
                                            .child("Version 0.9.3 — build 412"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child("Instant-replay capture for Windows"),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("website", "Website")
                                    .variant(ButtonVariant::Outline)
                                    .icon(IconSource::Named("external-link".into())),
                            ),
                    ),
            )
            .child(settings_card(
                "Credits", None,
                VStack::new()
                    .child(row("Built with", None,
                        div().text_sm().text_color(rgba(FG_MUTED))
                            .child("GPUI · GStreamer · libmpv").into_any_element()))
                    .child(row("License", None,
                        div().text_sm().text_color(rgba(FG_MUTED))
                            .child("GPL-3.0").into_any_element())),
            ))
    }
}

// ── Generic card ────────────────────────────────────────────────────
fn settings_card(
    title: &str,
    description: Option<&str>,
    body: impl IntoElement,
) -> impl IntoElement {
    let desc: Option<SharedString> = description.map(|d| SharedString::from(d.to_string()));
    let mut header = VStack::new()
        .px_6()
        .pt_5()
        .pb_3()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        );
    if let Some(d) = desc {
        header = header.child(
            div().text_xs().text_color(rgba(FG_SUBTLE)).child(d),
        );
    }
    VStack::new()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(rgba(BORDER))
        .bg(rgba(CARD))
        .child(header)
        .child(div().px_6().pb_5().child(body))
}

fn row(
    label: &str,
    description: Option<&str>,
    control: AnyElement,
) -> impl IntoElement {
    let label_owned = label.to_string();
    let desc_owned: Option<SharedString> = description.map(|d| SharedString::from(d.to_string()));
    let label_is_empty = label_owned.is_empty();

    let mut left = VStack::new().flex_1().gap_0p5();
    if !label_is_empty {
        left = left.child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(FG))
                .child(label_owned),
        );
    }
    if let Some(d) = desc_owned {
        left = left.child(div().text_xs().text_color(rgba(FG_SUBTLE)).child(d));
    }

    HStack::new()
        .w_full()
        .py_3()
        .gap_6()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(rgba(0x2A2A3080))
        .child(left)
        .child(div().child(control))
}

// ── Control helpers ─────────────────────────────────────────────────
fn toggle_row(
    cx: &mut Context<SettingsMockup>,
    id: &'static str,
    label: &str,
    description: Option<&str>,
    value: bool,
    on_toggle: impl Fn(&mut SettingsMockup, &mut Context<SettingsMockup>) + 'static + Send + Sync,
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
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                on_toggle(this, cx);
                cx.notify();
            }),
        )
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
    cx: &mut Context<SettingsMockup>,
    id_prefix: &'static str,
    label: &str,
    current: &str,
    options: &[&'static str],
    on_pick: impl Fn(&mut SettingsMockup, String, &mut Context<SettingsMockup>) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_pick = Arc::new(on_pick);
    let current_owned = current.to_string();
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(rgba(SURFACE))
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
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        on_pick(this, opt_string.clone(), cx);
                        cx.notify();
                    }),
                )
                .child(opt.to_string()),
        );
    }
    row(label, None, group.into_any_element())
}

fn stepper_row(
    cx: &mut Context<SettingsMockup>,
    id_prefix: &'static str,
    label: &str,
    description: Option<&str>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: impl Fn(&mut SettingsMockup, i32) + 'static + Send + Sync,
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

fn stepper_row_f32(
    cx: &mut Context<SettingsMockup>,
    id_prefix: &'static str,
    label: &str,
    description: Option<&str>,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    suffix: &'static str,
    on_change: impl Fn(&mut SettingsMockup, f32) + 'static + Send + Sync,
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
                .min_w(px(84.0))
                .text_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{:.1} {}", value, suffix)),
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

// ── Encoder selection ───────────────────────────────────────────────
fn encoder_grid(cx: &mut Context<SettingsMockup>, current: &str) -> impl IntoElement {
    let encoders = [
        ("HEVC", "High quality at low bitrate. Recommended."),
        ("AV1", "Best compression. Requires RTX 40-series or newer."),
        ("H.264", "Maximum compatibility. Larger file sizes."),
    ];
    let desc = encoders
        .iter()
        .find(|(n, _)| *n == current)
        .map(|(_, d)| *d)
        .unwrap_or("");

    VStack::new()
        .gap_2()
        .pt_2()
        .child(
            HStack::new().gap_2().children(encoders.iter().map(|(name, _)| {
                let active = *name == current;
                let name_str = name.to_string();
                Button::new(
                    SharedString::from(format!("enc-{}", name)),
                    *name,
                )
                .variant(if active { ButtonVariant::Default } else { ButtonVariant::Outline })
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.encoder = name_str.clone();
                    this.mark_dirty();
                    cx.notify();
                }))
            })),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgba(FG_SUBTLE))
                .child(desc.to_string()),
        )
}

// ── Accent-hue picker ───────────────────────────────────────────────
fn accent_picker_row(cx: &mut Context<SettingsMockup>, current: u32) -> impl IntoElement {
    let swatches: [(&str, u32, u32); 6] = [
        ("violet", 262, 0x8B5CF6FF),
        ("blue", 217, 0x3B82F6FF),
        ("emerald", 160, 0x10B981FF),
        ("amber", 38, 0xF59E0BFF),
        ("rose", 350, 0xF43F5EFF),
        ("slate", 215, 0x64748BFF),
    ];
    let ctl = HStack::new().gap_2().items_center().children(
        swatches.into_iter().map(|(name, hue, color)| {
            let active = hue == current;
            div()
                .id(SharedString::from(format!("acc-{}", name)))
                .size(px(28.0))
                .rounded_full()
                .bg(rgba(color))
                .border_2()
                .border_color(if active { rgba(FG) } else { rgba(0x00000000) })
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    this.accent_hue = hue;
                    this.mark_dirty();
                    cx.notify();
                }))
        }),
    );
    row("Accent colour", None, ctl.into_any_element())
}

// ── Meter (fake live VU) ────────────────────────────────────────────
fn meter_row(label: &str, detail: &str, level: f32) -> impl IntoElement {
    let level = level.clamp(0.0, 1.0);
    HStack::new()
        .w_full()
        .gap_4()
        .items_center()
        .py_2()
        .child(
            VStack::new()
                .w(px(180.0))
                .gap_0p5()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label.to_string()))
                .child(div().text_xs().text_color(rgba(FG_SUBTLE)).child(detail.to_string())),
        )
        .child(
            div()
                .flex_1()
                .h(px(10.0))
                .rounded_full()
                .bg(rgba(SURFACE))
                .border_1()
                .border_color(rgba(BORDER))
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(level))
                        .bg(rgba(PRIMARY))
                        .rounded_full(),
                ),
        )
        .child(
            div()
                .w(px(56.0))
                .text_right()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba(FG_MUTED))
                .child(format!("{} dB", (level * 60.0 - 60.0).round() as i32)),
        )
}

// ── Hotkey row ──────────────────────────────────────────────────────
fn hotkey_row(
    cx: &mut Context<SettingsMockup>,
    slot: u8,
    label: &'static str,
    combo: String,
    listening: bool,
) -> impl IntoElement {
    let chip = div()
        .id(SharedString::from(format!("hk-{}", slot)))
        .px_3()
        .py_1p5()
        .min_w(px(160.0))
        .rounded_md()
        .border_1()
        .border_color(if listening { rgba(PRIMARY) } else { rgba(BORDER) })
        .bg(if listening { rgba(0x5B3FA840) } else { rgba(SURFACE) })
        .text_color(if listening { rgba(FG) } else { rgba(FG_MUTED) })
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_center()
        .cursor_pointer()
        .hover(|s| s.border_color(rgba(BORDER_STRONG)))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.listening_slot = Some(slot);
            cx.notify();
        }))
        .child(if listening { "Press keys…".to_string() } else { combo });

    row(label, None, chip.into_any_element())
}

// ── Storage stacked bar ─────────────────────────────────────────────
fn storage_bar(clips: f32, sessions: f32, buffer: f32) -> impl IntoElement {
    let total = (clips + sessions + buffer).max(0.001);
    div()
        .w_full()
        .h(px(18.0))
        .rounded_full()
        .bg(rgba(SURFACE))
        .border_1()
        .border_color(rgba(BORDER))
        .flex()
        .flex_row()
        .child(
            div()
                .h_full()
                .w(relative(clips / total))
                .bg(rgba(PRIMARY))
                .rounded_l_full(),
        )
        .child(
            div()
                .h_full()
                .w(relative(sessions / total))
                .bg(rgba(0x6366F1FF)),
        )
        .child(
            div()
                .h_full()
                .w(relative(buffer / total))
                .bg(rgba(BORDER_STRONG))
                .rounded_r_full(),
        )
}

fn legend_swatch(color: u32, label: &str, value: String) -> impl IntoElement {
    HStack::new()
        .gap_2()
        .items_center()
        .child(div().size(px(10.0)).rounded_sm().bg(rgba(color)))
        .child(
            VStack::new()
                .child(div().text_xs().text_color(rgba(FG_MUTED)).child(label.to_string()))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                ),
        )
}

// ── main ────────────────────────────────────────────────────────────
fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Settings Redesign Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(SettingsMockup::new),
        )
        .unwrap();
    });
}
