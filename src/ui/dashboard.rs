use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    pub fn render_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64())
        } else {
            (0.0, 1.0)
        };
        let video_element = match &self.video_source {
            Some(v) => {
                let v_clone = v.clone();
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .bg(rgb(0x000000))
                    .child(video(v_clone).id("main-video"))
                    .into_any_element()
            }
            None => div().w_full().h_full().bg(rgb(0x000000)).flex().items_center().justify_center().child(
                VStack::new()
                    .items_center()
                    .gap_3()
                    .child(Icon::new("info").size(px(40.0)).color(theme.tokens.muted_foreground))
                    .child(div().text_color(theme.tokens.muted_foreground).font_weight(FontWeight::MEDIUM).child("Select a source to begin previewing"))
            ).into_any_element(),
        };

        let is_recording = self.app_state.recording.phase.lock().is_recording();

        // Collect recording stats for overlay
        let rec_elapsed = self.recording_start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        let rec_bitrate = *self.app_state.recording.rec_stats.bitrate_kbps.lock();
        let rec_dropped = self.app_state.recording.rec_stats.dropped_frames.load(std::sync::atomic::Ordering::Relaxed);
        let rec_disk_rate = *self.app_state.recording.rec_stats.disk_write_mbps.lock();
        let rec_segments = self.app_state.recording.rec_stats.segments_written.load(std::sync::atomic::Ordering::Relaxed);

        const CONTROLS_BASE_PX: f32 = 140.0;  // padding + gap + button bar + video track
        const AUDIO_TRACK_PX: f32 = 36.0;
        const CHROME_PX: f32 = 160.0;         // header + outer padding + gaps

        let audio_tracks = self.get_current_audio_tracks();
        let enabled_track_count = audio_tracks.iter().filter(|t| t.enabled).count();
        let controls_h = px(CONTROLS_BASE_PX + (enabled_track_count as f32 * AUDIO_TRACK_PX));
        let chrome_h = px(CHROME_PX);
        let video_h = (window.viewport_size().height - chrome_h - controls_h).max(px(150.0));

        div()
            .id("dashboard-scroll-area")
            .size_full()
            .overflow_y_scroll()
            .child(
                VStack::new()
                    .p_8()
                    .gap_6()
                    .w_full()
                    .child({
                        // Fetch logo for game sources (used as overlay on video)
                        let source = self.selected_source.as_deref().unwrap_or("monitor");
                        let logo_path = if source != "monitor" {
                            let lp = self.app_state.logo_cache.get(source).and_then(|v| v.value().clone()).map(std::path::PathBuf::from);
                            if !self.app_state.logo_cache.contains_key(source) {
                                self.app_state.logo_cache.insert(source.to_string(), None);
                                let app_state = self.app_state.clone();
                                let handle = cx.weak_entity();
                                let title = source.to_string();
                                cx.spawn(move |_, cx: &mut AsyncApp| {
                                    let mut cx = cx.clone();
                                    async move {
                                        let resolved = cx.background_executor().spawn({
                                            let title = title.clone();
                                            async move { crate::utils::find_steam_logo(&title) }
                                        }).await;
                                        let Some(source_url) = resolved else { return; };
                                        if !source_url.starts_with("http") {
                                            app_state.logo_cache.insert(title, Some(source_url));
                                            let _ = handle.update(&mut cx, |_, cx| cx.notify());
                                            return;
                                        }
                                        if let Ok(resp) = reqwest::get(&source_url).await {
                                            if let Ok(bytes) = resp.bytes().await {
                                                let app_id = source_url.split('/').nth(5).unwrap_or("unknown");
                                                let cache_dir = crate::utils::get_storage_root().join("Cache").join("Artwork");
                                                let _ = std::fs::create_dir_all(&cache_dir);
                                                let local_path = cache_dir.join(format!("{}_logo.png", app_id));
                                                if std::fs::write(&local_path, &bytes).is_ok() {
                                                    let path_str = local_path.to_string_lossy().replace('\\', "/");
                                                    app_state.logo_cache.insert(title, Some(path_str));
                                                    let _ = handle.update(&mut cx, |_, cx| cx.notify());
                                                }
                                            }
                                        }
                                    }
                                }).detach();
                            }
                            lp
                        } else {
                            None
                        };

                        div()
                            .relative()
                            .w_full()
                            .h(video_h)
                            .rounded_lg()
                            .overflow_hidden()
                            .child(if self.is_loading_video {
                                div().w_full().h_full().bg(rgb(0x000000)).flex().items_center().justify_center().child(div().text_color(theme.tokens.muted_foreground).child("Scanning recording segments...")).into_any_element()
                            } else {
                                video_element
                            })
                            .when(is_recording, |el| {
                                let elapsed_h = rec_elapsed / 3600;
                                let elapsed_m = (rec_elapsed % 3600) / 60;
                                let elapsed_s = rec_elapsed % 60;
                                let time_str = if elapsed_h > 0 {
                                    format!("{:02}:{:02}:{:02}", elapsed_h, elapsed_m, elapsed_s)
                                } else {
                                    format!("{:02}:{:02}", elapsed_m, elapsed_s)
                                };

                                let bitrate_str = if rec_bitrate >= 1000.0 {
                                    format!("{:.1} Mbps", rec_bitrate / 1000.0)
                                } else if rec_bitrate > 0.0 {
                                    format!("{:.0} kbps", rec_bitrate)
                                } else {
                                    "-- kbps".to_string()
                                };

                                let disk_str = if rec_disk_rate > 0.0 {
                                    format!("{:.1} MB/s", rec_disk_rate)
                                } else {
                                    "-- MB/s".to_string()
                                };

                                el.child(
                                    div()
                                        .absolute()
                                        .top_3()
                                        .right_3()
                                        .py_2()
                                        .px_3()
                                        .rounded(px(8.0))
                                        .bg(gpui::rgba(0x000000_bb))
                                        .child(
                                            VStack::new()
                                                .gap_1()
                                                .child(
                                                    HStack::new()
                                                        .items_center()
                                                        .gap_1_5()
                                                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(theme.tokens.destructive))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme.tokens.destructive)
                                                                .child("REC")
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(gpui::rgb(0xffffff))
                                                                .child(time_str)
                                                        )
                                                )
                                                .child(
                                                    HStack::new()
                                                        .gap_3()
                                                        .child(
                                                            div().text_xs().text_color(gpui::rgb(0xaaaaaa))
                                                                .child(bitrate_str)
                                                        )
                                                        .child(
                                                            div().text_xs().text_color(gpui::rgb(0xaaaaaa))
                                                                .child(disk_str)
                                                        )
                                                )
                                                .child(
                                                    HStack::new()
                                                        .gap_3()
                                                        .child(
                                                            div().text_xs().text_color(
                                                                if rec_dropped > 0 { gpui::rgb(0xff9500) } else { gpui::rgb(0xaaaaaa) }
                                                            ).child(format!("Dropped: {}", rec_dropped))
                                                        )
                                                        .child(
                                                            div().text_xs().text_color(gpui::rgb(0xaaaaaa))
                                                                .child(format!("Segments: {}", rec_segments))
                                                        )
                                                )
                                        )
                                )
                            })
                            .when_some(logo_path, |el, path| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_3()
                                        .left_3()
                                        .w(px(32.0))
                                        .h(px(32.0))
                                        .rounded_full()
                                        .overflow_hidden()
                                        .child(
                                            img(path)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                        )
                                )
                            })
                    })
                    .child({
                        let is_paused = self.video_source.as_ref().map_or(true, |v| v.paused());
                        let has_clip_range = self.clip_start >= 0.0;

                        let show_hours = duration >= 3600.0;
                        let fmt = |s: f64| {
                            let total = s.max(0.0) as u64;
                            let h = total / 3600;
                            let m = (total % 3600) / 60;
                            let sec = total % 60;
                            if show_hours {
                                format!("{:01}:{:02}:{:02}", h, m, sec)
                            } else {
                                format!("{:01}:{:02}", m, sec)
                            }
                        };
                        let time_display: SharedString = format!("{} / {}", fmt(position), fmt(duration)).into();

                        let clip_in_text: SharedString = if self.clip_start >= 0.0 {
                            fmt(self.clip_start).into()
                        } else {
                            "--:--".into()
                        };
                        let clip_out_text: SharedString = if self.clip_end >= 0.0 {
                            fmt(self.clip_end).into()
                        } else {
                            "--:--".into()
                        };

                        let divider = || div().w(px(1.0)).h(px(18.0)).bg(theme.tokens.border);

                        div()
                            .w_full()
                            .bg(theme.tokens.card)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .rounded_lg()
                            .p(px(12.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            // Row 1: Transport + Time + Refresh
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            // Record
                                            .child(
                                                div()
                                                    .id("btn-rec")
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(32.0))
                                                    .h(px(32.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .when(!is_recording, |el| {
                                                        el.hover(|s| s.bg(hsla(0.0, 0.7, 0.5, 0.15)))
                                                    })
                                                    .when(is_recording, |el| {
                                                        el.bg(hsla(0.0, 0.7, 0.5, 0.2))
                                                            .border_1()
                                                            .border_color(hsla(0.0, 0.7, 0.5, 0.4))
                                                    })
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, window, cx| {
                                                        this.toggle_recording(window, cx);
                                                    }))
                                                    .child(if is_recording {
                                                        Icon::new("square")
                                                            .size(px(14.0))
                                                            .color(theme.tokens.destructive)
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .w(px(12.0))
                                                            .h(px(12.0))
                                                            .rounded_full()
                                                            .bg(theme.tokens.destructive)
                                                            .into_any_element()
                                                    }),
                                            )
                                            .child(divider())
                                            // Skip back
                                            .child(
                                                div()
                                                    .id("btn-back")
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(32.0))
                                                    .h(px(32.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(theme.tokens.accent))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, _cx| {
                                                        if let Some(v) = &this.video_source {
                                                            let new_pos = (v.position().as_secs_f64() - 10.0).max(0.0);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                        }
                                                    }))
                                                    .child(Icon::new("skip-back").size(px(16.0)).color(theme.tokens.muted_foreground)),
                                            )
                                            // Play/Pause
                                            .child(
                                                div()
                                                    .id("btn-play")
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(36.0))
                                                    .h(px(36.0))
                                                    .rounded(px(8.0))
                                                    .bg(theme.tokens.primary)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0)))
                                                    .active(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.53, 1.0)))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.toggle_play_pause(cx);
                                                    }))
                                                    .child(
                                                        Icon::new(if is_paused { "play" } else { "pause" })
                                                            .size(px(18.0))
                                                            .color(theme.tokens.primary_foreground),
                                                    ),
                                            )
                                            // Skip forward
                                            .child(
                                                div()
                                                    .id("btn-fwd")
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(32.0))
                                                    .h(px(32.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(theme.tokens.accent))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, _cx| {
                                                        if let Some(v) = &this.video_source {
                                                            let new_pos = (v.position().as_secs_f64() + 30.0).min(v.duration().as_secs_f64());
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                        }
                                                    }))
                                                    .child(Icon::new("skip-forward").size(px(16.0)).color(theme.tokens.muted_foreground)),
                                            )
                                            .child(divider())
                                            // Time display
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_family("Consolas")
                                                    .text_color(theme.tokens.muted_foreground)
                                                    .child(time_display),
                                            ),
                                    )
                                    // Right: refresh
                                    .child(
                                        div()
                                            .id("btn-refresh")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(32.0))
                                            .h(px(32.0))
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.tokens.accent))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, window, cx| {
                                                let source = this.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                                                this.load_video(&source, window, cx);
                                            }))
                                            .child(Icon::new("rotate-cw").size(px(14.0)).color(theme.tokens.muted_foreground)),
                                    ),
                            )
                            // Row 2: Markers + Clip controls
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    // Left: markers
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.tokens.muted_foreground)
                                                    .mr(px(4.0))
                                                    .child("Markers"),
                                            )
                                            .children(
                                                crate::state::MarkerKind::ALL.iter().map(|&kind| {
                                                    let (h, s, l, a) = kind.color_hsla();
                                                    let color = hsla(h, s, l, a);
                                                    div()
                                                        .id(SharedString::from(format!("mk-{}", kind.label())))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .w(px(28.0))
                                                        .h(px(28.0))
                                                        .rounded(px(6.0))
                                                        .cursor_pointer()
                                                        .bg(color.opacity(0.1))
                                                        .hover(|s| s.bg(color.opacity(0.25)))
                                                        .active(|s| s.bg(color.opacity(0.35)))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                                                            this.add_marker_with_kind(kind, cx);
                                                        }))
                                                        .child(Icon::new(kind.icon_name()).size(px(14.0)).color(color))
                                                        .into_any_element()
                                                }),
                                            ),
                                    )
                                    // Right: clip controls
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            // IN
                                            .child(
                                                div()
                                                    .id("btn-in")
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .px(px(8.0))
                                                    .h(px(26.0))
                                                    .rounded(px(5.0))
                                                    .cursor_pointer()
                                                    .bg(theme.tokens.accent)
                                                    .hover(|s| s.bg(theme.tokens.border))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.set_clip_in(cx);
                                                    }))
                                                    .child(Icon::new("chevron-right").size(px(12.0)).color(theme.tokens.muted_foreground))
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.tokens.muted_foreground).child("IN"),
                                                    )
                                                    .child(
                                                        div().text_xs().font_family("Consolas")
                                                            .text_color(theme.tokens.muted_foreground).ml(px(2.0))
                                                            .child(clip_in_text),
                                                    ),
                                            )
                                            // OUT
                                            .child(
                                                div()
                                                    .id("btn-out")
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .px(px(8.0))
                                                    .h(px(26.0))
                                                    .rounded(px(5.0))
                                                    .cursor_pointer()
                                                    .bg(theme.tokens.accent)
                                                    .hover(|s| s.bg(theme.tokens.border))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.set_clip_out(cx);
                                                    }))
                                                    .child(Icon::new("chevron-left").size(px(12.0)).color(theme.tokens.muted_foreground))
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.tokens.muted_foreground).child("OUT"),
                                                    )
                                                    .child(
                                                        div().text_xs().font_family("Consolas")
                                                            .text_color(theme.tokens.muted_foreground).ml(px(2.0))
                                                            .child(clip_out_text),
                                                    ),
                                            )
                                            .child(divider())
                                            // Save clip
                                            .child(
                                                div()
                                                    .id("btn-save")
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(5.0))
                                                    .px(px(10.0))
                                                    .h(px(28.0))
                                                    .rounded(px(6.0))
                                                    .cursor_pointer()
                                                    .when(has_clip_range, |el| {
                                                        el.bg(theme.tokens.primary)
                                                            .hover(|s| s.bg(hsla(258.0 / 360.0, 0.9, 0.60, 1.0)))
                                                    })
                                                    .when(!has_clip_range, |el| {
                                                        el.bg(theme.tokens.accent)
                                                            .hover(|s| s.bg(theme.tokens.border))
                                                    })
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, window, cx| {
                                                        this.save_clip(window, cx);
                                                    }))
                                                    .child(
                                                        Icon::new("scissors").size(px(13.0))
                                                            .color(if has_clip_range { theme.tokens.primary_foreground } else { theme.tokens.muted_foreground }),
                                                    )
                                                    .child(
                                                        div().text_xs().font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(if has_clip_range { theme.tokens.primary_foreground } else { theme.tokens.muted_foreground })
                                                            .child("SAVE"),
                                                    ),
                                            ),
                                    ),
                            )
                            // Row 3: Timeline
                            .child(self.render_timeline(window, cx))
                    })
                    .child(
                        VStack::new()
                            .gap_4()
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Sources"))
                            .child(self.render_game_gallery(window, cx))
                    )
            )
    }

    pub fn render_game_gallery(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions = &self.app_state.manual_sessions;
        let theme = use_theme();
        let global_recording = self.app_state.recording.phase.lock().is_recording();

        // Responsive card sizing. Fixed-width cards left a dead gap on the right
        // because only 3 of the 304px cards fit the default window. Instead we
        // size cards to fill the row with at least 4 columns, growing the column
        // count (not the gap) on wider windows. Cards keep the original 304:188
        // aspect ratio.
        const GAP: f32 = 20.0;            // gap_5
        const FOOTER_PX: f32 = 52.0;      // stat_strip height (constant, doesn't scale)
        // Aspect of the art region *above* the footer. The card is sized as
        // art + fixed footer so this aspect stays constant at every card width —
        // which lets us pre-crop the art to it (see `ensure_card_art`) and display
        // with Fill (the only fit this gpui build rounds corners for) without
        // stretching. ART_ASPECT_INV = height / width.
        const ART_ASPECT_INV: f32 = 136.0 / 304.0;
        // Width the gallery actually gets: viewport minus the 72px sidebar, the
        // dashboard's p_8 (64px), and a margin covering the scrollbar / rounding.
        // Underestimating here is safe — it only ever makes cards slightly
        // narrower, never wide enough to drop a column.
        let avail = (window.viewport_size().width.0 - 72.0 - 64.0 - 24.0).max(400.0);
        let columns = (((avail + GAP) / (320.0 + GAP)).floor()).max(4.0);
        let card_w = ((avail - (columns - 1.0) * GAP) / columns).floor();
        let card_h = (card_w * ART_ASPECT_INV).round() + FOOTER_PX;

        let mut gallery = div()
            .id("source-gallery")
            .flex()
            .flex_wrap()
            .gap_5();

        // ── Add Source ──────────────────────────────────────────────────
        gallery = gallery.child(
            div()
                .id("add-source-wrap")
                .child(
                    div()
                        .w(px(card_w))
                        .h(px(card_h))
                        .bg(theme.tokens.card)
                        .border_2()
                        .border_color(theme.tokens.border)
                        .border_dashed()
                        .rounded_2xl()
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(theme.tokens.muted).border_color(theme.tokens.primary))
                        .child(
                            VStack::new()
                                .items_center()
                                .child(
                                    div()
                                        .id("add-plus-icon")
                                        .child(Icon::new("plus").size(px(36.0)).color(theme.tokens.muted_foreground))
                                )
                                .child(div().text_color(theme.tokens.muted_foreground).font_weight(FontWeight::MEDIUM).mt_2().child("Add Source"))
                        )
                )
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_add_source_modal = true;
                    this.refresh_available_windows(cx);
                    cx.notify();
                }))
        );

        // ── Monitor ─────────────────────────────────────────────────────
        let monitor_selected = self.selected_source.as_deref() == Some("monitor");
        let monitor_recording = monitor_selected && global_recording;
        let monitor_tint = avatar_tint("Monitor");
        gallery = gallery.child(
            div()
                .id("monitor-source-wrap")
                .relative()
                .w(px(card_w))
                .h(px(card_h))
                .child(
                    div()
                        .relative()
                        .size_full()
                        .rounded_2xl()
                        .bg(theme.tokens.card)
                        .border_2()
                        .border_color(if monitor_selected { theme.tokens.primary } else { theme.tokens.border })
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        // ── art body ──
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .overflow_hidden()
                                .rounded_t_2xl()
                                .bg(rgba((monitor_tint << 8) | 0x14))
                                .child(
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
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .gap(px(2.0))
                                                                .min_w(px(0.0))
                                                                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("Monitor"))
                                                                .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Record entire desktop"))
                                                        )
                                                )
                                                .child(
                                                    div()
                                                        .id("monitor-settings-btn-hitbox")
                                                        .size(px(28.0))
                                                        .rounded_full()
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(rgba(0x000000_55))
                                                        .border_1()
                                                        .border_color(rgba(0xffffff_1a))
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(rgba(0x000000_aa)).border_color(rgba(0xffffff_55)))
                                                        .child(Icon::new("settings").size(px(14.0)).color(theme.tokens.foreground))
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.advanced_settings_source = Some("monitor".to_string());

                                                            // Refresh window list for audio routing asynchronously
                                                            this.refresh_available_windows(cx);

                                                            // Load current monitor settings into form state
                                                            let config = crate::config::AppConfig::load();
                                                            this.form_encoder = config.global_video.encoder.clone();
                                                            this.form_rate_control = config.global_video.rate_control_index;
                                                            this.form_bitrate = config.global_video.bitrate_kbps;
                                                            this.form_cq = config.global_video.cq_level;
                                                            this.form_retention = config.global_video.retention_minutes;
                                                            this.form_resolution = config.global_video.resolution.clone();
                                                            this.form_fps = config.global_video.fps;
                                                            this.form_gop = config.global_video.gop_size;
                                                            this.form_bframes = config.global_video.bframes;
                                                            this.form_preset = config.global_video.preset.clone();
                                                            this.form_zero_latency = config.global_video.zero_latency;
                                                            this.form_lookahead = config.global_video.lookahead;
                                                            this.form_lookahead_frames = config.global_video.lookahead_frames;
                                                            this.form_spatial_aq = config.global_video.spatial_aq;
                                                            this.form_temporal_aq = config.global_video.temporal_aq;
                                                            this.form_audio_tracks = config.global_audio_tracks.clone();
                                                            this.form_active_tab = 0;
                                                            this.form_editing_track_index = None;

                                                            cx.notify();
                                                        }))
                                                )
                                        )
                                        .child(
                                            div().flex().items_center().child(status_chip(monitor_recording))
                                        )
                                )
                        )
                        // ── stat strip (placeholder values until per-source metrics land) ──
                        .child(stat_strip("—", "—", "—"))
                )
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, window, cx| {
                    log::debug!("[UI] Monitor card clicked");
                    this.selected_source = Some("monitor".to_string());
                    this.load_video("monitor", window, cx);
                    cx.notify();
                }))
        );

        // ── Sessions ────────────────────────────────────────────────────
        for session in sessions.iter() {
            let title = session.value().title.to_string();
            let is_selected = self.selected_source.as_deref() == Some(title.as_str());
            let is_recording_this = is_selected && global_recording;
            let auto_record = session.value().auto_record;
            let session_subtitle = if auto_record { "Auto-record on launch" } else { "Manual capture" };

            // Check cache
            let cached_path = self.app_state.artwork_cache.get(&title).map(|v| v.value().clone()).flatten();

            if !self.app_state.artwork_cache.contains_key(&title) {
                // Mark as in-progress immediately
                self.app_state.artwork_cache.insert(title.clone(), None);

                let app_state = self.app_state.clone();
                let handle = cx.weak_entity();
                let title_cache = title.clone();

                cx.spawn(move |_, cx: &mut AsyncApp| {
                    let app_state = app_state;
                    let handle = handle;
                    let mut cx = cx.clone();
                    let title = title_cache;
                    async move {
                        // Resolve app_id + check local cache off UI thread
                        let resolved = cx.background_executor().spawn({
                            let title = title.clone();
                            async move {
                                crate::utils::find_steam_artwork(&title)
                            }
                        }).await;

                        let Some(source) = resolved else { return; };

                        if !source.starts_with("http") {
                            // Local file already on disk (Steam's pre-blurred hero).
                            let raw = std::path::PathBuf::from(&source);
                            let final_path = ensure_card_art(&raw).unwrap_or(raw);
                            let path_str = final_path.to_string_lossy().replace('\\', "/");
                            app_state.artwork_cache.insert(title, Some(path_str));
                            let _ = handle.update(&mut cx, |_, cx| cx.notify());
                            return;
                        }

                        // Download from CDN
                        let result = if let Ok(resp) = reqwest::get(&source).await {
                            if let Ok(bytes) = resp.bytes().await { Some(bytes) } else { None }
                        } else { None };

                        if let Some(bytes) = result {
                            let app_id = source.split('/').nth(5).unwrap_or("unknown");
                            let cache_dir = crate::utils::get_storage_root().join("Cache").join("Artwork");
                            let _ = std::fs::create_dir_all(&cache_dir);
                            let local_path = cache_dir.join(format!("{}_heroblur.jpg", app_id));
                            if std::fs::write(&local_path, &bytes).is_ok() {
                                let final_path = ensure_card_art(&local_path).unwrap_or(local_path);
                                let path_str = final_path.to_string_lossy().replace('\\', "/");
                                app_state.artwork_cache.insert(title, Some(path_str));
                                let _ = handle.update(&mut cx, |_, cx| cx.notify());
                            }
                        }
                    }
                }).detach();
            }

            let final_image_path = cached_path.map(std::path::PathBuf::from);
            let session_key = *session.key() as usize;
            let tint = avatar_tint(&title);
            let title_for_label = title.clone();

            gallery = gallery.child(
                div()
                    .id(("session-wrap", session_key))
                    .relative()
                    .w(px(card_w))
                    .h(px(card_h))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener({
                        let title = title.clone();
                        move |this: &mut Self, _, window, cx| {
                            this.selected_source = Some(title.clone());
                            this.load_video(&title, window, cx);
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .rounded_2xl()
                            .bg(theme.tokens.card)
                            .border_2()
                            .border_color(if is_selected { theme.tokens.primary } else { theme.tokens.border })
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            // ── art body ──
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .overflow_hidden()
                                    .rounded_t_2xl()
                                    .bg(rgba((tint << 8) | 0x14))
                                    .when_some(final_image_path, |this, path| {
                                        // The art is pre-cropped to the card's aspect
                                        // ratio (see `ensure_card_art`), so Fill renders
                                        // it without distortion. We use Fill rather than
                                        // Cover because this gpui build squares the
                                        // corners of an image that overflows its box, and
                                        // Cover always overflows.
                                        this.child(
                                            img(path)
                                                .absolute()
                                                .inset_0()
                                                .size_full()
                                                .rounded_t_2xl()
                                                .object_fit(ObjectFit::Fill),
                                        )
                                    })
                                    .child(
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
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .gap(px(2.0))
                                                                    .min_w(px(0.0))
                                                                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(gpui::white()).child(title_for_label))
                                                                    .child(div().text_xs().text_color(gpui::rgba(0xffffff_b0)).child(session_subtitle))
                                                            )
                                                    )
                                                    .child({
                                                        let title_settings = title.clone();
                                                        div()
                                                            .id(("session-settings-btn-hitbox", session_key))
                                                            .size(px(28.0))
                                                            .rounded_full()
                                                            .flex_shrink_0()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .bg(rgba(0x000000_55))
                                                            .border_1()
                                                            .border_color(rgba(0xffffff_1a))
                                                            .cursor_pointer()
                                                            .hover(|s| s.bg(rgba(0x000000_aa)).border_color(rgba(0xffffff_55)))
                                                            .child(Icon::new("settings").size(px(14.0)).color(gpui::white()))
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                cx.stop_propagation();
                                                                this.advanced_settings_source = Some(title_settings.clone());

                                                                // Refresh window list for audio routing asynchronously
                                                                this.refresh_available_windows(cx);

                                                                // Load session settings into form state
                                                                let config = crate::config::AppConfig::load();
                                                                if let Some(settings) = config.game_registry.get(&title_settings) {
                                                                    if let Some(video) = &settings.video_overrides {
                                                                        this.form_encoder = video.encoder.clone();
                                                                        this.form_rate_control = video.rate_control_index;
                                                                        this.form_bitrate = video.bitrate_kbps;
                                                                        this.form_cq = video.cq_level;
                                                                        this.form_resolution = video.resolution.clone();
                                                                        this.form_fps = video.fps;
                                                                        this.form_retention = video.retention_minutes;
                                                                        this.form_gop = video.gop_size;
                                                                        this.form_bframes = video.bframes;
                                                                        this.form_preset = video.preset.clone();
                                                                        this.form_zero_latency = video.zero_latency;
                                                                        this.form_lookahead = video.lookahead;
                                                                        this.form_lookahead_frames = video.lookahead_frames;
                                                                        this.form_spatial_aq = video.spatial_aq;
                                                                        this.form_temporal_aq = video.temporal_aq;
                                                                    }
                                                                    if let Some(audio) = &settings.audio_routing {
                                                                        this.form_audio_tracks = audio.clone();
                                                                    } else {
                                                                        this.form_audio_tracks = config.global_audio_tracks.clone();
                                                                    }
                                                                    this.form_auto_record = settings.auto_record;
                                                                }
                                                                this.form_active_tab = 0;
                                                                this.form_editing_track_index = None;

                                                                cx.notify();
                                                            }))
                                                    })
                                            )
                                            .child(
                                                div().flex().items_center().child(status_chip(is_recording_this))
                                            )
                                    )
                            )
                            // ── stat strip (placeholder values until per-source metrics land) ──
                            .child(stat_strip("—", "—", "—"))
                    )
            );
        }

        gallery
    }
}

// ── source-card visual helpers ──────────────────────────────────────────

fn avatar_tint(title: &str) -> u32 {
    const PALETTE: &[u32] = &[
        0x6366f1, 0xec4899, 0xf59e0b, 0x10b981, 0x06b6d4, 0x8b5cf6,
    ];
    let idx = title.as_bytes().first().copied().unwrap_or(0) as usize % PALETTE.len();
    PALETTE[idx]
}

fn status_chip(recording: bool) -> Div {
    let (label, dot, fg, bg, br) = if recording {
        ("Recording", 0xef4444u32, 0xfecaca, 0xef4444_33u32, 0xef4444_66u32)
    } else {
        ("Idle", 0x10b981u32, 0xa7f3d0, 0x10b981_22u32, 0x10b981_55u32)
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

fn stat_strip(captured: &str, clips: &str, last: &str) -> Div {
    let theme = use_theme();
    div()
        .h(px(52.0))
        .rounded_b_2xl()
        .border_t_1()
        .border_color(theme.tokens.border)
        .bg(theme.tokens.muted)
        .overflow_hidden()
        .flex()
        .child(stat_cell("captured", captured))
        .child(stat_divider())
        .child(stat_cell("clips", clips))
        .child(stat_divider())
        .child(stat_cell("last", last))
}

fn stat_cell(label: &str, value: &str) -> Div {
    let theme = use_theme();
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
                .text_color(theme.tokens.foreground)
                .child(value.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.tokens.muted_foreground)
                .child(label.to_string()),
        )
}

fn stat_divider() -> Div {
    let theme = use_theme();
    div().my_3().w(px(1.0)).bg(theme.tokens.border)
}

/// Center-crop an artwork file to the source-card aspect ratio (304:188) and
/// cache the result next to the original. The card displays art with
/// `ObjectFit::Fill` so its rounded corners render (this gpui build squares the
/// corners of an image that overflows its box, which `Cover` does); pre-cropping
/// to the card's aspect means `Fill` no longer distorts the image. Returns the
/// cropped path, or `None` on any decode/encode failure (caller falls back to
/// the original).
fn ensure_card_art(raw: &std::path::Path) -> Option<std::path::PathBuf> {
    // Must match `render_game_gallery`'s art region: width / height = 304 / 136.
    const TARGET: f64 = 304.0 / 136.0;
    let stem = raw.file_stem()?.to_string_lossy().into_owned();
    let ext = raw.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    // `_card2` so any mis-cropped art cached by an earlier build is regenerated.
    let cropped = raw.with_file_name(format!("{stem}_card2.{ext}"));
    if cropped.exists() {
        return Some(cropped);
    }
    let img = image::open(raw).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    let (cw, ch) = if (w as f64 / h as f64) > TARGET {
        ((h as f64 * TARGET).round() as u32, h) // too wide → trim sides
    } else {
        (w, (w as f64 / TARGET).round() as u32) // too tall → trim top/bottom
    };
    let x = (w - cw) / 2;
    let y = (h - ch) / 2;
    img.crop_imm(x, y, cw, ch).save(&cropped).ok()?;
    Some(cropped)
}

