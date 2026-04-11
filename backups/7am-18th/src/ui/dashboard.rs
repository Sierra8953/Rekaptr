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
            None => div().w_full().h_full().bg(rgb(0x000000)).flex().items_center().justify_center().child(div().text_color(theme.tokens.muted_foreground).child("No video source loaded")).into_any_element(),
        };

        let is_recording = self.app_state.is_recording.load(std::sync::atomic::Ordering::SeqCst);

        div()
            .id("dashboard-scroll-area")
            .size_full()
            .overflow_y_scroll()
            .child(
                VStack::new()
                    .p_8()
                    .gap_6()
                    .w_full()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(
                                HStack::new()
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.tokens.foreground)
                                            .child("Dashboard")
                                    )
                                    .when(is_recording, |this| {
                                        this.child(
                                            HStack::new()
                                                .items_center()
                                                .gap_1_5()
                                                .px_2()
                                                .py_0p5()
                                                .child(div().w_1_5().h_1_5().rounded_full().bg(theme.tokens.destructive))
                                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.destructive).child("Recording"))
                                        )
                                    })
                            )
                            .child(
                                div()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("Source: {}", self.selected_source.as_deref().unwrap_or("monitor")))
                            )
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(600.0)) // Set a large base height that can grow
                            .flex_grow()
                            .child(if self.is_loading_video {
                                div().w_full().h_full().bg(rgb(0x000000)).flex().items_center().justify_center().child(div().text_color(theme.tokens.muted_foreground).child("Scanning recording segments...")).into_any_element()
                            } else {
                                video_element
                            })
                    )
                    .child(
                        Card::new()
                            .p_4()
                            .content(
                                VStack::new()
                                    .gap_4()
                                    .child(self.render_timeline(window, cx))
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
                                                    .child(format!("{:.1}s / {:.1}s", position, duration))
                                            )
                                    )
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
                        .h(px(140.0))
                        .bg(theme.tokens.card)
                        .border_2()
                        .border_color(theme.tokens.border)
                        .border_dashed()
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
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
                .h(px(140.0))
                .border_color(if monitor_selected { theme.tokens.primary } else { theme.tokens.border })
                .border(if monitor_selected { px(2.0) } else { px(1.0) })
                .rounded_lg()
                .overflow_hidden()
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
                                .child(
                                    Button::new("monitor-settings-btn", "")
                                        .icon(IconSource::Named("settings".to_string()))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
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
            
            if cached_path.is_none() && !self.app_state.artwork_cache.contains_key(&title) {
                let artwork_url_or_path = crate::utils::find_steam_artwork(&title);
                if let Some(source) = artwork_url_or_path {
                    if !source.starts_with("http") {
                        // Found local file, cache immediately
                        self.app_state.artwork_cache.insert(title.clone(), Some(source));
                    } else {
                        // URL, download in background
                        let url = source.clone();
                        let app_state = self.app_state.clone();
                        let handle = cx.weak_entity();
                        let title_cache = title.clone();
                        let title_log = title.clone();
                        
                        // Mark as None to prevent redundant spawns
                        app_state.artwork_cache.insert(title_cache.clone(), None);

                        cx.spawn(move |_, cx: &mut AsyncApp| {
                            let app_state = app_state.clone();
                            let handle = handle.clone();
                            let mut cx = cx.clone();
                            let url = url.clone();
                            let title_cache = title_cache.clone();
                            let title_log = title_log.clone();
                            async move {
                                log::debug!("[UI] Starting download for '{}' from {}", title_log, url);
                                let result = if let Ok(resp) = reqwest::get(&url).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        Some(bytes)
                                    } else { None }
                                } else { None };

                                if let Some(bytes) = result {
                                    let app_id = url.split('/').nth(5).unwrap_or("unknown");
                                    let cache_dir = crate::utils::get_storage_root().join("Cache").join("Artwork");
                                    let local_path = cache_dir.join(format!("{}_hero.jpg", app_id));
                                    if std::fs::write(&local_path, bytes).is_ok() {
                                        log::info!("[UI] Saved artwork for '{}' to {:?}", title_log, local_path);
                                        let path_str = local_path.to_string_lossy().replace('\\', "/");
                                        app_state.artwork_cache.insert(title_cache, Some(path_str));
                                        
                                        let _ = handle.update(&mut cx, |_, cx| {
                                            cx.notify();
                                        });
                                    }
                                }
                            }
                        }).detach();
                    }
                } else {
                    // Cache that we found nothing to stop re-checking
                    self.app_state.artwork_cache.insert(title.clone(), None);
                }
            }

            let final_image_path = cached_path.map(std::path::PathBuf::from);
            let image_exists = final_image_path.is_some();
            let session_key = *session.key() as usize;

            gallery = gallery.child(
                div()
                    .id(("session-wrap", session_key))
                    .relative()
                    .w(px(240.0))
                    .h(px(140.0))
                    .border_color(if is_selected { theme.tokens.primary } else { theme.tokens.border })
                    .border(if is_selected { px(2.0) } else { px(1.0) })
                    .rounded_lg()
                    .overflow_hidden()
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
                                    .child(
                                        Button::new(("session-settings-btn", session_key), "")
                                            .icon(IconSource::Named("settings".to_string()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
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
