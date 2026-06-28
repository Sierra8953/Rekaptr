//! Free helpers, enums and constants shared across the UI views, split out of
//! `ui/mod.rs` so that module stays the workspace shell + `Render` impl. These
//! are re-exported from `ui` (`pub use shared::*`), so call sites keep using the
//! stable `crate::ui::*` paths.

use super::RekaptrWorkspace;
use adabraka_ui::prelude::*;
use gpui::{Corners, Hsla, ObjectFit, StyledImage};

/// Canonical icon name for an audio track's source type. Shared across the
/// timeline, export dialog and source settings so the same track type always
/// shows the same glyph.
pub fn audio_track_icon(source_type: &str) -> &'static str {
    match source_type {
        "Mic" => "mic",
        "App" => "gamepad-2",
        _ => "volume-2",
    }
}

/// Friendly display name for an audio track. App-routed tracks are named after
/// the process(es) they capture (e.g. `Discord.exe` → "Discord"); System and Mic
/// tracks get human labels. An explicit user-chosen name (anything other than the
/// default `Track N`) always wins, so renaming a track is never overridden.
pub fn audio_track_display_name(track: &crate::config::AudioRouting) -> String {
    if !is_default_track_name(&track.name) {
        return track.name.clone();
    }
    match track.source_type.as_str() {
        "System" => "System".to_string(),
        "Mic" => "Microphone".to_string(),
        "App" => match track.app_targets.as_slice() {
            [] => "App".to_string(),
            [one] => prettify_process_name(one),
            [first, rest @ ..] => format!("{} +{}", prettify_process_name(first), rest.len()),
        },
        _ => track.name.clone(),
    }
}

/// True for the auto-generated placeholder names (`Track 1`, `Track 2`, …) so we
/// know a track hasn't been deliberately renamed.
fn is_default_track_name(name: &str) -> bool {
    name.strip_prefix("Track ")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// Turn a process / executable name into a friendly label: drop any path and the
/// `.exe` extension, map well-known processes to their product name, otherwise
/// capitalize the first letter.
pub fn prettify_process_name(proc: &str) -> String {
    let stem = proc.rsplit(['/', '\\']).next().unwrap_or(proc);
    let stem = stem
        .strip_suffix(".exe")
        .or_else(|| stem.strip_suffix(".EXE"))
        .unwrap_or(stem);

    // Well-known processes whose executable name isn't a nice product name.
    match stem.to_ascii_lowercase().as_str() {
        "msedge" => return "Microsoft Edge".to_string(),
        "chrome" => return "Chrome".to_string(),
        "firefox" => return "Firefox".to_string(),
        "brave" => return "Brave".to_string(),
        "opera" | "opera_gx" => return "Opera".to_string(),
        "discord" => return "Discord".to_string(),
        "spotify" => return "Spotify".to_string(),
        "slack" => return "Slack".to_string(),
        "teams" | "ms-teams" => return "Microsoft Teams".to_string(),
        "zoom" => return "Zoom".to_string(),
        "vlc" => return "VLC".to_string(),
        "obs64" | "obs32" | "obs" => return "OBS".to_string(),
        "steam" => return "Steam".to_string(),
        "pioneergame-d" => return "ARC Raiders".to_string(),
        "rustclient" => return "Rust".to_string(),
        _ => {}
    }

    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => stem.to_string(),
    }
}

/// Per-track accent colors for the audio mixer, shared so a track's dot, meter
/// fill and any other per-track UI all read the same color.
pub const TRACK_COLORS: [u32; 6] = [0x22d3ee, 0x8b5cf6, 0x4ade80, 0xf472b6, 0x60a5fa, 0xfbbf24];

pub fn track_color(i: usize) -> gpui::Hsla {
    gpui::rgb(TRACK_COLORS[i % TRACK_COLORS.len()]).into()
}

/// Shared pill toggle switch. `small` renders the compact 28×16 variant used in
/// dense rows; otherwise the standard 40×22 variant. Calling `on_toggle` and
/// `cx.notify()` is handled here so every toggle behaves and looks identical.
pub(crate) fn toggle_switch(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    id: impl Into<ElementId>,
    value: bool,
    small: bool,
    on_toggle: impl Fn(&mut RekaptrWorkspace) + 'static + Send + Sync,
) -> impl IntoElement {
    let (w, h, thumb, on_left) = if small {
        (28.0, 16.0, 12.0, 14.0)
    } else {
        (40.0, 22.0, 18.0, 20.0)
    };
    let fg = theme.tokens.foreground;
    div()
        .id(id.into())
        .w(px(w))
        .h(px(h))
        .rounded_full()
        .relative()
        .cursor_pointer()
        .bg(if value { theme.tokens.primary } else { theme.tokens.border })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            on_toggle(this);
            cx.notify();
        }))
        .child(
            div()
                .absolute()
                .top(px(2.0))
                .left(if value { px(on_left) } else { px(2.0) })
                .size(px(thumb))
                .rounded_full()
                .bg(fg),
        )
}

/// A rounded image tile whose image can never spill past the corners.
///
/// The image paints with `ObjectFit::Fill` at exactly the tile bounds and is
/// rounded to the same `radius`, so its rounded corners always coincide with the
/// tile's. This is the reliable way to round-clip an image in this GPUI fork:
/// the content mask is a plain rectangle (it can't round-clip children), and
/// `ObjectFit::Cover` paints an *oversized* quad whose rounded corners fall
/// outside the tile — leaving the tile's rounded corners filled by the square
/// part of the image (the classic corner "spill"). `Fill` keeps the painted quad
/// equal to the tile, so the rounding lands on the visible corners.
///
/// Caveat: `Fill` stretches to the tile, so the source should roughly match the
/// tile's aspect ratio (true for clip thumbnails and Steam covers) or it will
/// distort. Fills its parent (`size_full`); the caller sizes the wrapper and
/// layers any overlays (scrims, chips, action buttons) on as children.
///
/// `radius` is per-corner so a tile used as a card *header* (image only at the
/// top, a footer below) can round its top corners and leave the bottom square —
/// rounding corners that don't sit on a card edge would cut visible notches.
///
/// `placeholder` fills the tile until/unless an image is present (a muted card
/// color, a per-item tint, etc.). It must live on the rounded element so the
/// placeholder itself can't spill the corners.
pub fn thumbnail(
    image: Option<SharedString>,
    radius: Corners<Pixels>,
    placeholder: Hsla,
) -> Div {
    // Apply the same radii to the frame and the image so the (Fill-sized) image
    // quad's rounded corners land exactly on the tile's.
    let round = |el: Div| {
        el.rounded_tl(radius.top_left)
            .rounded_tr(radius.top_right)
            .rounded_bl(radius.bottom_left)
            .rounded_br(radius.bottom_right)
    };
    round(div())
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(placeholder)
        .when_some(image, |this, src| {
            this.child(
                img(src)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded_tl(radius.top_left)
                    .rounded_tr(radius.top_right)
                    .rounded_bl(radius.bottom_left)
                    .rounded_br(radius.bottom_right)
                    .object_fit(ObjectFit::Fill),
            )
        })
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Dashboard,
    Settings,
    Clips,
    Teams,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Startup,
    Video,
    Audio,
    Hotkeys,
    Overlay,
    Storage,
    Export,
    About,
}

pub struct SettingsNavGroup {
    pub title: &'static str,
    pub items: &'static [SettingsTab],
}

pub const SETTINGS_NAV: &[SettingsNavGroup] = &[
    SettingsNavGroup { title: "GENERAL", items: &[SettingsTab::General, SettingsTab::Startup] },
    SettingsNavGroup { title: "CAPTURE", items: &[SettingsTab::Video, SettingsTab::Audio, SettingsTab::Hotkeys, SettingsTab::Overlay] },
    SettingsNavGroup { title: "STORAGE", items: &[SettingsTab::Storage, SettingsTab::Export] },
    SettingsNavGroup { title: "SYSTEM", items: &[SettingsTab::About] },
];

impl SettingsTab {
    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "Behavior",
            SettingsTab::Startup => "Startup",
            SettingsTab::Video => "Video",
            SettingsTab::Audio => "Audio",
            SettingsTab::Hotkeys => "Hotkeys",
            SettingsTab::Overlay => "Overlay",
            SettingsTab::Storage => "Storage",
            SettingsTab::Export => "Export",
            SettingsTab::About => "About",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            SettingsTab::General => "sliders-horizontal",
            SettingsTab::Startup => "power",
            SettingsTab::Video => "video",
            SettingsTab::Audio => "mic",
            SettingsTab::Hotkeys => "keyboard",
            SettingsTab::Overlay => "layout-dashboard",
            SettingsTab::Storage => "hard-drive",
            SettingsTab::Export => "scissors",
            SettingsTab::About => "info",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            SettingsTab::General | SettingsTab::Startup => "General",
            SettingsTab::Video | SettingsTab::Audio | SettingsTab::Hotkeys | SettingsTab::Overlay => "Capture",
            SettingsTab::Storage | SettingsTab::Export => "Storage",
            SettingsTab::About => "System",
        }
    }
}
