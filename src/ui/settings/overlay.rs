use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use crate::overlay::{OverlayEvent, OverlaySettings};
use super::{settings_card, settings_row, settings_toggle};

/// Load config, mutate the overlay section, persist, and push the change to the
/// live overlay window so opacity / toggles update without a restart.
fn apply_overlay(this: &RekaptrWorkspace, f: impl FnOnce(&mut OverlaySettings)) {
    let mut config = crate::config::AppConfig::load();
    f(&mut config.overlay);
    config.save();
    crate::overlay::send(
        &this.app_state,
        OverlayEvent::ConfigChanged(config.overlay.clone()),
    );
}

impl RekaptrWorkspace {
    pub(crate) fn render_settings_overlay(
        &self,
        theme: &Theme,
        view_handle: &WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let ov = config.overlay.clone();
        let displays = cx.displays();

        // ── Master enable ──────────────────────────────────────────────
        let enable_card = settings_card(
            theme,
            "In-game overlay",
            Some("Summon an overlay on top of your game (default F8) to save replays, control recording, and drop markers. Works over borderless / windowed-fullscreen (not exclusive fullscreen)."),
            VStack::new().child(settings_row(
                theme,
                "Enable overlay",
                Some("Master switch. Summon it in-game with the Toggle Overlay hotkey."),
                settings_toggle("ov-enabled", ov.enabled, view_handle.clone(), |this, cx| {
                    apply_overlay(this, |o| o.enabled = !o.enabled);
                    cx.notify();
                }),
            )),
        );

        // ── Appearance ─────────────────────────────────────────────────
        let op_pct = (ov.opacity * 100.0).round() as i32;
        let op_key = if op_pct <= 77 {
            "0.70"
        } else if op_pct <= 90 {
            "0.85"
        } else if op_pct < 98 {
            "0.95"
        } else {
            "1.00"
        };

        let mut appearance = VStack::new()
            .child(super::ss_segmented_row(
                theme, cx, "ov-op", "Panel opacity", op_key,
                &[("70%", "0.70"), ("85%", "0.85"), ("95%", "0.95"), ("100%", "1.00")],
                |this, v| apply_overlay(this, |o| {
                    if let Ok(f) = v.parse::<f32>() { o.opacity = f; }
                }),
            ));

        // Monitor picker — only meaningful with more than one display.
        if displays.len() > 1 {
            let current = ov.monitor;
            let mut row = HStack::new().gap_1().flex_wrap();
            for i in 0..displays.len() {
                let selected = current == Some(i) || (current.is_none() && i == 0);
                let vh = view_handle.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("ov-mon-{i}")), format!("Monitor {}", i + 1))
                        .variant(if selected { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                        .size(ButtonSize::Sm)
                        .on_click(move |_, _, cx| {
                            let _ = vh.update(cx, |this, cx| {
                                apply_overlay(this, |o| o.monitor = Some(i));
                                cx.notify();
                            });
                        }),
                );
            }
            appearance = appearance.child(settings_row(
                theme,
                "Monitor",
                Some("Which display the overlay appears on. Applies on next launch."),
                row,
            ));
        }

        let appearance_card = settings_card(
            theme,
            "Appearance",
            Some("Opacity updates live; monitor changes apply on next launch."),
            appearance,
        );

        // ── What to show ───────────────────────────────────────────────
        let show_card = settings_card(
            theme,
            "What to show",
            None,
            VStack::new()
                .child(settings_row(
                    theme,
                    "Recording status badge",
                    Some("Show the REC / elapsed-time badge in the overlay header."),
                    settings_toggle("ov-rec", ov.show_recording_indicator, view_handle.clone(), |this, cx| {
                        apply_overlay(this, |o| o.show_recording_indicator = !o.show_recording_indicator);
                        cx.notify();
                    }),
                ))
                .child(settings_row(
                    theme,
                    "Clip confirmations",
                    Some("Show a \"Replay saved\" confirmation after saving."),
                    settings_toggle("ov-clip", ov.show_clip_confirmations, view_handle.clone(), |this, cx| {
                        apply_overlay(this, |o| o.show_clip_confirmations = !o.show_clip_confirmations);
                        cx.notify();
                    }),
                ))
                .child(settings_row(
                    theme,
                    "Marker buttons",
                    Some("Show the flag / kill / death / highlight buttons."),
                    settings_toggle("ov-mark", ov.show_markers, view_handle.clone(), |this, cx| {
                        apply_overlay(this, |o| o.show_markers = !o.show_markers);
                        cx.notify();
                    }),
                )),
        );

        // ── Anti-cheat & compatibility (info only) ─────────────────────
        let note = |t: &str| {
            div()
                .text_xs()
                .text_color(theme.tokens.muted_foreground)
                .child(t.to_string())
        };
        let info_card = settings_card(
            theme,
            "Anti-cheat & compatibility",
            None,
            VStack::new()
                .gap_2()
                .child(note("The overlay is a separate window — it never injects into or hooks the game, so it stays clear of what anti-cheats detect. It is also excluded from your own recordings."))
                .child(note("For anti-cheat-sensitive titles (e.g. Valorant/Vanguard, FACEIT) the overlay is off by default. You can force it on or off for any game from that source's Advanced settings → overlay override."))
                .child(note("It cannot draw over true exclusive-fullscreen games; use borderless / windowed-fullscreen mode for those.")),
        );

        VStack::new()
            .gap_6()
            .child(enable_card)
            .child(appearance_card)
            .child(show_card)
            .child(info_card)
    }
}
