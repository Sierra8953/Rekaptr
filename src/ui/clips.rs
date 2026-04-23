use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use crate::state::Clip;
use adabraka_ui::display::data_table::ColumnDef;
use adabraka_ui::components::input::Input;

#[derive(Clone, PartialEq, Eq)]
pub enum ClipsFilter {
    All,
    Favorites,
    Recent,
    Game(String),
}

impl RekaptrWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_selection = !self.selected_clips.is_empty();
        let filtered = self.clips_filtered(cx);
        let (hero_clip, groups) = self.clips_groups(&filtered);

        let mut root = div()
            .size_full()
            .flex()
            .relative()
            .child(
                HStack::new()
                    .flex_1()
                    .h_full()
                    .child(self.render_clips_filter_rail(cx))
                    .child(
                        VStack::new()
                            .flex_1()
                            .h_full()
                            .child(self.render_clips_top_bar(filtered.len(), cx))
                            .child(
                                div()
                                    .id("clips-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(self.render_clips_body(hero_clip, groups, cx)),
                            )
                            .when(has_selection, |this| {
                                this.child(self.render_batch_actions_bar(cx))
                            }),
                    ),
            )
            .when_some(self.selected_clip_for_details.clone(), |this, clip| {
                this.child(self.render_clip_details_sidebar(clip, cx))
            })
            .when(self.is_loading_clips, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui::rgba(0x000000_88))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Spinner::new().size(SpinnerSize::Xl)),
                )
            });

        if let Some(clip) = self.clip_to_preview.clone() {
            root = root.child(self.render_mini_player(clip, window, cx));
        }

        root
    }

    pub fn create_clip_columns() -> Vec<ColumnDef<Clip>> {
        vec![
            ColumnDef::new("title", "Title", |c: &Clip| c.title.clone().into()).width(px(300.0)),
            ColumnDef::new("date", "Date", |c: &Clip| c.date.clone().into()).width(px(150.0)),
            ColumnDef::new("size", "Size", |c: &Clip| c.size.clone().into()).width(px(100.0)),
        ]
    }

    fn clips_filtered(&self, cx: &mut Context<Self>) -> Vec<Clip> {
        let q = self.clips_search_input.read(cx).content.to_lowercase();
        let favs = &self.favorite_clips;
        let mut clips: Vec<Clip> = self
            .cached_clips
            .iter()
            .filter(|c| match &self.clips_filter {
                ClipsFilter::All => true,
                ClipsFilter::Favorites => favs.contains(&c.path.to_string_lossy().to_string()),
                ClipsFilter::Recent => true,
                ClipsFilter::Game(g) => &c.title == g,
            })
            .filter(|c| {
                if q.is_empty() {
                    return true;
                }
                c.title.to_lowercase().contains(&q)
                    || c.path.to_string_lossy().to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if matches!(self.clips_filter, ClipsFilter::Recent) {
            clips.truncate(12);
        }

        clips
    }

    /// Split filtered clips into (hero, groups-by-game). Hero is the newest clip
    /// when there's no specific filter. Groups preserve timestamp DESC order.
    fn clips_groups(&self, clips: &[Clip]) -> (Option<Clip>, Vec<(String, Vec<Clip>)>) {
        if clips.is_empty() {
            return (None, Vec::new());
        }

        let hero = if matches!(self.clips_filter, ClipsFilter::All) {
            Some(clips[0].clone())
        } else {
            None
        };

        let mut groups: Vec<(String, Vec<Clip>)> = Vec::new();
        for c in clips {
            if let Some(entry) = groups.iter_mut().find(|(g, _)| g == &c.title) {
                entry.1.push(c.clone());
            } else {
                groups.push((c.title.clone(), vec![c.clone()]));
            }
        }

        (hero, groups)
    }

    fn render_clips_filter_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let total = self.cached_clips.len();
        let fav_count = self
            .cached_clips
            .iter()
            .filter(|c| self.favorite_clips.contains(&c.path.to_string_lossy().to_string()))
            .count();

        let mut games: Vec<(String, usize)> = Vec::new();
        for c in &self.cached_clips {
            if let Some(entry) = games.iter_mut().find(|(g, _)| g == &c.title) {
                entry.1 += 1;
            } else {
                games.push((c.title.clone(), 1));
            }
        }
        games.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        VStack::new()
            .w(px(240.0))
            .h_full()
            .bg(theme.tokens.card.opacity(0.5))
            .border_r_1()
            .border_color(theme.tokens.border)
            .py_5()
            .px_3()
            .gap_1()
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.muted_foreground)
                    .child("LIBRARY"),
            )
            .child(self.clips_rail_item(cx, "layout-dashboard", "All Clips", total, ClipsFilter::All))
            .child(self.clips_rail_item(cx, "star", "Favorites", fav_count, ClipsFilter::Favorites))
            .child(self.clips_rail_item(cx, "rotate-ccw", "Recent", total.min(12), ClipsFilter::Recent))
            .child(
                div()
                    .px_3()
                    .pt_5()
                    .pb_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.muted_foreground)
                    .child("GAMES"),
            )
            .children(
                games
                    .into_iter()
                    .map(|(g, n)| self.clips_rail_item(cx, "gamepad-2", &g.clone(), n, ClipsFilter::Game(g))),
            )
    }

    fn clips_rail_item(
        &self,
        cx: &mut Context<Self>,
        icon: &str,
        label: &str,
        count: usize,
        filter: ClipsFilter,
    ) -> impl IntoElement {
        let theme = use_theme();
        let active = self.clips_filter == filter;
        let label_owned = label.to_string();
        let icon_owned = icon.to_string();
        let filter_clone = filter.clone();

        div()
            .id(SharedString::from(format!("clips-rail-{}", label)))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .h(px(36.0))
            .px_3()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { theme.tokens.muted } else { gpui::transparent_black() })
            .hover(|s| s.bg(theme.tokens.muted.opacity(0.5)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.clips_filter = filter_clone.clone();
                    cx.notify();
                }),
            )
            .child(
                Icon::new(IconSource::Named(icon_owned))
                    .size(px(16.0))
                    .color(if active { theme.tokens.primary } else { theme.tokens.muted_foreground }),
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                    .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                    .child(label_owned),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child(format!("{}", count)),
            )
    }

    fn render_clips_top_bar(&self, visible_count: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let title = match &self.clips_filter {
            ClipsFilter::All => "All Clips".to_string(),
            ClipsFilter::Favorites => "Favorites".to_string(),
            ClipsFilter::Recent => "Recent".to_string(),
            ClipsFilter::Game(g) => g.clone(),
        };
        let subtitle = format!("{} clips", visible_count);

        HStack::new()
            .px_8()
            .py_5()
            .border_b_1()
            .border_color(theme.tokens.border)
            .justify_between()
            .items_center()
            .child(
                VStack::new()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::BOLD).child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(subtitle),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(self.render_clips_search(cx))
                    .child(
                        Button::new("clips-topbar-sort", "")
                            .icon(IconSource::Named("chevron-down".to_string()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    )
                    .child(
                        Button::new("clips-topbar-more", "")
                            .icon(IconSource::Named("settings".to_string()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
    }

    fn render_clips_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = use_theme();
        if self.clips_search_expanded {
            HStack::new()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .w(px(280.0))
                        .child(Input::new(&self.clips_search_input).placeholder("Search clips...")),
                )
                .child(
                    Button::new("clips-search-collapse", "")
                        .icon(IconSource::Named("x".to_string()))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clips_search_input
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.clips_search_expanded = false;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            div()
                .id("clips-search-icon")
                .size(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.tokens.muted))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.clips_search_expanded = true;
                        cx.notify();
                    }),
                )
                .child(
                    Icon::new(IconSource::Named("search".to_string()))
                        .size(px(16.0))
                        .color(theme.tokens.muted_foreground),
                )
                .into_any_element()
        }
    }

    fn render_clips_body(
        &self,
        hero: Option<Clip>,
        groups: Vec<(String, Vec<Clip>)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        if self.cached_clips.is_empty() && !self.is_loading_clips {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .py_20()
                .child(
                    VStack::new()
                        .items_center()
                        .gap_4()
                        .child(
                            Icon::new(IconSource::Named("video".to_string()))
                                .size(px(64.0))
                                .color(theme.tokens.muted_foreground.opacity(0.5)),
                        )
                        .child(div().text_xl().text_color(theme.tokens.muted_foreground).child("No clips found"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground.opacity(0.7))
                                .child("Start recording to see your clips here"),
                        ),
                )
                .into_any_element();
        }

        let view_handle = cx.entity().downgrade();

        VStack::new()
            .gap_10()
            .pb_10()
            .when_some(hero, |this, clip| {
                this.child(self.render_clips_hero(clip, cx))
            })
            .children(groups.into_iter().map(|(game, clips)| {
                self.render_clips_group_row(game, clips, view_handle.clone(), cx)
                    .into_any_element()
            }))
            .into_any_element()
    }

    fn render_clips_hero(&self, clip: Clip, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clip_path = clip.path.to_string_lossy().to_string();
        let is_fav = self.favorite_clips.contains(&clip_path);
        let clip_for_play = clip.clone();
        let thumb = clip.thumbnail_path.clone();
        let title = clip.title.clone();
        let date = clip.date.clone();
        let duration = clip.duration.clone();

        div().px_8().pt_8().child(
            div()
                .w_full()
                .h(px(320.0))
                .rounded_xl()
                .overflow_hidden()
                .relative()
                .bg(theme.tokens.muted)
                .border_1()
                .border_color(theme.tokens.border)
                .when_some(thumb, |this, path| {
                    let p = path.to_string_lossy().to_string();
                    this.child(
                        div()
                            .absolute()
                            .inset(px(-40.0))
                            .child(
                                img(p.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .opacity(0.55),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset(px(-40.0))
                            .child(
                                img(p.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .opacity(0.35),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset(px(-40.0))
                            .child(
                                img(p)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .opacity(0.25),
                            ),
                    )
                })
                .child(div().absolute().inset_0().bg(gpui::rgba(0x0a0a0a99)))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(gpui::linear_gradient(
                            180.0,
                            gpui::linear_color_stop(gpui::rgba(0x0a0a0a33), 0.0),
                            gpui::linear_color_stop(gpui::rgba(0x0a0a0acc), 1.0),
                        )),
                )
                .child(
                    VStack::new()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .w_full()
                        .p_8()
                        .gap_3()
                        .child(
                            HStack::new()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme.tokens.primary)
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.tokens.foreground)
                                        .child("LATEST"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(format!("{} · {}", date, duration)),
                                ),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .child(title),
                        )
                        .child(
                            HStack::new()
                                .gap_2()
                                .pt_2()
                                .child(
                                    Button::new("clips-hero-play", "Play")
                                        .icon(IconSource::Named("play".to_string()))
                                        .variant(ButtonVariant::Default)
                                        .size(ButtonSize::Lg)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_clip_preview(clip_for_play.clone(), window, cx);
                                        })),
                                )
                                .child({
                                    let fav_path = clip_path.clone();
                                    Button::new("clips-hero-fav", "")
                                        .icon(IconSource::Named("star".to_string()))
                                        .variant(if is_fav {
                                            ButtonVariant::Default
                                        } else {
                                            ButtonVariant::Ghost
                                        })
                                        .size(ButtonSize::Lg)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_favorite(&fav_path, cx);
                                        }))
                                })
                                .child(
                                    Button::new("clips-hero-export", "Export")
                                        .icon(IconSource::Named("scissors".to_string()))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Lg),
                                )
                                .child(
                                    Button::new("clips-hero-more", "")
                                        .icon(IconSource::Named("settings".to_string()))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Lg),
                                ),
                        ),
                ),
        )
    }

    fn render_clips_group_row(
        &self,
        game: String,
        clips: Vec<Clip>,
        view_handle: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const VISIBLE: usize = 4;
        let count = clips.len();
        let game_for_nav = game.clone();
        let game_for_tile = game.clone();
        let overflow = count.saturating_sub(VISIBLE);
        let visible_clips: Vec<Clip> = clips.into_iter().take(VISIBLE).collect();

        let fav_map: Vec<bool> = visible_clips
            .iter()
            .map(|c| self.favorite_clips.contains(&c.path.to_string_lossy().to_string()))
            .collect();
        let sel_map: Vec<bool> = visible_clips
            .iter()
            .map(|c| self.selected_clips.contains(&c.path.to_string_lossy().to_string()))
            .collect();

        let theme = use_theme();

        VStack::new()
            .px_8()
            .gap_3()
            .child(
                HStack::new()
                    .justify_between()
                    .items_end()
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(game.clone()),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(theme.tokens.muted)
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("{} clips", count)),
                            ),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("clips-view-all-{}", game)),
                            "View all",
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.clips_filter = ClipsFilter::Game(game_for_nav.clone());
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div().w_full().child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .children(visible_clips.into_iter().enumerate().map(|(i, c)| {
                            Self::render_clip_card_advanced(c, &view_handle, sel_map[i], fav_map[i])
                                .into_any_element()
                        }))
                        .when(overflow > 0, |s| {
                            s.child(self.render_clips_view_all_tile(game_for_tile, overflow, cx))
                        }),
                ),
            )
    }

    fn render_clips_view_all_tile(
        &self,
        game: String,
        overflow: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let game_owned = game.clone();
        div()
            .id(SharedString::from(format!("clips-view-all-tile-{}", game)))
            .w(px(280.0))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(theme.tokens.card)
            .rounded_xl()
            .border_1()
            .border_color(theme.tokens.border)
            .cursor_pointer()
            .hover(|s| s.border_color(theme.tokens.primary).bg(theme.tokens.muted))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.clips_filter = ClipsFilter::Game(game_owned.clone());
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("chevron-right".to_string()))
                            .size(px(28.0))
                            .color(theme.tokens.primary),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("+{} more", overflow)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child("View all"),
                    ),
            )
    }

    /// Open a clip in the mini player overlay. Shared by the hero and card play buttons.
    pub fn open_clip_preview(&mut self, clip: Clip, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.preview_video_source.as_ref().map(|v| v.render_image());
        self.clip_to_preview = Some(clip.clone());
        self.last_preview_mouse_move = std::time::Instant::now();
        self.show_preview_controls = true;
        let url = clip.path.to_string_lossy().to_string();
        let d3d_device_ptr = self.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
        if let Ok(video) = crate::video_player::Video::new_with_options(
            &url,
            crate::video_player::VideoOptions {
                source_name: Some("preview".to_string()),
                ..Default::default()
            },
            d3d_device_ptr,
        ) {
            self.preview_video_source = Some(video);
            self.init_preview_audio_tracks();
        }
        if let Some(ri) = old {
            window.drop_image(ri).ok();
        }
        cx.notify();
    }
}

impl RekaptrWorkspace {
    /// Trigger portrait artwork fetches for a list of game titles.
    /// Everything runs off the UI thread — app_id resolution, local cache check, and download.
    pub fn fetch_portrait_artwork(&self, titles: &[String], cx: &mut Context<Self>) {
        let app_state = self.app_state.clone();
        for title in titles {
            // Already fetched or fetch in progress
            if app_state.portrait_cache.contains_key(title) {
                continue;
            }

            // Mark as in-progress immediately so we don't double-fetch
            app_state.portrait_cache.insert(title.clone(), None);

            let handle = cx.weak_entity();
            let title_cache = title.clone();
            let app_state_spawn = app_state.clone();

            cx.spawn(move |_, cx: &mut gpui::AsyncApp| {
                let app_state = app_state_spawn;
                let handle = handle;
                let mut cx = cx.clone();
                let title = title_cache;
                async move {
                    // Run the blocking app_id resolution + local cache check off the UI thread
                    let resolved = cx.background_executor().spawn({
                        let title = title.clone();
                        async move {
                            crate::utils::find_steam_artwork_portrait(&title)
                        }
                    }).await;

                    let Some(source) = resolved else {
                        // No artwork found — leave None in cache
                        return;
                    };

                    if !source.starts_with("http") {
                        // Local file already cached on disk
                        app_state.portrait_cache.insert(title, Some(source));
                        let _ = handle.update(&mut cx, |_, cx| cx.notify());
                        return;
                    }

                    // Download from CDN
                    let result = if let Ok(resp) = reqwest::get(&source).await {
                        if let Ok(bytes) = resp.bytes().await {
                            Some(bytes)
                        } else { None }
                    } else { None };

                    if let Some(bytes) = result {
                        let app_id = source.split('/').nth(5).unwrap_or("unknown");
                        let cache_dir = crate::utils::get_storage_root().join("Cache").join("Artwork");
                        let _ = std::fs::create_dir_all(&cache_dir);
                        let local_path = cache_dir.join(format!("{}_portrait.jpg", app_id));
                        if std::fs::write(&local_path, &bytes).is_ok() {
                            let path_str = local_path.to_string_lossy().replace('\\', "/");
                            app_state.portrait_cache.insert(title, Some(path_str));
                            let _ = handle.update(&mut cx, |_, cx| cx.notify());
                        }
                    }
                }
            }).detach();
        }
    }

    fn render_clip_card_advanced(clip: Clip, view_handle: &WeakEntity<Self>, is_selected: bool, is_favorited: bool) -> impl IntoElement {
        let theme = use_theme();
        let view_handle_click = view_handle.clone();
        let view_handle_actions = view_handle.clone();
        let clip_for_mouse = clip.clone();

        div()
            .group("clip-card")
            .relative()
            .flex()
            .flex_col()
            .w(px(280.0))
            .bg(if is_selected { theme.tokens.primary.opacity(0.1) } else { theme.tokens.card })
            .border(if is_selected { px(2.0) } else { px(1.0) })
            .border_color(if is_selected { theme.tokens.primary } else { theme.tokens.border })
            .rounded_xl()
            .overflow_hidden()
            .cursor_pointer()
            .hover(|s| s.shadow_lg())
            .on_mouse_down(MouseButton::Left, {
                let view_handle_click = view_handle_click.clone();
                let clip_for_mouse = clip_for_mouse.clone();
                move |event, _, cx| {
                    let clip_path = clip_for_mouse.path.to_string_lossy().to_string();
                    let clip = clip_for_mouse.clone();
                    let _ = view_handle_click.update(cx, |this, cx| {
                        if event.modifiers.control {
                            if this.selected_clips.contains(&clip_path) {
                                this.selected_clips.remove(&clip_path);
                            } else {
                                this.selected_clips.insert(clip_path.clone());
                            }
                        } else {
                            this.selected_clips.clear();
                            this.selected_clip_for_details = Some(clip.clone());
                        }
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(158.0))
                    .rounded_t_xl()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
                    .when_some(clip.thumbnail_path.as_ref(), |this, path| {
                        this.child(
                            img(path.to_string_lossy().to_string())
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .rounded_t_xl(),
                        )
                    })
                    .when(is_favorited, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_2()
                                .left_2()
                                .text_color(gpui::rgba(0xfbbf24ff))
                                .child(Icon::new(IconSource::Named("star".to_string())).size(px(16.0)))
                        )
                    })
                    .child(
                        div()
                            .id(("play-overlay", clip.timestamp))
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.0)
                            .group_hover("clip-card", |this| this.opacity(1.0))
                            .group("play-btn")
                            .text_color(gpui::rgba(0xffffffaa))
                            .hover(|this| this.text_color(gpui::white()))
                            .child(
                                Icon::new(IconSource::Named("play".to_string()))
                                    .size(px(32.0))
                            )
                            .on_mouse_down(MouseButton::Left, {
                                        let clip = clip.clone();
                                        let view_handle = view_handle_click.clone();
                                        move |_, window, cx| {
                                            cx.stop_propagation();
                                            let old_ri = view_handle.update(cx, |this, cx| {
                                                let old = this.preview_video_source.as_ref().map(|v| v.render_image());
                                                this.clip_to_preview = Some(clip.clone());
                                                this.last_preview_mouse_move = std::time::Instant::now();
                                                this.show_preview_controls = true;
                                                let url = clip.path.to_string_lossy().to_string();
                                                let d3d_device_ptr = this.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
                                                if let Ok(video) = crate::video_player::Video::new_with_options(
                                                    &url,
                                                    crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                    d3d_device_ptr,
                                                ) {
                                                    this.preview_video_source = Some(video);
                                                    this.init_preview_audio_tracks();
                                                }
                                                cx.notify();
                                                old
                                            });
                                            if let Ok(Some(ri)) = old_ri {
                                                window.drop_image(ri).ok();
                                            }
                                        }
                                    })
                    )
            )
            .child(
                VStack::new()
                    .p_4()
                    .gap_1()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_start()
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_sm().child(clip.title.clone()))
                            .child(
                                div()
                                    .child(
                                        Button::new(("actions-btn", clip.timestamp), "")
                                            .icon(IconSource::Named("plus".to_string()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
                                            .on_click({
                                                let clip = clip.clone();
                                                let view_handle = view_handle_actions.clone();
                                                move |_, window, cx| {
                                                    let mouse_pos = window.mouse_position();
                                                    let clip = clip.clone();
                                                    let _ = view_handle.update(cx, |this, cx| {
                                                        this.clip_popover = Some((mouse_pos, clip.clone()));
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                    )
                            )
                    )
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(clip.date.clone()))
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(clip.size.clone()))
                    )
            )
    }

    fn render_mini_player(&self, clip: Clip, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        let (pos, dur) = if let Some(v) = &self.preview_video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64().max(1.0))
        } else {
            (0.0, 1.0)
        };
        
        let display_pos = if self.is_scrubbing_preview {
            self.preview_scrubbing_progress as f64 * dur
        } else {
            pos
        };
        
        let progress = (display_pos / dur) as f32;
        let controls_visible = self.show_preview_controls;

        let show_hours = dur >= 3600.0;
        let format_time = move |s: f64| {
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

        let player_width = 1120.0;
        let window_width = window.viewport_size().width.0;
        let left_offset = (window_width - player_width) / 2.0;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.clip_to_preview = None;
                if let Some(old) = this.preview_video_source.take() {
                    window.drop_image(old.render_image()).ok();
                }
                this.is_scrubbing_preview = false;
                this.preview_audio_enabled.clear();
                cx.notify();
            }))
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        this.last_preview_mouse_move = std::time::Instant::now();
                        if !this.show_preview_controls {
                            this.show_preview_controls = true;
                        }
                        
                        if this.is_scrubbing_preview {
                            let relative_x = event.position.x.0 - left_offset;
                            let p = (relative_x as f32 / player_width).clamp(0.0, 1.0);
                            this.preview_scrubbing_progress = p;
                            if let Some(v) = &this.preview_video_source {
                                let target = p as f64 * dur;
                                let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                            }
                        }
                        cx.notify();
                    }))
                    .id("mini-player-container")
                    .w(px(player_width))
                    .h(px(630.0))
                    .bg(theme.tokens.card)
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .overflow_hidden()
                    .relative()
                    .child(
                        div()
                            .size_full()
                            .bg(gpui::black())
                            .when_some(self.preview_video_source.as_ref(), |this, v| {
                                this.child(crate::video_player::video(v.clone()))
                            })
                    )
                    // 1. Top HUD Overlay
                    .child(
                        HStack::new()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .p_6()
                            .bg(gpui::rgba(0x000000_88)) 
                            .justify_between()
                            .items_center()
                            .opacity(if controls_visible { 1.0 } else { 0.0 })
                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(gpui::white()).child(clip.title.clone()))
                            .child(
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                        this.clip_to_preview = None;
                                        if let Some(old) = this.preview_video_source.take() {
                                            window.drop_image(old.render_image()).ok();
                                        }
                                        this.preview_audio_enabled.clear();
                                        cx.notify();
                                    }))
                                    .child(Icon::new("x").size(px(28.0)).color(gpui::white()))
                            )
                    )
                    // 2. Bottom HUD Overlay
                    .child(
                        VStack::new()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .bg(gpui::rgba(0x000000_88))
                            .gap_0()
                            .opacity(if controls_visible { 1.0 } else { 0.0 })
                            .child(
                                // Interactive Progress Bar
                                div()
                                    .h(px(12.0)) // Larger hit area
                                    .w_full()
                                    .relative()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.is_scrubbing_preview = true;
                                        let relative_x = event.position.x.0 - left_offset;
                                        let p = (relative_x as f32 / player_width).clamp(0.0, 1.0);
                                        this.preview_scrubbing_progress = p;
                                        if let Some(v) = &this.preview_video_source {
                                            let target = p as f64 * dur;
                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                        }
                                        cx.notify();
                                    }))
                                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.is_scrubbing_preview = false;
                                        cx.notify();
                                    }))
                                    .child(
                                        adabraka_ui::components::progress::ProgressBar::new(progress)
                                            .h(px(6.0))
                                            .absolute()
                                            .bottom_0()
                                    )
                            )
                            .child(
                                HStack::new()
                                    .px_4()
                                    .py_3()
                                    .items_center()
                                    // Left section — fixed width to balance the right
                                    .child(
                                        div()
                                            .w(px(260.0))
                                            .flex()
                                            .items_center()
                                            .child(
                                                div().text_xs().font_weight(FontWeight::MEDIUM).text_color(gpui::hsla(0.0, 0.0, 1.0, 0.7))
                                                    .child(format!("{} / {}", format_time(display_pos), format_time(dur)))
                                            )
                                    )
                                    // Center: play controls
                                    .child(div().flex_1())
                                    .child(
                                        HStack::new()
                                            .gap_4()
                                            .items_center()
                                            .child(
                                                div()
                                                    .id("preview-skip-back")
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.7))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(v) = &this.preview_video_source {
                                                            let target = (v.position().as_secs_f64() - 5.0).max(0.0);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new("rotate-ccw").size(px(22.0)).color(gpui::white()))
                                            )
                                            .child({
                                                let is_paused = self.preview_video_source.as_ref().map_or(true, |v| v.paused());
                                                div()
                                                    .id("preview-play-pause")
                                                    .cursor_pointer()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(36.0))
                                                    .h(px(36.0))
                                                    .rounded_full()
                                                    .bg(theme.tokens.primary)
                                                    .hover(|s| s.bg(gpui::hsla(258.0/360.0, 0.90, 0.56, 1.0)))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(v) = &this.preview_video_source {
                                                            if v.position() >= v.duration().saturating_sub(std::time::Duration::from_millis(500)) {
                                                                let _ = v.seek(std::time::Duration::ZERO, false);
                                                                v.set_paused(false);
                                                            } else {
                                                                v.set_paused(!v.paused());
                                                            }
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new(if is_paused { "play" } else { "pause" }).size(px(20.0)).color(theme.tokens.primary_foreground))
                                            })
                                            .child(
                                                div()
                                                    .id("preview-skip-fwd")
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.7))
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(v) = &this.preview_video_source {
                                                            let duration = v.duration().as_secs_f64();
                                                            let target = (v.position().as_secs_f64() + 5.0).min(duration);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new("rotate-cw").size(px(22.0)).color(gpui::white()))
                                            )
                                    )
                                    .child(div().flex_1())
                                    // Right section — fixed width to balance the left
                                    .child({
                                        HStack::new()
                                            .w(px(200.0))
                                            .justify_end()
                                            .items_center()
                                            .child(self.preview_vol_slider.clone())
                                            // Audio track toggles
                                            .when(self.preview_video_source.as_ref().map_or(false, |v| v.audio_tracks().len() > 1), |this| {
                                                let audio_tracks = self.preview_video_source.as_ref()
                                                    .map(|v| v.audio_tracks())
                                                    .unwrap_or_default();
                                                this
                                                    .child(
                                                        div()
                                                            .w(px(1.0))
                                                            .h(px(16.0))
                                                            .bg(gpui::hsla(0.0, 0.0, 1.0, 0.2))
                                                    )
                                                    .children(audio_tracks.into_iter().enumerate().map(|(idx, (_id, label))| {
                                                        let enabled = self.preview_audio_enabled.get(idx).copied().unwrap_or(true);
                                                        let short_label = if label.len() > 14 {
                                                            format!("{}...", &label[..12])
                                                        } else {
                                                            label
                                                        };
                                                        div()
                                                            .id(("track-toggle", idx))
                                                            .cursor_pointer()
                                                            .flex()
                                                            .items_center()
                                                            .px(px(6.0))
                                                            .h(px(24.0))
                                                            .rounded(px(4.0))
                                                            .text_xs()
                                                            .when(enabled, |this| {
                                                                this.bg(gpui::hsla(0.0, 0.0, 1.0, 0.2))
                                                                    .text_color(gpui::white())
                                                                    .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 1.0, 0.3)))
                                                            })
                                                            .when(!enabled, |this| {
                                                                this.text_color(gpui::hsla(0.0, 0.0, 1.0, 0.35))
                                                                    .hover(|s| s.text_color(gpui::hsla(0.0, 0.0, 1.0, 0.5)))
                                                            })
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                this.init_preview_audio_tracks();
                                                                if let Some(v) = this.preview_audio_enabled.get_mut(idx) {
                                                                    *v = !*v;
                                                                }
                                                                this.update_preview_audio_mix();
                                                                cx.notify();
                                                            }))
                                                            .child(short_label)
                                                    }))
                                            })
                                    })
                            )
                    )
            )
    }

    fn render_batch_actions_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let count = self.selected_clips.len();
        HStack::new()
            .w_full()
            .px_8()
            .py_4()
            .bg(theme.tokens.card)
            .border_t_1()
            .border_color(theme.tokens.border)
            .justify_between()
            .items_center()
            .child(
                HStack::new()
                    .gap_4()
                    .child(div().font_weight(FontWeight::BOLD).child(format!("{} Clips Selected", count)))
                    .child(
                        Button::new("clear-selection", "Clear Selection")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.selected_clips.clear();
                                cx.notify();
                              }))
                    )
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(
                        Button::new("batch-delete", "Delete Selected")
                            .variant(ButtonVariant::Destructive)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                for path_str in this.selected_clips.clone() {
                                    let path = std::path::PathBuf::from(path_str);
                                    let _ = std::fs::remove_file(&path);
                                    let mut thumb = path.clone();
                                    thumb.set_extension("jpg");
                                    let _ = std::fs::remove_file(thumb);
                                }
                                this.selected_clips.clear();
                                this.refresh_clips(cx);
                                cx.notify();
                            }))
                    )
            )
    }

    fn render_clip_details_sidebar(&self, clip: Clip, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        div()
            .w(px(320.0))
            .h_full()
            .bg(theme.tokens.card)
            .border_l_1()
            .border_color(theme.tokens.border)
            .child(
                VStack::new()
                    .p_6()
                    .gap_6()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).child("Clip Details"))
                            .child(
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.selected_clip_for_details = None;
                                        cx.notify();
                                    }))
                                    .child(Icon::new("x").size(px(24.0)).color(theme.tokens.muted_foreground))
                            )
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(160.0))
                            .bg(gpui::black())
                            .rounded_md()
                            .overflow_hidden()
                            .when_some(clip.thumbnail_path.as_ref(), |this, path| {
                                this.child(
                                    img(path.to_string_lossy().to_string())
                                        .size_full()
                                )
                            })
                    )
                    .child(
                        VStack::new()
                            .gap_4()
                            .child(self.render_detail_item("Title", &clip.title))
                            .child(self.render_detail_item("Recorded", &clip.date))
                            .child(self.render_detail_item("File Size", &clip.size))
                            .child(self.render_detail_item("Duration", &clip.duration))
                            .child(self.render_detail_item("Path", &clip.path.to_string_lossy()))
                    )
                    .child(
                        VStack::new()
                            .gap_2()
                            .child(
                                Button::new("play-details", "Preview Clip")
                                    .w_full()
                                    .icon(IconSource::Named("play".to_string()))
                                    .on_click(cx.listener({
                                        let clip = clip.clone();
                                        move |this, _, window, cx| {
                                            if let Some(old) = this.preview_video_source.take() {
                                                window.drop_image(old.render_image()).ok();
                                            }
                                            this.clip_to_preview = Some(clip.clone());
                                            this.last_preview_mouse_move = std::time::Instant::now();
                                            this.show_preview_controls = true;
                                            let url = clip.path.to_string_lossy().to_string();
                                            let d3d_device_ptr = this.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
                                            if let Ok(video) = crate::video_player::Video::new_with_options(
                                                &url,
                                                crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                d3d_device_ptr,
                                            ) {
                                                this.preview_video_source = Some(video);
                                                this.init_preview_audio_tracks();
                                            }
                                            cx.notify();
                                        }
                                    }))
                            )
                            .child(
                                Button::new("open-folder-details", "Show in Folder")
                                    .w_full()
                                    .variant(ButtonVariant::Outline)
                                    .icon(IconSource::Named("folder".to_string()))
                                    .on_click({
                                        let clip = clip.clone();
                                        move |_, _, _| {
                                            let _ = std::process::Command::new("explorer").arg("/select,").arg(&clip.path).spawn();
                                        }
                                    })
                            )
                    )
            )
    }

    fn render_detail_item(&self, label: &str, value: &str) -> impl IntoElement {
        let theme = use_theme();
        VStack::new()
            .gap_1()
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child(label.to_uppercase()))
            .child(div().text_sm().child(value.to_string()))
    }
}
