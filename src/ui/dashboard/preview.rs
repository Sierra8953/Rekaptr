//! Dashboard preview pane: the video preview, scrub handling, and the
//! playback transport strip (timeline + scrubber).

use super::*;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    // ── preview ──────────────────────────────────────────────────────────────
    pub(super) fn render_preview_pane(&mut self, is_recording: bool, rec_elapsed: u64, _cx: &mut Context<Self>) -> AnyElement {
        let theme = use_theme();

        let video_element = match &self.video_source {
            Some(v) => div()
                .relative()
                .w_full()
                .h_full()
                .bg(rgb(0x000000))
                .child(video(v.clone()).id("main-video"))
                .into_any_element(),
            None => div()
                .w_full()
                .h_full()
                .bg(rgb(0x000000))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(Icon::new("info").size(px(40.0)).color(theme.tokens.muted_foreground))
                        .child(
                            div()
                                .text_color(theme.tokens.muted_foreground)
                                .font_weight(FontWeight::MEDIUM)
                                .child("Select a source to begin previewing"),
                        ),
                )
                .into_any_element(),
        };

        div()
            .relative()
            .flex_1()
            .h_full()
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(theme.tokens.border)
            .child(if self.is_loading_video {
                div()
                    .w_full()
                    .h_full()
                    .bg(rgb(0x000000))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_color(theme.tokens.muted_foreground).child("Scanning recording segments..."))
                    .into_any_element()
            } else {
                video_element
            })
            .when(is_recording, |el| {
                let h = rec_elapsed / 3600;
                let m = (rec_elapsed % 3600) / 60;
                let s = rec_elapsed % 60;
                let time_str = if h > 0 {
                    format!("{:02}:{:02}:{:02}", h, m, s)
                } else {
                    format!("{:02}:{:02}", m, s)
                };
                el.child(
                    div()
                        .absolute()
                        .top_4()
                        .left_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(gpui::rgba(0x0a0a0c_cc))
                        .border_1()
                        .border_color(gpui::rgba(0xffffff_0d))
                        .child(div().size(px(8.0)).rounded_full().bg(theme.tokens.destructive))
                        .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.destructive).child("REC"))
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(gpui::white()).child(time_str)),
                )
            })
            .into_any_element()
    }

    /// Map a window-space x over the preview seek bar to a 0..1 position, update
    /// the scrub indicator, and seek the preview video there.
    fn scrub_preview_to(&mut self, mouse_x: Pixels, duration: f64) {
        let bounds = self.preview_bar_bounds;
        let width = bounds.size.width;
        if width <= px(0.0) {
            return;
        }
        let rel = (mouse_x - bounds.left()).clamp(px(0.0), width);
        let progress = (f32::from(rel) / f32::from(width)).clamp(0.0, 1.0);
        self.scrubbing_progress = progress;
        if let Some(v) = &self.video_source {
            let target = progress as f64 * duration;
            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
        }
    }
    // ── transport strip ────────────────────────────────────────────────────────
    pub(super) fn render_transport_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let is_recording = self.app_state.recording.phase.lock().is_recording();
        let is_paused = self.video_source.as_ref().map_or(true, |v| v.paused());
        let has_clip_range = self.clip_start >= 0.0;

        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64())
        } else {
            (0.0, 1.0)
        };
        let show_hours = duration >= 3600.0;
        let fmt = move |s: f64| {
            let total = s.max(0.0) as u64;
            let h = total / 3600;
            let m = (total % 3600) / 60;
            let sec = total % 60;
            if show_hours { format!("{:01}:{:02}:{:02}", h, m, sec) } else { format!("{:01}:{:02}", m, sec) }
        };
        let time_display: SharedString = format!("{} / {}", fmt(position), fmt(duration)).into();
        let clip_in_text: SharedString = if self.clip_start >= 0.0 { fmt(self.clip_start).into() } else { "--:--".into() };
        let clip_out_text: SharedString = if self.clip_end >= 0.0 { fmt(self.clip_end).into() } else { "--:--".into() };

        let vline = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme.tokens.border);

        // ── seek track (lives in the transport box now) ──
        let has_video = self.video_source.is_some();
        let progress = if self.is_scrubbing {
            self.scrubbing_progress
        } else if duration > 0.0 {
            (position / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let view = cx.entity().downgrade();
        let seek_dur = duration;
        let fill_grad = gpui::linear_gradient(
            90.0,
            gpui::linear_color_stop(hsla(258.0 / 360.0, 0.55, 0.52, 1.0), 0.0),
            gpui::linear_color_stop(hsla(258.0 / 360.0, 0.60, 0.44, 1.0), 1.0),
        );

        // clip in/out positions (0..1)
        let clip_in_prog = if self.clip_start >= 0.0 && duration > 0.0 { (self.clip_start / duration).clamp(0.0, 1.0) as f32 } else { -1.0 };
        let clip_out_prog = if self.clip_end >= 0.0 && duration > 0.0 { (self.clip_end / duration).clamp(0.0, 1.0) as f32 } else { -1.0 };
        let in_color = hsla(142.0 / 360.0, 0.71, 0.45, 1.0);
        let out_color = hsla(346.0 / 360.0, 0.84, 0.61, 1.0);

        const RULER_H: f32 = 14.0;

        // ── time ruler with evenly spaced tick marks ──
        let mut ruler = div()
            .relative()
            .w_full()
            .h(px(RULER_H))
            .bg(theme.tokens.card)
            .border_b_1()
            .border_color(theme.tokens.border);
        for n in 1..20 {
            let f = n as f32 / 20.0;
            let major = n % 5 == 0;
            ruler = ruler.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(relative(f))
                    .w(px(1.0))
                    .h(px(if major { 8.0 } else { 4.0 }))
                    .bg(gpui::rgba(if major { 0xffffff_26 } else { 0xffffff_12 })),
            );
        }

        // ── track lane: flat rectangle, played fill, clip range + in/out ticks ──
        let mut lane = div()
            .relative()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .bg(theme.tokens.background)
            // Round the bottom-left corner so the fill follows the strip's rounded
            // silhouette — this fork's overflow_hidden clips to a rectangle, so the
            // fill must carry the radius itself. The top stays square since the fill
            // butts up against the ruler, not the top edge. Once the fill reaches the
            // right edge (fully played) its bottom-right corner must round too, or
            // its square corner pokes past the strip's rounded border.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(relative(progress))
                    .rounded_bl_md()
                    .when(progress >= 0.999, |d| d.rounded_br_md())
                    .bg(fill_grad),
            );
        if clip_in_prog >= 0.0 {
            let end = if clip_out_prog >= 0.0 { clip_out_prog } else { 1.0 };
            let w = (end - clip_in_prog).max(0.0);
            lane = lane
                .child(div().absolute().top_0().bottom_0().left(relative(clip_in_prog)).w(relative(w)).bg(theme.tokens.primary.opacity(0.18)))
                .child(div().absolute().top_0().bottom_0().left(relative(clip_in_prog)).w(px(2.0)).bg(in_color));
        }
        if clip_out_prog >= 0.0 {
            lane = lane.child(div().absolute().top_0().bottom_0().left(relative(clip_out_prog)).w(px(2.0)).bg(out_color));
        }

        // Ruler + lane stacked into one bordered, lightly-rounded strip.
        let base = div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .rounded_md()
            .overflow_hidden()
            .border_1()
            .border_color(theme.tokens.border)
            .child(ruler)
            .child(lane);

        let mut timeline = div()
            .id("transport-seek")
            .relative()
            .w_full()
            .h(px(36.0))
            // capture window-space bounds so a click maps to a 0..1 position
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds, _, _, cx| {
                        let _ = view.update(cx, |this, _| {
                            this.preview_bar_bounds = bounds;
                        });
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .child(base);

        // ── markers: a vertical line down the lane + a clickable flag on the
        // ruler. Left-click seeks to the marker, right-click removes it. ──
        if has_video && duration > 0.0 {
            let marker_source = self.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
            let markers = self.app_state.markers_for(&marker_source);
            for (i, m) in markers.iter().enumerate() {
                let prog = (m.time_secs / duration).clamp(0.0, 1.0) as f32;
                let (h, s, l, a) = m.kind.color_hsla();
                let color = hsla(h, s, l, a);
                let mtime = m.time_secs;
                // decorative line (no hitbox → clicks fall through to the seek bar)
                timeline = timeline.child(
                    div().absolute().top(px(RULER_H)).bottom_0().left(relative(prog)).w(px(1.0)).bg(color.opacity(0.7)),
                );
                // clickable flag on the ruler — a small badge carrying the
                // marker's kind icon so kills/deaths/etc. are distinguishable at a
                // glance (mirrors the marker toolbar), not just by color.
                timeline = timeline.child(
                    div()
                        .id(SharedString::from(format!("tl-mk-{}", i)))
                        .absolute()
                        .top(px(-1.0))
                        .left(relative(prog))
                        .ml(px(-8.0))
                        .w(px(16.0))
                        .h(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .child(Icon::new(m.kind.icon_name()).size(px(13.0)).color(color))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(v) = &this.video_source {
                                let _ = v.seek(std::time::Duration::from_secs_f64(mtime), false);
                            }
                            cx.notify();
                        }))
                        .on_mouse_down(MouseButton::Right, cx.listener(move |this: &mut Self, _, _, cx| {
                            cx.stop_propagation();
                            this.remove_marker(i, cx);
                        })),
                );
            }
        }

        // ── playhead: full-height line with a handle at the top ──
        if has_video {
            timeline = timeline
                .child(div().absolute().top_0().bottom_0().left(relative(progress)).ml(px(-1.0)).w(px(2.0)).bg(gpui::white()))
                .child(
                    div()
                        .absolute()
                        .top(px(-2.0))
                        .left(relative(progress))
                        .ml(px(-5.0))
                        .w(px(10.0))
                        .h(px(9.0))
                        .rounded_sm()
                        .bg(gpui::white())
                        .shadow_sm(),
                )
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, ev: &MouseDownEvent, _, cx| {
                    this.is_scrubbing = true;
                    this.scrub_preview_to(ev.position.x, seek_dur);
                    cx.notify();
                }));
        }

        div()
            .w_full()
            .flex_shrink_0()
            .bg(theme.tokens.card)
            .rounded_xl()
            .border_1()
            .border_color(theme.tokens.border)
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            // continue / finish the scrub anywhere within the transport box
            .on_mouse_move(cx.listener(move |this: &mut Self, ev: &MouseMoveEvent, _, cx| {
                if this.is_scrubbing {
                    this.scrub_preview_to(ev.position.x, seek_dur);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this: &mut Self, _ev: &MouseUpEvent, _, cx| {
                if this.is_scrubbing {
                    this.is_scrubbing = false;
                    cx.notify();
                }
            }))
            // controls row (on top)
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    // left: transport cluster
                    .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded_lg()
                    .bg(theme.tokens.background)
                    .border_1()
                    .border_color(theme.tokens.border)
                    // record
                    .child(
                        div()
                            .id("btn-rec")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(30.0))
                            .h(px(30.0))
                            .rounded_md()
                            .cursor_pointer()
                            .when(!is_recording, |el| el.hover(|s| s.bg(hsla(0.0, 0.7, 0.5, 0.15))))
                            .when(is_recording, |el| el.bg(hsla(0.0, 0.7, 0.5, 0.2)).border_1().border_color(hsla(0.0, 0.7, 0.5, 0.4)))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, window, cx| {
                                this.toggle_recording(window, cx);
                            }))
                            .child(if is_recording {
                                Icon::new("square").size(px(13.0)).color(theme.tokens.destructive).into_any_element()
                            } else {
                                div().size(px(13.0)).rounded_full().bg(theme.tokens.destructive).into_any_element()
                            }),
                    )
                    .child(vline())
                    // skip back
                    .child(
                        div()
                            .id("btn-back")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(30.0))
                            .h(px(30.0))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.tokens.accent))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, _| {
                                if let Some(v) = &this.video_source {
                                    let new_pos = (v.position().as_secs_f64() - 10.0).max(0.0);
                                    let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                }
                            }))
                            .child(Icon::new("skip-back").size(px(16.0)).color(theme.tokens.foreground)),
                    )
                    // play/pause
                    .child(
                        div()
                            .id("btn-play")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(36.0))
                            .h(px(36.0))
                            .rounded_lg()
                            .bg(theme.tokens.primary)
                            .cursor_pointer()
                            .hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0)))
                            .active(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.53, 1.0)))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                this.toggle_play_pause(cx);
                            }))
                            .child(Icon::new(if is_paused { "play" } else { "pause" }).size(px(16.0)).color(theme.tokens.primary_foreground)),
                    )
                    // skip forward
                    .child(
                        div()
                            .id("btn-fwd")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(30.0))
                            .h(px(30.0))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.tokens.accent))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, _| {
                                if let Some(v) = &this.video_source {
                                    let new_pos = (v.position().as_secs_f64() + 30.0).min(v.duration().as_secs_f64());
                                    let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                }
                            }))
                            .child(Icon::new("skip-forward").size(px(16.0)).color(theme.tokens.foreground)),
                    )
                    .child(vline())
                    // time
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .mx_1()
                            .rounded_md()
                            .bg(gpui::rgba(0xffffff_08))
                            .text_sm()
                            .font_family("Consolas")
                            .text_color(theme.tokens.muted_foreground)
                            .child(time_display),
                    ),
            )
            // right: markers + clip controls
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.muted_foreground).mr_1().child("Markers"))
                    .children(crate::state::MarkerKind::ALL.iter().map(|&kind| {
                        let (h, s, l, a) = kind.color_hsla();
                        let color = hsla(h, s, l, a);
                        div()
                            .id(SharedString::from(format!("mk-{}", kind.label())))
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(30.0))
                            .h(px(30.0))
                            .rounded_md()
                            .cursor_pointer()
                            .bg(color.opacity(0.12))
                            .border_1()
                            .border_color(color.opacity(0.2))
                            .hover(|s| s.bg(color.opacity(0.25)))
                            .active(|s| s.bg(color.opacity(0.35)))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                                this.add_marker_with_kind(kind, cx);
                            }))
                            .child(Icon::new(kind.icon_name()).size(px(14.0)).color(color))
                            .into_any_element()
                    }))
                    .child(vline())
                    // IN
                    .child(
                        div()
                            .id("btn-in")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .h(px(28.0))
                            .rounded_md()
                            .cursor_pointer()
                            .bg(theme.tokens.background)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .hover(|s| s.bg(theme.tokens.accent))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                this.set_clip_in(cx);
                            }))
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("In"))
                            .child(div().text_xs().font_family("Consolas").font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(clip_in_text)),
                    )
                    // OUT
                    .child(
                        div()
                            .id("btn-out")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .h(px(28.0))
                            .rounded_md()
                            .cursor_pointer()
                            .bg(theme.tokens.background)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .hover(|s| s.bg(theme.tokens.accent))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                this.set_clip_out(cx);
                            }))
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("Out"))
                            .child(div().text_xs().font_family("Consolas").font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(clip_out_text)),
                    )
                    // save
                    .child(
                        div()
                            .id("btn-save")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .h(px(30.0))
                            .rounded_lg()
                            .cursor_pointer()
                            .when(has_clip_range, |el| el.bg(theme.tokens.primary).hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0))))
                            .when(!has_clip_range, |el| el.bg(theme.tokens.accent).hover(|s| s.bg(theme.tokens.border)))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, window, cx| {
                                this.save_clip(window, cx);
                            }))
                            .child(Icon::new("scissors").size(px(13.0)).color(if has_clip_range { theme.tokens.primary_foreground } else { theme.tokens.muted_foreground }))
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(if has_clip_range { theme.tokens.primary_foreground } else { theme.tokens.muted_foreground }).child("Save clip")),
                    ),
            ),
            )
            // timeline below the controls
            .child(timeline)
    }
}
