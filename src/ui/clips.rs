use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use crate::state::Clip;
use adabraka_ui::display::data_table::{ColumnDef, DataTable};
use adabraka_ui::components::input::Input;

/// Clips-library view state, grouped out of the `RekaptrWorkspace` god-object:
/// the cached clip list + filter/search/selection UI and the pending
/// delete/preview/popover interaction targets.
pub struct ClipsState {
    /// Clip pending deletion confirmation.
    pub to_delete: Option<Clip>,
    /// Clip open in the full mini-player overlay.
    pub to_preview: Option<Clip>,
    /// Right-click action popover: (anchor, clip).
    pub popover: Option<(Point<Pixels>, Clip)>,
    pub table: Entity<DataTable<Clip>>,
    pub search_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub search_expanded: bool,
    /// Paths of favorited clips (persisted in config).
    pub favorites: std::collections::HashSet<String>,
    /// Paths selected for batch actions.
    pub selected: std::collections::HashSet<String>,
    /// Clip shown in the details sidebar.
    pub selected_for_details: Option<Clip>,
    pub filter: ClipsFilter,
    /// Last fetched clip library (rebuilt when the Clips view opens). Kept
    /// across navigation so re-entry paints from cache instantly.
    pub cached: Vec<Clip>,
    /// Phase 1 (metadata fetch) in flight — drives the full-screen spinner.
    pub is_loading: bool,
    /// Phase 2 (background thumbnail backfill) in flight. Guards against
    /// overlapping refreshes without showing the spinner — the list is already
    /// up while thumbnails fill in.
    pub thumbs_backfilling: bool,
}

impl ClipsState {
    pub fn new(cx: &mut Context<RekaptrWorkspace>) -> Self {
        let table = cx.new(|cx| {
            DataTable::new(Vec::new(), RekaptrWorkspace::create_clip_columns(), cx)
        });
        Self {
            to_delete: None,
            to_preview: None,
            popover: None,
            table,
            search_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            search_expanded: false,
            favorites: crate::config::AppConfig::load_favorites(),
            selected: std::collections::HashSet::new(),
            selected_for_details: None,
            filter: ClipsFilter::All,
            cached: Vec::new(),
            is_loading: false,
            thumbs_backfilling: false,
        }
    }
}

/// Clips-page mini-player state, grouped out of the `RekaptrWorkspace`
/// god-object: the libmpv `Video` streaming the selected clip plus its hover
/// controls / scrub / volume UI.
pub struct ClipPreviewState {
    /// The clip currently loaded in the mini-player (`None` = closed).
    pub player: Option<crate::video_player::Video>,
    pub last_mouse_move: std::time::Instant,
    pub show_controls: bool,
    pub scrubbing: bool,
    pub scrub_progress: f32,
    pub vol_slider: Entity<crate::ui::volume_slider::VolumeSlider>,
    pub volume: f64,
    pub audio_enabled: Vec<bool>,
}

impl ClipPreviewState {
    pub fn new(cx: &mut Context<RekaptrWorkspace>) -> Self {
        let vh = cx.entity().downgrade();
        let vol_slider = cx.new(|cx| {
            crate::ui::volume_slider::VolumeSlider::new(cx)
                .with_value(100.0 / 150.0)
                .on_change(move |value, _window, cx| {
                    let _ = vh.update(cx, |this, cx| {
                        this.clip_preview.volume = (value * 150.0) as f64;
                        if let Some(v) = &this.clip_preview.player {
                            v.set_volume(this.clip_preview.volume);
                        }
                        cx.notify();
                    });
                })
        });
        Self {
            player: None,
            last_mouse_move: std::time::Instant::now(),
            show_controls: true,
            scrubbing: false,
            scrub_progress: 0.0,
            vol_slider,
            volume: 100.0,
            audio_enabled: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ClipsFilter {
    All,
    Favorites,
    Recent,
    Game(String),
}

impl RekaptrWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_selection = !self.clips.selected.is_empty();
        let filtered = self.clips_filtered(cx);
        let filtered_count = filtered.len();

        // Card rows need a *definite* width for flex-grow / flex-wrap to resolve
        // (the scroll container doesn't propagate one in this GPUI fork), so we
        // derive the inner content width from the window: viewport minus the
        // icon sidebar (72), the filter rail (240), the details panel when open
        // (320), and this view's horizontal padding (px_8 = 64).
        let details_w = if self.clips.selected_for_details.is_some() { 320.0 } else { 0.0 };
        let content_w = (window.viewport_size().width.0 - 72.0 - 240.0 - details_w - 64.0).max(280.0);

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
                            // Let this column shrink below its content's natural
                            // width (flex items default to min-width:auto), so the
                            // card rows reflow instead of clipping the window edge.
                            .min_w_0()
                            .h_full()
                            .child(self.render_clips_top_bar(filtered_count, cx))
                            .child(
                                div()
                                    .id("clips-scroll")
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_y_scroll()
                                    .child(self.render_clips_body(filtered, content_w, cx)),
                            )
                            .when(has_selection, |this| {
                                this.child(self.render_batch_actions_bar(cx))
                            }),
                    ),
            )
            .when_some(self.clips.selected_for_details.clone(), |this, clip| {
                this.child(self.render_clip_details_sidebar(clip, cx))
            })
            .when(self.clips.is_loading, |this| {
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

        if let Some(clip) = self.clips.to_preview.clone() {
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
        let q = self.clips.search_input.read(cx).content.to_lowercase();
        let favs = &self.clips.favorites;
        let mut clips: Vec<Clip> = self
            .clips.cached
            .iter()
            .filter(|c| match &self.clips.filter {
                ClipsFilter::All => true,
                ClipsFilter::Favorites => favs.contains(&c.path_str),
                ClipsFilter::Recent => true,
                ClipsFilter::Game(g) => &c.title == g,
            })
            .filter(|c| {
                if q.is_empty() {
                    return true;
                }
                c.title.to_lowercase().contains(&q)
                    || c.path_str.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if matches!(self.clips.filter, ClipsFilter::Recent) {
            clips.truncate(12);
        }

        clips
    }

    /// Per-game (title, clip-count) buckets for the "Games" folder posters,
    /// derived from already-filtered clips. `clips` is timestamp-DESC, so the
    /// first time we see a game is its most-recent clip — buckets stay in
    /// most-recently-active order, O(n) via a first-seen index map.
    fn clips_game_buckets(&self, clips: &[Clip]) -> Vec<(String, usize)> {
        let mut index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let mut games: Vec<(String, usize)> = Vec::new();
        for c in clips {
            if let Some(&i) = index.get(c.title.as_str()) {
                games[i].1 += 1;
            } else {
                index.insert(c.title.as_str(), games.len());
                games.push((c.title.clone(), 1));
            }
        }
        games
    }

    fn render_clips_filter_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let total = self.clips.cached.len();

        // Single pass: favorites count and per-game counts together, bucketing
        // games through a map (O(n)) instead of a linear find per clip (O(n²)).
        let mut fav_count = 0usize;
        let mut game_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let mut games: Vec<(String, usize)> = Vec::new();
        for c in &self.clips.cached {
            if self.clips.favorites.contains(&c.path_str) {
                fav_count += 1;
            }
            if let Some(&i) = game_index.get(c.title.as_str()) {
                games[i].1 += 1;
            } else {
                game_index.insert(c.title.as_str(), games.len());
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
        let active = self.clips.filter == filter;
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
                    this.clips.filter = filter_clone.clone();
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
        let title = match &self.clips.filter {
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
                    .child(self.render_clips_search(cx)),
            )
    }

    fn render_clips_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = use_theme();
        if self.clips.search_expanded {
            HStack::new()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .w(px(280.0))
                        .child(Input::new(&self.clips.search_input).placeholder("Search clips...")),
                )
                .child(
                    Button::new("clips-search-collapse", "")
                        .icon(IconSource::Named("x".to_string()))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clips.search_input
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.clips.search_expanded = false;
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
                        this.clips.search_expanded = true;
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
        filtered: Vec<Clip>,
        content_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        if filtered.is_empty() {
            // Still loading: render nothing here (the spinner overlay covers it)
            // so we never index into an empty `filtered` below.
            if self.clips.is_loading {
                return div().flex_1().into_any_element();
            }
            let msg = if self.clips.cached.is_empty() {
                "No clips found"
            } else {
                "No clips match this filter"
            };
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
                        .child(div().text_xl().text_color(theme.tokens.muted_foreground).child(msg))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground.opacity(0.7))
                                .child("Start recording to see your clips here"),
                        ),
                )
                .into_any_element();
        }

        // Non-"All" filters (a drilled-into game, Favorites, Recent) show a flat
        // responsive grid of every matching clip rather than the landing layout.
        if !matches!(self.clips.filter, ClipsFilter::All) {
            return self.render_clips_flat_grid(filtered, cx).into_any_element();
        }

        // "All" landing layout: hero (latest) → Most Recent (4) → Games (folders).
        let hero = filtered[0].clone();
        // The 4 newest after the hero, so the latest clip isn't shown twice.
        let recent: Vec<Clip> = filtered.iter().skip(1).take(4).cloned().collect();
        let games = self.clips_game_buckets(&filtered);

        VStack::new()
            .w_full()
            .gap_4()
            .pb_8()
            .child(self.render_clips_hero(hero, content_w, cx))
            .when(!recent.is_empty(), |this| {
                this.child(self.render_clips_recent_row(recent, content_w, cx))
            })
            .when(!games.is_empty(), |this| {
                this.child(self.render_game_folders(games, cx))
            })
            .into_any_element()
    }

    /// Flat responsive grid of clip cards — used for drilled-in / non-landing filters.
    fn render_clips_flat_grid(&self, clips: Vec<Clip>, cx: &mut Context<Self>) -> impl IntoElement {
        let view_handle = cx.entity().downgrade();
        div().px_8().pt_8().child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap_3()
                .children(clips.into_iter().map(|c| {
                    let is_sel = self.clips.selected.contains(&c.path_str);
                    let is_fav = self.clips.favorites.contains(&c.path_str);
                    Self::render_clip_card_advanced(c, &view_handle, is_sel, is_fav, 280.0)
                        .into_any_element()
                })),
        )
    }

    /// "Most Recent" row — the 4 newest clips across every game, playable
    /// directly. Cards split the (definite) content width 4-up and shrink with
    /// the window so they never wrap or clip at the default size.
    fn render_clips_recent_row(&self, clips: Vec<Clip>, content_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let view_handle = cx.entity().downgrade();
        // gap_3 = 12px between the 4 cards (3 gaps); a few px of slack keeps the
        // 4th card from wrapping to a second line under float rounding.
        let card_w = ((content_w - 44.0) / 4.0).max(120.0);
        VStack::new()
            .px_8()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Most Recent"),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .children(clips.into_iter().map(|c| {
                        let is_sel = self.clips.selected.contains(&c.path_str);
                        let is_fav = self.clips.favorites.contains(&c.path_str);
                        Self::render_clip_card_advanced(c, &view_handle, is_sel, is_fav, card_w)
                            .into_any_element()
                    })),
            )
    }

    /// "Games" section — one portrait (2:3) cover-art folder per game; clicking
    /// drills into that game's full clip grid. Columns reflow with the window.
    fn render_game_folders(&self, games: Vec<(String, usize)>, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .px_8()
            .gap_2()
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Games"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap_4()
                    .children(
                        games
                            .into_iter()
                            .map(|(game, count)| self.render_game_folder_card(game, count, cx)),
                    ),
            )
    }

    /// A single portrait game-folder poster. Lazily resolves the Steam library
    /// cover (`library_600x900`) through `cover_cache`, mirroring the dashboard's
    /// artwork-resolve pattern; falls back to a tinted placeholder until it loads.
    fn render_game_folder_card(&self, game: String, count: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        // Cache lookup + lazy background resolve (off the UI thread).
        let cached_cover = self.app_state.cover_cache.get(&game).and_then(|v| v.value().clone());
        if !self.app_state.cover_cache.contains_key(&game) {
            self.app_state.cover_cache.insert(game.clone(), None);
            let app_state = self.app_state.clone();
            let handle = cx.weak_entity();
            let title = game.clone();
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let resolved = cx
                        .background_executor()
                        .spawn({
                            let title = title.clone();
                            async move { crate::utils::find_steam_cover(&title) }
                        })
                        .await;
                    let Some(source) = resolved else { return };

                    if !source.starts_with("http") {
                        app_state.cover_cache.insert(title, Some(source.replace('\\', "/")));
                        let _ = handle.update(&mut cx, |_, cx| cx.notify());
                        return;
                    }

                    if let Ok(resp) = reqwest::get(&source).await {
                        if let Ok(bytes) = resp.bytes().await {
                            if bytes.len() > 5000 {
                                let app_id = source.split('/').nth(5).unwrap_or("unknown");
                                let cache_dir = crate::utils::get_storage_root().join("Cache").join("Artwork");
                                let _ = std::fs::create_dir_all(&cache_dir);
                                let local = cache_dir.join(format!("{}_cover.jpg", app_id));
                                if std::fs::write(&local, &bytes).is_ok() {
                                    let path = local.to_string_lossy().replace('\\', "/");
                                    app_state.cover_cache.insert(title, Some(path));
                                    let _ = handle.update(&mut cx, |_, cx| cx.notify());
                                }
                            }
                        }
                    }
                }
            })
            .detach();
        }

        let game_for_click = game.clone();
        let label = game.clone();

        div()
            .id(SharedString::from(format!("game-folder-{}", game)))
            .group("game-folder")
            .w(px(170.0))
            .flex_none()
            .flex()
            .flex_col()
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(theme.tokens.border)
            .bg(theme.tokens.card)
            .cursor_pointer()
            .hover(|s| s.border_color(theme.tokens.primary).shadow_lg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.clips.filter = ClipsFilter::Game(game_for_click.clone());
                    cx.notify();
                }),
            )
            // Portrait 2:3 cover area. Uses the shared `thumbnail` tile (Fill +
            // matching radius) so the cover can't spill the card's rounded
            // corners — test site for the component before any wider rollout.
            .child(
                crate::ui::thumbnail(
                    cached_cover.map(SharedString::from),
                    // Top-only: the cover is the card header; its bottom meets
                    // the title/count footer, so those corners stay square.
                    gpui::Corners {
                        top_left: px(12.0),
                        top_right: px(12.0),
                        bottom_left: px(0.0),
                        bottom_right: px(0.0),
                    },
                    theme.tokens.muted,
                )
                    .w_full()
                    .h(px(255.0))
                    // Folder/count chip over a bottom scrim.
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .h(px(72.0))
                            .bg(gpui::linear_gradient(
                                180.0,
                                gpui::linear_color_stop(gpui::rgba(0x0a0a0a00), 0.0),
                                gpui::linear_color_stop(gpui::rgba(0x0a0a0acc), 1.0),
                            )),
                    )
                    .child(
                        HStack::new()
                            .absolute()
                            .bottom_2()
                            .right_2()
                            .gap_1()
                            .items_center()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(gpui::rgba(0x000000_99))
                            .child(
                                Icon::new(IconSource::Named("folder".to_string()))
                                    .size(px(12.0))
                                    .color(gpui::white()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .child(format!("{}", count)),
                            ),
                    ),
            )
            .child(
                VStack::new()
                    .px_3()
                    .py_2()
                    .gap_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("{} clips", count)),
                    ),
            )
    }

    fn render_clips_hero(&self, clip: Clip, content_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clip_path = clip.path_str.clone();
        let is_fav = self.clips.favorites.contains(&clip_path);
        let clip_for_play = clip.clone();
        let clip_for_more = clip.clone();
        let thumb = clip.thumbnail_path.clone();
        let title = clip.title.clone();
        let date = clip.date.clone();
        let duration = clip.duration.clone();

        div().px_8().pt_6().child(
            div()
                .w(px(content_w))
                .h(px(240.0))
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
                                        .text_color(theme.tokens.primary_foreground)
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
                                    Button::new("clips-hero-more", "")
                                        .icon(IconSource::Named("ellipsis".to_string()))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Lg)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let mouse_pos = window.mouse_position();
                                            this.clips.popover = Some((mouse_pos, clip_for_more.clone()));
                                            cx.notify();
                                        })),
                                ),
                        ),
                ),
        )
    }

    /// Open a clip in the mini player overlay. Shared by the hero and card play buttons.
    pub fn open_clip_preview(&mut self, clip: Clip, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.clip_preview.player.as_ref().map(|v| v.render_image());
        self.clips.to_preview = Some(clip.clone());
        self.clip_preview.last_mouse_move = std::time::Instant::now();
        self.clip_preview.show_controls = true;
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
            self.clip_preview.player = Some(video);
            self.init_preview_audio_tracks();
        }
        if let Some(ri) = old {
            window.drop_image(ri).ok();
        }
        cx.notify();
    }
}

impl RekaptrWorkspace {
    /// `width` is the card's explicit pixel width; the thumbnail height scales
    /// to keep a 16:9 frame. Callers compute this from the available content
    /// width so rows reflow with the window instead of overflowing it.
    fn render_clip_card_advanced(clip: Clip, view_handle: &WeakEntity<Self>, is_selected: bool, is_favorited: bool, width: f32) -> impl IntoElement {
        let theme = use_theme();
        let view_handle_click = view_handle.clone();
        let view_handle_actions = view_handle.clone();
        let clip_for_mouse = clip.clone();
        let thumb_h = width * 9.0 / 16.0;

        div()
            .group("clip-card")
            .relative()
            .flex()
            .flex_col()
            .w(px(width))
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
                            if this.clips.selected.contains(&clip_path) {
                                this.clips.selected.remove(&clip_path);
                            } else {
                                this.clips.selected.insert(clip_path.clone());
                            }
                        } else {
                            this.clips.selected.clear();
                            this.clips.selected_for_details = Some(clip.clone());
                        }
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(thumb_h))
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
                                                let old = this.clip_preview.player.as_ref().map(|v| v.render_image());
                                                this.clips.to_preview = Some(clip.clone());
                                                this.clip_preview.last_mouse_move = std::time::Instant::now();
                                                this.clip_preview.show_controls = true;
                                                let url = clip.path.to_string_lossy().to_string();
                                                let d3d_device_ptr = this.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
                                                if let Ok(video) = crate::video_player::Video::new_with_options(
                                                    &url,
                                                    crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                    d3d_device_ptr,
                                                ) {
                                                    this.clip_preview.player = Some(video);
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
                                            .icon(IconSource::Named("ellipsis".to_string()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
                                            .on_click({
                                                let clip = clip.clone();
                                                let view_handle = view_handle_actions.clone();
                                                move |_, window, cx| {
                                                    let mouse_pos = window.mouse_position();
                                                    let clip = clip.clone();
                                                    let _ = view_handle.update(cx, |this, cx| {
                                                        this.clips.popover = Some((mouse_pos, clip.clone()));
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

        let (pos, dur) = if let Some(v) = &self.clip_preview.player {
            (v.position().as_secs_f64(), v.duration().as_secs_f64().max(1.0))
        } else {
            (0.0, 1.0)
        };
        
        let display_pos = if self.clip_preview.scrubbing {
            self.clip_preview.scrub_progress as f64 * dur
        } else {
            pos
        };
        
        let progress = (display_pos / dur) as f32;
        let controls_visible = self.clip_preview.show_controls;

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

        let window_width = window.viewport_size().width.0;
        let window_height = window.viewport_size().height.0;
        // Fit within the window on smaller displays instead of overflowing a
        // fixed 1120px box (which also broke the scrub-position math).
        let player_width = (window_width - 80.0).clamp(480.0, 1120.0);
        let player_height = (player_width * 630.0 / 1120.0).min(window_height - 80.0);
        let left_offset = (window_width - player_width) / 2.0;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.clips.to_preview = None;
                if let Some(old) = this.clip_preview.player.take() {
                    window.drop_image(old.render_image()).ok();
                }
                this.clip_preview.scrubbing = false;
                this.clip_preview.audio_enabled.clear();
                cx.notify();
            }))
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        this.clip_preview.last_mouse_move = std::time::Instant::now();
                        if !this.clip_preview.show_controls {
                            this.clip_preview.show_controls = true;
                        }
                        
                        if this.clip_preview.scrubbing {
                            let relative_x = event.position.x.0 - left_offset;
                            let p = (relative_x as f32 / player_width).clamp(0.0, 1.0);
                            this.clip_preview.scrub_progress = p;
                            if let Some(v) = &this.clip_preview.player {
                                let target = p as f64 * dur;
                                let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                            }
                        }
                        cx.notify();
                    }))
                    .id("mini-player-container")
                    .w(px(player_width))
                    .h(px(player_height))
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
                            .when_some(self.clip_preview.player.as_ref(), |this, v| {
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
                                        this.clips.to_preview = None;
                                        if let Some(old) = this.clip_preview.player.take() {
                                            window.drop_image(old.render_image()).ok();
                                        }
                                        this.clip_preview.audio_enabled.clear();
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
                                        this.clip_preview.scrubbing = true;
                                        let relative_x = event.position.x.0 - left_offset;
                                        let p = (relative_x as f32 / player_width).clamp(0.0, 1.0);
                                        this.clip_preview.scrub_progress = p;
                                        if let Some(v) = &this.clip_preview.player {
                                            let target = p as f64 * dur;
                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                        }
                                        cx.notify();
                                    }))
                                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.clip_preview.scrubbing = false;
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
                                                        if let Some(v) = &this.clip_preview.player {
                                                            let target = (v.position().as_secs_f64() - 5.0).max(0.0);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new("rotate-ccw").size(px(22.0)).color(gpui::white()))
                                            )
                                            .child({
                                                let is_paused = self.clip_preview.player.as_ref().map_or(true, |v| v.paused());
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
                                                        if let Some(v) = &this.clip_preview.player {
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
                                                        if let Some(v) = &this.clip_preview.player {
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
                                            .child(self.clip_preview.vol_slider.clone())
                                            // Audio track toggles
                                            .when(self.clip_preview.player.as_ref().map_or(false, |v| v.audio_tracks().len() > 1), |this| {
                                                let audio_tracks = self.clip_preview.player.as_ref()
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
                                                        let enabled = self.clip_preview.audio_enabled.get(idx).copied().unwrap_or(true);
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
                                                                if let Some(v) = this.clip_preview.audio_enabled.get_mut(idx) {
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
        let count = self.clips.selected.len();
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
                                this.clips.selected.clear();
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
                                for path_str in this.clips.selected.clone() {
                                    let path = std::path::PathBuf::from(path_str);
                                    let _ = std::fs::remove_file(&path);
                                    let mut thumb = path.clone();
                                    thumb.set_extension("jpg");
                                    let _ = std::fs::remove_file(thumb);
                                }
                                this.clips.selected.clear();
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
                                        this.clips.selected_for_details = None;
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
                                        .object_fit(ObjectFit::Cover)
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
                                            if let Some(old) = this.clip_preview.player.take() {
                                                window.drop_image(old.render_image()).ok();
                                            }
                                            this.clips.to_preview = Some(clip.clone());
                                            this.clip_preview.last_mouse_move = std::time::Instant::now();
                                            this.clip_preview.show_controls = true;
                                            let url = clip.path.to_string_lossy().to_string();
                                            let d3d_device_ptr = this.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
                                            if let Ok(video) = crate::video_player::Video::new_with_options(
                                                &url,
                                                crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                d3d_device_ptr,
                                            ) {
                                                this.clip_preview.player = Some(video);
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
