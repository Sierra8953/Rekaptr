//! Dashboard audio mixer pane: per-track volume/mute/solo controls shown to
//! the right of the preview.

use super::*;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    // ── audio mixer (right of the preview) ─────────────────────────────────────
    pub(super) fn render_mixer(&mut self, enabled_tracks: &[AudioRouting], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let count = enabled_tracks.len();
        let source = self.selected_source.clone();

        // dB from a 0..150 volume value (100 == unity / 0 dB).
        let db_text = |v: f64| -> String {
            if v <= 0.5 { return "-∞".to_string(); }
            format!("{:.1}", 20.0 * (v / 100.0).log10())
        };

        let master = self.mixer.master_slider.clone();
        let master_val = master.as_ref().map(|s| s.read(cx).effective_value()).unwrap_or(1.0);

        // ── per-track rows ──
        let mut rows = div().id("mixer-rows").flex_1().w_full().overflow_y_scroll().flex().flex_col();
        if count == 0 {
            rows = rows.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .p_6()
                    .child(Icon::new("volume-x").size(px(28.0)).color(theme.tokens.muted_foreground))
                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("No audio tracks for this source")),
            );
        } else {
            for (i, track) in enabled_tracks.iter().enumerate() {
                let color = crate::ui::track_color(i);
                let icon = crate::ui::audio_track_icon(&track.source_type);
                let slider = self.mixer.sliders.get(i).cloned();
                let muted = self.mixer.muted.get(i).copied().unwrap_or(false);
                let soloed = self.mixer.solo.get(i).copied().unwrap_or(false);
                let vol = self.mixer.volumes.get(i).copied().unwrap_or(100.0);
                let db = if muted { "mute".to_string() } else { db_text(vol) };

                let mut row = div()
                    .w_full()
                    .h(px(44.0))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.tokens.border)
                    .hover(|s| s.bg(theme.tokens.accent))
                    // label cluster (dot + icon + name). 8px gap to the slider on
                    // the left is kept; the slider→cluster gap on the right is
                    // tightened separately so the slider extends further right.
                    .child(
                        div()
                            .w(px(128.0))
                            .flex_shrink_0()
                            .mr(px(8.0))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(8.0)).rounded_full().flex_shrink_0().bg(color))
                            .child(Icon::new(icon).size(px(13.0)).color(if muted { theme.tokens.muted_foreground } else { theme.tokens.muted_foreground }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if muted { theme.tokens.muted_foreground } else { theme.tokens.foreground })
                                    .child(crate::ui::audio_track_display_name(track)),
                            ),
                    );
                // meter-style volume bar
                if let Some(s) = slider {
                    row = row.child(div().flex_1().min_w(px(0.0)).child(s));
                } else {
                    row = row.child(div().flex_1().min_w(px(0.0)));
                }
                // Right cluster: dB readout (right-aligned, snug) + M/S buttons.
                // Tight 5px gaps bring the dB number close to the mute button and
                // let the slider's flex_1 track extend further to the right.
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .flex_shrink_0()
                        .ml(px(6.0))
                        // dB readout
                        .child(
                            div()
                                .w(px(34.0))
                                .text_xs()
                                .text_right()
                                .font_family("Consolas")
                                .text_color(theme.tokens.muted_foreground)
                                .child(db),
                        )
                        // Mute (M)
                        .child(mixer_tag_btn(i, "M", muted, theme.tokens.destructive, cx.listener(move |this: &mut Self, _ev: &MouseDownEvent, _, cx| {
                            this.toggle_mixer_mute(i, cx);
                        })))
                        // Solo (S)
                        .child(mixer_tag_btn(i, "S", soloed, theme.tokens.primary, cx.listener(move |this: &mut Self, _ev: &MouseDownEvent, _, cx| {
                            this.toggle_mixer_solo(i, cx);
                        }))),
                );
                rows = rows.child(row);
            }
        }

        div()
            .w(px(380.0))
            .flex_none()
            .h_full()
            .bg(theme.tokens.card)
            .rounded_xl()
            .shadow_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            // header
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.tokens.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Icon::new("sliders-horizontal").size(px(15.0)).color(theme.tokens.muted_foreground))
                            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("AUDIO MIXER")),
                    )
                    .when_some(source.clone(), |el, src| {
                        el.child(
                            div()
                                .id("mixer-settings-btn")
                                .size(px(24.0))
                                .rounded_md()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.tokens.accent))
                                .child(Icon::new("settings").size(px(15.0)).color(theme.tokens.muted_foreground))
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.open_source_settings(&src, cx);
                                })),
                        )
                    }),
            )
            // master row
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.background)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(64.0)).flex_shrink_0().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("Master"))
                    .child(div().flex_1().min_w(px(0.0)).when_some(master, |el, m| el.child(m)))
                    .child(
                        div()
                            .w(px(40.0))
                            .flex_shrink_0()
                            .text_xs()
                            .font_family("Consolas")
                            .text_color(theme.tokens.muted_foreground)
                            .child(db_text(master_val as f64 * 100.0)),
                    ),
            )
            .child(rows)
            // footer
            .child(
                div()
                    .id("mixer-add-source")
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(theme.tokens.border)
                    // Round the bottom corners so the hover background follows the
                    // card's rounded_xl edge instead of poking out square (gpui
                    // clips to a rectangle, not the rounded silhouette).
                    .rounded_b_xl()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.accent))
                    .child(Icon::new("plus").size(px(14.0)).color(theme.tokens.muted_foreground))
                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.muted_foreground).child("Add audio source"))
                    .when_some(source, |el, src| {
                        el.on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            this.open_source_settings(&src, cx);
                        }))
                    }),
            )
    }
}

/// Small "M" / "S" tag toggle used in the mixer rows. Filled with the accent
/// color when active, subtle otherwise.
fn mixer_tag_btn(
    i: usize,
    label: &'static str,
    active: bool,
    accent: Hsla,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = use_theme();
    let (bg, fg, border) = if active {
        (accent.opacity(0.18), accent, accent.opacity(0.5))
    } else {
        (gpui::white().opacity(0.03), theme.tokens.muted_foreground, theme.tokens.border)
    };
    div()
        .id(SharedString::from(format!("mix-{}-{}", label, i)))
        .w(px(20.0))
        .h(px(18.0))
        .flex_shrink_0()
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(bg)
        .border_1()
        .border_color(border)
        .hover(|s| s.bg(accent.opacity(0.12)))
        .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(fg).child(label))
        .on_mouse_down(MouseButton::Left, on_down)
}
