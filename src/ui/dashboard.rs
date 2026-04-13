use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_dashboard(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                                        .child(div().w_1_5().h_1_5().rounded_full().bg(theme.tokens.destructive))
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
                    .child(
                        Card::new()
                            .p_4()
                            .content(
                                VStack::new()
                                    .gap_4()
                                    .child(
                                        HStack::new()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                HStack::new()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("btn-record", "")
                                                            .icon(if is_recording { IconSource::Named("square".to_string()) } else { IconSource::Named("circle-dot".to_string()) })
                                                            .variant(if is_recording { ButtonVariant::Destructive } else { ButtonVariant::Default })
                                                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                                this.toggle_recording(window, cx);
                                                            }))
                                                    )
                                                    .child(
                                                        Button::new("btn-back", "")
                                                            .icon(IconSource::Named("rotate-ccw".to_string()))
                                                            .variant(ButtonVariant::Outline)
                                                            .on_click(cx.listener(|this: &mut Self, _, _, _cx| {
                                                                if let Some(v) = &this.video_source {
                                                                    let new_pos = (v.position().as_secs_f64() - 10.0).max(0.0);
                                                                    let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                                }
                                                            }))
                                                    )
                                                    .child({
                                                        let is_paused = self.video_source.as_ref().map_or(true, |v| v.paused());
                                                        Button::new("btn-play", "")
                                                            .icon(if is_paused { IconSource::Named("play".to_string()) } else { IconSource::Named("pause".to_string()) })
                                                            .variant(ButtonVariant::Outline)
                                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                                this.toggle_play_pause(cx);
                                                            }))
                                                    })
                                                    .child(
                                                        Button::new("btn-fwd", "")
                                                            .icon(IconSource::Named("rotate-cw".to_string()))
                                                            .variant(ButtonVariant::Outline)
                                                            .on_click(cx.listener(|this: &mut Self, _, _, _cx| {
                                                                if let Some(v) = &this.video_source {
                                                                    let new_pos = (v.position().as_secs_f64() + 30.0).min(v.duration().as_secs_f64());
                                                                    let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                                }
                                                            }))
                                                    )
                                                    .child(
                                                        Button::new("btn-refresh", "")
                                                            .icon(IconSource::Named("rotate-cw".to_string()))
                                                            .variant(ButtonVariant::Secondary)
                                                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                                let source = this.selected_source.clone().unwrap_or_else(|| "monitor".to_string());
                                                                this.load_video(&source, window, cx);
                                                            }))
                                                    )
                                                    .child(div().w(px(10.0)))
                                                    .children(crate::state::MarkerKind::ALL.iter().map(|&kind| {
                                                        Button::new(SharedString::from(format!("btn-marker-{}", kind.label())), "")
                                                            .icon(IconSource::Named(kind.icon_name().to_string()))
                                                            .variant(ButtonVariant::Secondary)
                                                            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                                                                this.add_marker_with_kind(kind, cx);
                                                            }))
                                                            .into_any_element()
                                                    }))
                                                    .child(div().w(px(6.0)))
                                                    .child(
                                                        Button::new("btn-in", "IN")
                                                            .variant(ButtonVariant::Secondary)
                                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                                this.set_clip_in(cx);
                                                            }))
                                                    )
                                                    .child(
                                                        Button::new("btn-out", "OUT")
                                                            .variant(ButtonVariant::Secondary)
                                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                                this.set_clip_out(cx);
                                                            }))
                                                    )
                                                    .child(
                                                        Button::new("btn-save", "SAVE")
                                                            .variant(ButtonVariant::Default)
                                                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                                this.save_clip(window, cx);
                                                            }))
                                                    )
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme.tokens.muted_foreground)
                                                    .text_sm()
                                                    .font_family("Consolas")
                                                    .child({
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
                                                        format!("{} / {}", fmt(position), fmt(duration))
                                                    })
                                            )
                                    )
                                    .child(self.render_timeline(window, cx))
                            )
                    )
                    .child(
                        VStack::new()
                            .gap_4()
                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Recent Sessions"))
                            .child(self.render_game_gallery(window, cx))
                    )
            )
    }

    pub fn render_game_gallery(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions = &self.app_state.manual_sessions;
        let theme = use_theme();
        
        let mut gallery = div()
            .id("source-gallery")
            .flex()
            .flex_wrap()
            .gap_4();

        gallery = gallery.child(
            div()
                .id("add-source-wrap")
                .child(
                    div()
                        .w(px(240.0))
                        .h(px(135.0))
                        .bg(theme.tokens.card)
                        .border_2()
                        .border_color(theme.tokens.border)
                        .border_dashed()
                        .rounded_xl()
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
                                        .child(Icon::new("plus").size(px(32.0)).color(theme.tokens.muted_foreground))
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

        let monitor_selected = self.selected_source.as_deref() == Some("monitor");
        gallery = gallery.child(
            div()
                .id("monitor-source-wrap")
                .relative()
                .w(px(240.0))
                .h(px(135.0))
                .border_color(if monitor_selected { theme.tokens.primary } else { theme.tokens.border })
                .border(if monitor_selected { px(2.0) } else { px(1.0) })
                .rounded_xl()
                .overflow_hidden()
                .hover(|s| s.shadow_lg())
                .child(
                    div()
                        .size_full()
                        .bg(theme.tokens.card)
                        .child(
                            VStack::new()
                                .h_full()
                                .justify_between()
                                .p_4()
                                .child(
                                    HStack::new()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("Monitor"))
                                )
                                .child(
                                    div().text_sm().text_color(theme.tokens.muted_foreground).child("Record entire desktop")
                                )
                        )
                )
                .child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .child(
                            div()
                                .id("monitor-settings-btn-hitbox")
                                .cursor_pointer()
                                .p_1()
                                .text_color(theme.tokens.muted_foreground)
                                .hover(|s| s.text_color(theme.tokens.primary))
                                .child(
                                    svg().path("icons/settings.svg").size(px(16.0)).flex_shrink_0()
                                )
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
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, window, cx| {
                    log::debug!("[UI] Monitor card clicked");
                    this.selected_source = Some("monitor".to_string());
                    this.load_video("monitor", window, cx);
                    cx.notify();
                }))
        );

        for session in sessions.iter() {
            let title = session.value().title.to_string();
            let is_selected = self.selected_source.as_deref() == Some(title.as_str());
            
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
                            app_state.artwork_cache.insert(title, Some(source));
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
                            let local_path = cache_dir.join(format!("{}_hero.jpg", app_id));
                            if std::fs::write(&local_path, &bytes).is_ok() {
                                let path_str = local_path.to_string_lossy().replace('\\', "/");
                                app_state.artwork_cache.insert(title, Some(path_str));
                                let _ = handle.update(&mut cx, |_, cx| cx.notify());
                            }
                        }
                    }
                }).detach();
            }

            let final_image_path = cached_path.map(std::path::PathBuf::from);
            let image_exists = final_image_path.is_some();
            let session_key = *session.key() as usize;

            gallery = gallery.child(
                div()
                    .id(("session-wrap", session_key))
                    .relative()
                    .w(px(240.0))
                    .h(px(135.0))
                    .border_color(if is_selected { theme.tokens.primary } else { theme.tokens.border })
                    .border(if is_selected { px(2.0) } else { px(1.0) })
                    .rounded_xl()
                    .overflow_hidden()
                    .cursor_pointer()
                    .hover(|s| s.shadow_lg())
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
                            .size_full()
                            .bg(theme.tokens.card)
                            // Artwork Background
                            .child({
                                div()
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .when_some(final_image_path, |this, path| {
                                        this.child(
                                            img(path)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                        )
                                    })
                            })
                            // Dark Overlay for readability - ONLY if image exists
                            .when(image_exists, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(gpui::rgba(0x000000_99))
                                )
                            })
                            .child(
                                VStack::new()
                                    .relative()
                                    .h_full()
                                    .justify_between()
                                    .p_4()
                                    .child(
                                        HStack::new()
                                            .justify_between()
                                            .items_center()
                                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(gpui::white()).child(title.clone()))
                                    )
                                    .child(
                                        div().text_sm().text_color(gpui::rgba(0xffffff_aa)).child("Click to view buffer")
                                    )
                            )
                    )
                    .child(
                        div()
                            .absolute()
                            .top_2()
                            .right_2()
                            .child({
                                let title_settings = title.clone();
                                div()
                                    .id(("session-settings-btn-hitbox", session_key))
                                    .cursor_pointer()
                                    .p_1()
                                    .text_color(theme.tokens.muted_foreground)
                                    .hover(|s| s.text_color(theme.tokens.primary))
                                    .child(
                                        svg().path("icons/settings.svg").size(px(16.0)).flex_shrink_0()
                                    )
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
            );
        }

        gallery
    }
}
