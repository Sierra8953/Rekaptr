use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use adabraka_ui::components::input::Input;
use crate::config::AudioRouting;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    pub fn render_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        // Audio tracks drive the side mixer; make sure a volume slider exists per
        // enabled track (these are the same sliders that mix playback audio).
        let enabled_tracks: Vec<AudioRouting> = self
            .get_current_audio_tracks()
            .into_iter()
            .filter(|t| t.enabled)
            .collect();
        self.ensure_track_vol_sliders(enabled_tracks.len(), cx);

        let is_recording = self.app_state.recording.phase.lock().is_recording();
        let rec_elapsed = self.recording_start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);

        div()
            .id("dashboard-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.tokens.background)
            // No top bar — the preview + mixer sit flush at the very top. The row
            // flexes to whatever space is left after the transport/timeline box
            // and the sources list.
            .child(
                div()
                    .id("dashboard-content")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .px_8()
                    .pt_3()
                    .pb_8()
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .min_h(px(240.0))
                            .flex()
                            .gap_5()
                            .child(self.render_preview_pane(is_recording, rec_elapsed, cx))
                            .child(self.render_mixer(&enabled_tracks, cx)),
                    )
                    .child(self.render_transport_strip(cx))
                    // Everything under the timeline box is nudged down 5px.
                    .child(div().mt(px(5.0)).child(self.render_sources_list(window, cx))),
            )
    }

    // ── preview ──────────────────────────────────────────────────────────────
    fn render_preview_pane(&mut self, is_recording: bool, rec_elapsed: u64, _cx: &mut Context<Self>) -> AnyElement {
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

    // ── audio mixer (right of the preview) ─────────────────────────────────────
    fn render_mixer(&mut self, enabled_tracks: &[AudioRouting], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let count = enabled_tracks.len();
        let source = self.selected_source.clone();

        // dB from a 0..150 volume value (100 == unity / 0 dB).
        let db_text = |v: f64| -> String {
            if v <= 0.5 { return "-∞".to_string(); }
            format!("{:.1}", 20.0 * (v / 100.0).log10())
        };

        let master = self.master_vol_slider.clone();
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
                let slider = self.track_vol_sliders.get(i).cloned();
                let muted = self.mixer_muted.get(i).copied().unwrap_or(false);
                let soloed = self.mixer_solo.get(i).copied().unwrap_or(false);
                let vol = self.playback_volumes.get(i).copied().unwrap_or(100.0);
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

    // ── transport strip ────────────────────────────────────────────────────────
    fn render_transport_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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


    // ── sources list (dense table) ─────────────────────────────────────────────
    fn render_sources_list(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let global_recording = self.app_state.recording.phase.lock().is_recording();
        let total = self.app_state.manual_sessions.len() + 1; // + monitor

        // Build row data (monitor, then sessions), filtered by the search query
        // and resolving icon + stats.
        let query = self.sources_search_input.read(cx).content().trim().to_lowercase();
        let matches = |title: &str| query.is_empty() || title.to_lowercase().contains(&query);

        let mut data: Vec<SrcRow> = Vec::with_capacity(total);
        let monitor_selected = self.selected_source.as_deref() == Some("monitor");
        if matches("Monitor") {
            data.push(self.build_src_row("monitor", "Monitor", "Display", "Record entire desktop", false, monitor_selected, monitor_selected && global_recording, cx));
        }

        let sessions: Vec<(String, bool)> = self
            .app_state
            .manual_sessions
            .iter()
            .map(|s| (s.value().title.clone(), s.value().auto_record))
            .collect();
        for (title, auto) in sessions {
            if !matches(&title) {
                continue;
            }
            let selected = self.selected_source.as_deref() == Some(title.as_str());
            let subtitle = if auto { "Auto-record on launch" } else { "Manual capture" };
            data.push(self.build_src_row(&title, &title, "Game", subtitle, auto, selected, selected && global_recording, cx));
        }

        // Sort by most recently recorded first; sources that have never been
        // recorded (no segment mtime) fall to the bottom.
        data.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

        // Rows live in a scroll area sized to fit up to 3 rows (each a fixed
        // 64px). A 4th row makes the area scroll (with a visible scrollbar)
        // rather than growing the table; fewer rows shrink it to fit. The scroll
        // handle + scrollbar state are persisted on the workspace so the offset
        // survives re-renders.
        const ROW_H: f32 = 64.0;
        let rows_h = (data.len().clamp(1, 3) as f32) * ROW_H;
        let mut content = div().w_full().flex().flex_col();
        let last_idx = data.len().saturating_sub(1);
        for (i, row) in data.iter().enumerate() {
            content = content.child(self.source_row(row, i == last_idx, cx));
        }
        // ── custom scrollbar geometry (dim-purple thumb, slightly inset track,
        // visible only while the box is hovered or the thumb is being dragged) ──
        const TRACK_INSET: f32 = 16.0; // shortens the track at top & bottom
        const THUMB_W: f32 = 6.0;
        const MIN_THUMB: f32 = 28.0;
        const MAX_THUMB: f32 = 40.0; // caps the thumb length so the bar stays short
        let content_h = data.len() as f32 * ROW_H;
        let needs_scroll = content_h > rows_h + 0.5;
        let max_off = (content_h - rows_h).max(0.0);
        let track_len = (rows_h - TRACK_INSET * 2.0).max(0.0);
        let thumb_len = (rows_h / content_h * track_len)
            .clamp(MIN_THUMB.min(track_len), MAX_THUMB.min(track_len));
        let scroll = (-self.sources_scroll_handle.offset().y.0).clamp(0.0, max_off);
        let frac = if max_off > 0.0 { scroll / max_off } else { 0.0 };
        let thumb_top = TRACK_INSET + frac * (track_len - thumb_len);
        let show_thumb = self.sources_box_hovered || self.sources_scrollbar_dragging;
        // dim purple; brighter while dragging, hidden when the box isn't hovered
        let thumb_alpha = if self.sources_scrollbar_dragging { 0.85 } else { 0.55 };
        let thumb_color = hsla(258.0 / 360.0, 0.5, 0.62, thumb_alpha);

        let view = cx.entity().downgrade();
        let scroll_area = div()
            .w_full()
            .h(px(rows_h))
            .relative()
            .child(
                div()
                    .id("sources-scroll")
                    .track_scroll(&self.sources_scroll_handle)
                    .overflow_y_scroll()
                    .relative()
                    .size_full()
                    .child(content),
            )
            .when(needs_scroll, |area| {
                area
                    // capture the scroll area's window-space rect for drag mapping
                    .child(
                        canvas(
                            move |_, _, _| {},
                            move |bounds, _, _, cx| {
                                let _ = view.update(cx, |this, _| this.sources_track_bounds = bounds);
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .when(show_thumb, |area| {
                        area.child(
                            div()
                                .id("sources-scrollthumb")
                                .absolute()
                                .top(px(thumb_top))
                                .right(px(3.0))
                                .w(px(THUMB_W))
                                .h(px(thumb_len))
                                .rounded_full()
                                .bg(thumb_color)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                    this.sources_scrollbar_dragging = true;
                                    cx.stop_propagation();
                                    cx.notify();
                                })),
                        )
                    })
            });

        // Search box + Add-source button form a bar that lives at the top of the
        // table card, sitting directly on top of the rows. (No "Sources" title or
        // sort dropdown — removed to free vertical room for the preview pane.)
        let controls_bar = div()
            .w_full()
            .px_4()
            .py_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(theme.tokens.border)
            // search box (takes the remaining width)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(Input::new(&self.sources_search_input).placeholder("Search sources")),
            )
            // add source (functional)
            .child(
                div()
                    .id("add-source-btn")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(32.0))
                    .flex_shrink_0()
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(theme.tokens.background)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .hover(|s| s.border_color(theme.tokens.primary).bg(theme.tokens.accent))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                        this.show_add_source_modal = true;
                        this.refresh_available_windows(cx);
                        cx.notify();
                    }))
                    .child(Icon::new("plus").size(px(15.0)).color(theme.tokens.muted_foreground))
                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child("Add source")),
            );

        let table = div()
            .w_full()
            .bg(theme.tokens.card)
            .rounded_xl()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .child(controls_bar)
            .child(list_header())
            .child(scroll_area);

        // Drag geometry snapshot for mapping cursor-Y → scroll offset.
        let drag_handle = self.sources_scroll_handle.clone();
        let (drag_track_len, drag_thumb_len, drag_max_off) = (track_len, thumb_len, max_off);

        div()
            .id("sources-box")
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            // Pop the thumb up whenever the cursor is anywhere over the box.
            .on_hover(cx.listener(|this: &mut Self, hovered: &bool, _, cx| {
                if this.sources_box_hovered != *hovered {
                    this.sources_box_hovered = *hovered;
                    cx.notify();
                }
            }))
            // Continue/finish a thumb drag from anywhere within the box.
            .on_mouse_move(cx.listener(move |this: &mut Self, ev: &MouseMoveEvent, _, cx| {
                if !this.sources_scrollbar_dragging || drag_max_off <= 0.0 {
                    return;
                }
                let track_top = this.sources_track_bounds.origin.y.0 + TRACK_INSET;
                let usable = (drag_track_len - drag_thumb_len).max(1.0);
                let frac = ((ev.position.y.0 - track_top - drag_thumb_len / 2.0) / usable).clamp(0.0, 1.0);
                drag_handle.set_offset(gpui::point(px(0.0), px(-(frac * drag_max_off))));
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                if this.sources_scrollbar_dragging {
                    this.sources_scrollbar_dragging = false;
                    cx.notify();
                }
            }))
            .child(table)
    }

    /// Resolve a source's icon + on-disk stats + clip count and pack them into a
    /// renderable row.
    #[allow(clippy::too_many_arguments)]
    fn build_src_row(&self, key: &str, title: &str, kind: &'static str, subtitle: &'static str, auto: bool, selected: bool, recording: bool, cx: &mut Context<Self>) -> SrcRow {
        let icon = self.ensure_source_icon(key, title, cx);
        let stats = self.ensure_source_stats(key, cx);
        let clips = stats.clip_count;
        SrcRow {
            key: key.to_string(),
            title: title.to_string(),
            kind,
            subtitle,
            auto,
            selected,
            recording,
            icon,
            captured: fmt_dur(stats.total_secs),
            on_disk: fmt_size(stats.disk_bytes),
            clips: clips.to_string(),
            last: fmt_ago(stats.last_modified),
            last_modified: stats.last_modified,
        }
    }

    /// One row of the sources table — a 1:1 port of the mockup's `list_row`.
    fn source_row(&self, r: &SrcRow, last: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let dir = crate::utils::get_storage_root().join(crate::utils::clean_title(&r.title));
        let key_for_load = r.key.clone();
        let key_for_settings = r.key.clone();
        let key_for_play = r.key.clone();

        div()
            .id(SharedString::from(format!("src-row-{}", r.key)))
            .relative()
            .w_full()
            .h(px(64.0))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .cursor_pointer()
            .when(r.selected, |el| el.bg(theme.tokens.accent))
            .when(!last, |el| el.border_b_1().border_color(theme.tokens.border))
            .hover(|s| s.bg(theme.tokens.accent))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, window, cx| {
                this.selected_source = Some(key_for_load.clone());
                this.load_video(&key_for_load, window, cx);
                cx.notify();
            }))
            // selection accent bar
            .when(r.selected, |el| {
                el.child(div().absolute().top_0().bottom_0().left_0().w(px(3.0)).bg(theme.tokens.primary))
            })
            // SOURCE (flex)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .min_w(px(0.0))
                    .child(source_avatar(r.icon.clone(), &r.title, r.recording))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.title.clone()))
                                    .child(kind_tag(r.kind))
                                    .when(r.auto, |el| el.child(auto_chip())),
                            )
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(r.subtitle)),
                    ),
            )
            // STATUS
            .child(div().w(px(104.0)).flex().items_center().child(status_chip(r.recording)))
            // ACTIVITY
            .child(div().w(px(110.0)).flex().items_center().child(sparkline(&spark_pattern(&r.title), r.recording)))
            // CAPTURED
            .child(div().w(px(74.0)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.captured.clone()))
            // ON DISK
            .child(div().w(px(78.0)).text_sm().text_color(theme.tokens.muted_foreground).child(r.on_disk.clone()))
            // CLIPS
            .child(div().w(px(50.0)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.clips.clone()))
            // LAST
            .child(div().w(px(74.0)).text_sm().text_color(theme.tokens.muted_foreground).child(r.last.clone()))
            // quick actions
            .child(
                div()
                    .w(px(100.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(ghost_action("circle-play", SharedString::from(format!("src-play-{}", r.key)), cx.listener(move |this: &mut Self, _, window, cx| {
                        cx.stop_propagation();
                        this.selected_source = Some(key_for_play.clone());
                        this.load_video(&key_for_play, window, cx);
                        cx.notify();
                    })))
                    .child(ghost_action("folder", SharedString::from(format!("src-folder-{}", r.key)), cx.listener(move |_this: &mut Self, _, _, cx| {
                        cx.stop_propagation();
                        if dir.exists() {
                            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                        }
                    })))
                    .child(ghost_action("settings", SharedString::from(format!("src-settings-{}", r.key)), cx.listener(move |this: &mut Self, _, _, cx| {
                        cx.stop_propagation();
                        this.open_source_settings(&key_for_settings, cx);
                    }))),
            )
    }

    /// Square Steam game-icon path for a source, resolved from the local Steam
    /// librarycache and cached. `None` while resolving / when unavailable (caller
    /// falls back to a letter tile). Mirrors the mockup's clienticon avatar.
    fn ensure_source_icon(&self, key: &str, title: &str, cx: &mut Context<Self>) -> Option<std::path::PathBuf> {
        if key == "monitor" {
            return None; // not a Steam game — letter tile
        }
        let title = title.to_string();
        if let Some(p) = self.app_state.icon_cache.get(&title).and_then(|v| v.value().clone()) {
            return Some(std::path::PathBuf::from(p));
        }
        if self.app_state.icon_cache.contains_key(&title) {
            return None; // resolving (or none)
        }
        self.app_state.icon_cache.insert(title.clone(), None);
        let app_state = self.app_state.clone();
        let handle = cx.weak_entity();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let resolved = cx
                    .background_executor()
                    .spawn({
                        let title = title.clone();
                        async move { crate::utils::find_steam_icon(&title) }
                    })
                    .await;
                if let Some(path) = resolved {
                    app_state.icon_cache.insert(title, Some(path.to_string_lossy().replace('\\', "/")));
                    let _ = handle.update(&mut cx, |_, cx| cx.notify());
                }
            }
        })
        .detach();
        None
    }

    /// On-disk stats for a source, cached in `AppState::source_stats`. Kicks off
    /// a one-time background scan on first sight (returns zeros until it lands).
    fn ensure_source_stats(&self, key: &str, cx: &mut Context<Self>) -> crate::utils::SourceStats {
        if let Some(s) = self.app_state.source_stats.get(key) {
            return *s.value();
        }
        self.app_state.source_stats.insert(key.to_string(), crate::utils::SourceStats::default());
        let app_state = self.app_state.clone();
        let handle = cx.weak_entity();
        let key = key.to_string();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let scan_key = key.clone();
                let stats = cx
                    .background_executor()
                    .spawn(async move { crate::utils::source_stats(&scan_key) })
                    .await;
                app_state.source_stats.insert(key, stats);
                let _ = handle.update(&mut cx, |_, cx| cx.notify());
            }
        })
        .detach();
        crate::utils::SourceStats::default()
    }

    /// Load the advanced-settings form state for a source and open the dialog.
    /// Shared by the mixer header/footer and the source-row settings button.
    pub fn open_source_settings(&mut self, source: &str, cx: &mut Context<Self>) {
        self.advanced_settings_source = Some(source.to_string());
        self.refresh_available_windows(cx);
        self.form_overlay_enabled = None;

        let config = crate::config::AppConfig::load();
        if source == "monitor" {
            let v = &config.global_video;
            self.form_encoder = v.encoder.clone();
            self.form_rate_control = v.rate_control_index;
            self.form_bitrate = v.bitrate_kbps;
            self.form_cq = v.cq_level;
            self.form_retention = v.retention_minutes;
            self.form_resolution = v.resolution.clone();
            self.form_fps = v.fps;
            self.form_gop = v.gop_size;
            self.form_bframes = v.bframes;
            self.form_preset = v.preset.clone();
            self.form_zero_latency = v.zero_latency;
            self.form_lookahead = v.lookahead;
            self.form_lookahead_frames = v.lookahead_frames;
            self.form_spatial_aq = v.spatial_aq;
            self.form_temporal_aq = v.temporal_aq;
            self.form_audio_tracks = config.global_audio_tracks.clone();
        } else if let Some(settings) = config.game_registry.get(source) {
            if let Some(video) = &settings.video_overrides {
                self.form_encoder = video.encoder.clone();
                self.form_rate_control = video.rate_control_index;
                self.form_bitrate = video.bitrate_kbps;
                self.form_cq = video.cq_level;
                self.form_resolution = video.resolution.clone();
                self.form_fps = video.fps;
                self.form_retention = video.retention_minutes;
                self.form_gop = video.gop_size;
                self.form_bframes = video.bframes;
                self.form_preset = video.preset.clone();
                self.form_zero_latency = video.zero_latency;
                self.form_lookahead = video.lookahead;
                self.form_lookahead_frames = video.lookahead_frames;
                self.form_spatial_aq = video.spatial_aq;
                self.form_temporal_aq = video.temporal_aq;
            }
            if let Some(audio) = &settings.audio_routing {
                self.form_audio_tracks = audio.clone();
            } else {
                self.form_audio_tracks = config.global_audio_tracks.clone();
            }
            self.form_auto_record = settings.auto_record;
            self.form_overlay_enabled = settings.overlay_enabled;
        }
        self.form_active_tab = 0;
        self.form_editing_track_index = None;
        cx.notify();
    }
}

// ── source-card visual helpers ──────────────────────────────────────────

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

/// Packed, render-ready data for one sources-table row.
struct SrcRow {
    key: String,
    title: String,
    kind: &'static str,
    subtitle: &'static str,
    auto: bool,
    selected: bool,
    recording: bool,
    icon: Option<std::path::PathBuf>,
    captured: String,
    on_disk: String,
    clips: String,
    last: String,
    /// Most-recent recording mtime, used to sort rows by recency.
    last_modified: Option<std::time::SystemTime>,
}

fn avatar_tint(title: &str) -> u32 {
    const PALETTE: &[u32] = &[
        0x8b5cf6, 0x22d3ee, 0x4ade80, 0xf472b6, 0x60a5fa, 0xfbbf24,
    ];
    let idx = title.as_bytes().first().copied().unwrap_or(0) as usize % PALETTE.len();
    PALETTE[idx]
}

/// 42px square avatar — the Steam game icon if resolved, else a gradient letter
/// tile. A red ring is drawn while recording. 1:1 with the mockup's `avatar()`.
fn source_avatar(icon: Option<std::path::PathBuf>, title: &str, recording: bool) -> AnyElement {
    let ring = |d: Div| {
        d.child(
            div()
                .absolute()
                .inset(px(-3.0))
                .rounded_lg()
                .border_2()
                .border_color(gpui::rgba(0xef4444_cc)),
        )
    };
    if let Some(path) = icon {
        let mut d = div().relative().size(px(42.0)).flex_shrink_0().child(
            img(path)
                .size_full()
                .rounded_lg()
                .border_1()
                .border_color(gpui::rgba(0xffffff_12))
                .shadow_md()
                .object_fit(ObjectFit::Cover),
        );
        if recording {
            d = ring(d);
        }
        d.into_any_element()
    } else {
        let tint = avatar_tint(title);
        let letter = title.chars().next().unwrap_or('?').to_uppercase().to_string();
        let grad = gpui::linear_gradient(
            155.0,
            gpui::linear_color_stop(gpui::rgba((tint << 8) | 0xff), 0.0),
            gpui::linear_color_stop(gpui::rgba((tint << 8) | 0x80), 1.0),
        );
        let mut d = div()
            .relative()
            .size(px(42.0))
            .rounded_lg()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(grad)
            .border_1()
            .border_color(gpui::rgba(0xffffff_12))
            .shadow_md()
            .text_base()
            .font_weight(FontWeight::BLACK)
            .text_color(gpui::white())
            .child(letter);
        if recording {
            d = ring(d);
        }
        d.into_any_element()
    }
}

/// Deterministic decorative activity sparkline pattern per source title. There
/// is no real per-source activity time-series, so this is stable visual filler
/// (varied but reproducible), not measured data.
fn spark_pattern(title: &str) -> [f32; 12] {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset
    for b in title.as_bytes() {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    let mut out = [0.0f32; 12];
    for (i, v) in out.iter_mut().enumerate() {
        h ^= h >> 33;
        h = h
            .wrapping_mul(0xff51afd7ed558ccd)
            .wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        *v = 0.15 + ((h >> 24) % 1000) as f32 / 1000.0 * 0.85;
    }
    out
}

fn sparkline(vals: &[f32; 12], recording: bool) -> impl IntoElement {
    let theme = use_theme();
    let color = if recording { theme.tokens.primary } else { gpui::rgb(0x52525b).into() };
    let mut row = div().flex().items_end().gap(px(2.0)).h(px(26.0));
    for &v in vals {
        row = row.child(div().w(px(5.0)).h(px(4.0 + v * 22.0)).rounded_sm().bg(color.opacity(0.8)));
    }
    row
}

fn kind_tag(kind: &'static str) -> impl IntoElement {
    let theme = use_theme();
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded_md()
        .bg(theme.tokens.background)
        .border_1()
        .border_color(theme.tokens.border)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.tokens.muted_foreground)
        .child(kind)
}

fn auto_chip() -> impl IntoElement {
    let theme = use_theme();
    div()
        .flex()
        .items_center()
        .gap_1()
        .px(px(6.0))
        .py(px(1.0))
        .rounded_md()
        .bg(theme.tokens.primary.opacity(0.14))
        .border_1()
        .border_color(theme.tokens.primary.opacity(0.33))
        .child(Icon::new("zap").size(px(10.0)).color(theme.tokens.primary))
        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.primary).child("Auto"))
}

fn ghost_action(
    name: &'static str,
    id: SharedString,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = use_theme();
    div()
        .id(id)
        .size(px(28.0))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(gpui::rgba(0xffffff_06))
        .border_1()
        .border_color(theme.tokens.border)
        .hover(|s| s.bg(theme.tokens.muted).border_color(theme.tokens.border))
        .child(Icon::new(name).size(px(14.0)).color(theme.tokens.muted_foreground))
        .on_mouse_down(MouseButton::Left, on_down)
}

fn list_header() -> Div {
    let theme = use_theme();
    let col = |label: &'static str, w: f32| {
        div()
            .w(px(w))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.tokens.muted_foreground)
            .child(label)
    };
    div()
        .w_full()
        .h(px(34.0))
        .px_4()
        .flex()
        .items_center()
        .bg(theme.tokens.background)
        .border_b_1()
        .border_color(theme.tokens.border)
        .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.muted_foreground).child("SOURCE"))
        .child(col("STATUS", 104.0))
        .child(col("ACTIVITY", 110.0))
        .child(col("CAPTURED", 74.0))
        .child(col("ON DISK", 78.0))
        .child(col("CLIPS", 50.0))
        .child(col("LAST", 74.0))
        .child(div().w(px(100.0)))
}

fn status_chip(recording: bool) -> Div {
    let (label, dot, fg, bg, br) = if recording {
        ("Recording", 0xef4444u32, 0xfecaca, 0xef4444_2e_u32, 0xef4444_66u32)
    } else {
        ("Idle", 0x4ade80u32, 0xbbf7d0, 0x4ade80_24u32, 0x4ade80_55u32)
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
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(fg))
                .child(label),
        )
}

fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else if bytes == 0 {
        "—".to_string()
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_dur(secs: f64) -> String {
    if secs <= 0.0 {
        return "—".to_string();
    }
    let h = secs / 3600.0;
    if h >= 1.0 {
        format!("{:.1}h", h)
    } else if secs >= 60.0 {
        format!("{:.0}m", secs / 60.0)
    } else {
        format!("{:.0}s", secs)
    }
}

fn fmt_ago(t: Option<std::time::SystemTime>) -> String {
    let Some(t) = t else { return "—".to_string() };
    let s = t.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    if s < 60 {
        "now".to_string()
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else if s < 172800 {
        "yesterday".to_string()
    } else {
        format!("{}d ago", s / 86400)
    }
}
