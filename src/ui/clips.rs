use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{LumaWorkspace, ClipsViewMode};
use crate::state::Clip;
use std::collections::BTreeMap;
use adabraka_ui::display::data_table::ColumnDef;
use adabraka_ui::components::input::Input;

impl LumaWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let all_clips = self.cached_clips.clone();
        
        let search_query = self.clips_search_input.read(cx).content.to_lowercase();
        let mut filtered_clips: Vec<Clip> = all_clips.into_iter()
            .filter(|c| {
                if search_query.is_empty() { return true; }
                c.title.to_lowercase().contains(&search_query) || 
                c.path.to_string_lossy().to_lowercase().contains(&search_query)
            })
            .collect();

        // Global sort by timestamp DESC
        filtered_clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let has_selection = !self.selected_clips.is_empty();

        let mut root = div()
            .size_full()
            .flex()
            .relative()
            .child(
                VStack::new()
                    .flex_1()
                    .child(
                        // Header Area
                        HStack::new()
                            .px_8()
                            .py_6()
                            .justify_between()
                            .items_center()
                            .child(
                                HStack::new()
                                    .gap_4()
                                    .items_center()
                                    .when_some(self.selected_game_filter.clone(), |this, game| {
                                        this.child(
                                            Button::new("back-to-library", "")
                                                .icon(IconSource::Named("chevron-left".to_string()))
                                                .variant(ButtonVariant::Default)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.selected_game_filter = None;
                                                    this.rebuild_library_items(cx);
                                                    cx.notify();
                                                }))
                                        )
                                        .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(game))
                                    })
                                    .when(self.selected_game_filter.is_none(), |this| {
                                        this.child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("Clips Library"))
                                    })
                            )
                            .child(
                                HStack::new()
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        div()
                                            .w(px(300.0))
                                            .child(Input::new(&self.clips_search_input).placeholder("Search clips..."))
                                    )
                                    .child(
                                        HStack::new()
                                            .bg(theme.tokens.muted)
                                            .p_1()
                                            .rounded_md()
                                            .child(
                                                Button::new("view-grid", "")
                                                    .icon(IconSource::Named("layout-dashboard".to_string()))
                                                    .variant(if self.clips_view_mode == ClipsViewMode::Grid { ButtonVariant::Default } else { ButtonVariant::Ghost })
                                                    .size(ButtonSize::Sm)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.clips_view_mode = ClipsViewMode::Grid;
                                                        cx.notify();
                                                    }))
                                            )
                                            .child(
                                                Button::new("view-table", "")
                                                    .icon(IconSource::Named("video".to_string()))
                                                    .variant(if self.clips_view_mode == ClipsViewMode::Table { ButtonVariant::Default } else { ButtonVariant::Ghost })
                                                    .size(ButtonSize::Sm)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.clips_view_mode = ClipsViewMode::Table;
                                                        let clips = this.cached_clips.clone();
                                                        this.clip_table.update(cx, |table, cx| {
                                                            table.set_data(clips, cx);
                                                        });
                                                        cx.notify();
                                                    }))
                                            )
                                    )
                            )
                    )
                    .child(
                        div()
                            .id("clips-scroll-area")
                            .flex_1()
                            .child(
                                match self.clips_view_mode {
                                    ClipsViewMode::Grid => {
                                        if self.library_items.is_empty() && !self.is_loading_clips {
                                            div().flex_1().flex().items_center().justify_center().py_20().child(
                                                VStack::new()
                                                    .items_center()
                                                    .gap_4()
                                                    .child(Icon::new("video").size(px(64.0)).color(theme.tokens.muted_foreground.opacity(0.5)))
                                                    .child(div().text_xl().text_color(theme.tokens.muted_foreground).child("No clips found"))
                                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground.opacity(0.7)).child("Start recording to see your clips here"))
                                            ).into_any_element()
                                        } else {
                                            let view_handle = cx.entity().downgrade();
                                            let app_state = self.app_state.clone();
                                            list(self.clips_list_state.clone(), move |i, _window, cx| {
                                                let Some(view) = view_handle.upgrade() else {
                                                    return div().into_any_element();
                                                };
                                                let row = &view.read(cx).library_items[i];
                                                
                                                match row {
                                                    LibraryRow::SectionHeader(title) => {
                                                        div()
                                                            .px_8()
                                                            .py_3()
                                                            .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(use_theme().tokens.muted_foreground).child(title.clone()))
                                                            .into_any_element()
                                                    }
                                                    LibraryRow::ClipChunk(clips) => {
                                                        let chunk_view_handle = view_handle.clone();
                                                        let mut row = HStack::new().px_8().gap_6().py_2();
                                                        // Get selection state from view.read(cx)
                                                        let is_selected_map: Vec<bool> = clips.iter().map(|c| {
                                                            let path = c.path.to_string_lossy().to_string();
                                                            view.read(cx).selected_clips.contains(&path)
                                                        }).collect();

                                                        for (idx, clip) in clips.iter().enumerate() {
                                                            row = row.child(Self::render_clip_card_advanced(clip.clone(), &chunk_view_handle, is_selected_map[idx]));
                                                        }
                                                        row.into_any_element()
                                                    }
                                                    LibraryRow::GameChunk(games) => {
                                                        let chunk_view_handle = view_handle.clone();
                                                        let chunk_app_state = app_state.clone();
                                                        HStack::new()
                                                            .px_8()
                                                            .gap_6()
                                                            .py_3()
                                                            .children(games.iter().map(|(title, count)| {
                                                                Self::render_game_card_vertical(title.clone(), *count, &chunk_app_state, chunk_view_handle.clone())
                                                            }))
                                                            .into_any_element()
                                                    }
                                                }
                                            })
                                            .size_full()
                                            .into_any_element()
                                        }
                                    }
                                    ClipsViewMode::Table => {
                                        div().p_8().pt_0().child(self.clip_table.clone()).into_any_element()
                                    }
                                }
                            )
                    )
                    .when(has_selection, |this| {
                        this.child(self.render_batch_actions_bar(cx))
                    })
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
                        .child(Spinner::new().size(SpinnerSize::Xl))
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
}

#[derive(Clone)]
pub enum LibraryRow {
    SectionHeader(String),
    ClipChunk(Vec<Clip>),
    GameChunk(Vec<(String, usize)>),
}

impl LumaWorkspace {
    fn render_clips_grid_new_style(&self, clips: Vec<Clip>, cx: &mut Context<Self>) -> impl IntoElement {
        let view_handle = cx.entity().downgrade();
        let app_state = self.app_state.clone();

        // If a game is selected, we only show clips for that game
        if let Some(game_title) = &self.selected_game_filter {
            let game_clips: Vec<Clip> = clips.into_iter().filter(|c| &c.title == game_title).collect();

            // Use a chunked approach for the grid to keep the element tree manageable
            let mut list = VStack::new().gap_4().pb_8();
            for chunk in game_clips.chunks(4) {
                let mut row = HStack::new().px_8().gap_6();
                for clip in chunk {
                    let path = clip.path.to_string_lossy().to_string();
                    let is_selected = self.selected_clips.contains(&path);
                    row = row.child(Self::render_clip_card_advanced(clip.clone(), &view_handle, is_selected));
                }
                list = list.child(row);
            }
            return list;
        }

        // Dashboard Style: Recent + Game Groups
        let mut game_groups: BTreeMap<String, Vec<Clip>> = BTreeMap::new();
        for clip in clips.iter() {
            game_groups.entry(clip.title.clone()).or_default().push(clip.clone());
        }

        let mut content = VStack::new().gap_0().pb_8();

        // 1. Most Recent Section (Top 4)
        if !clips.is_empty() {
            let recent_clips: Vec<Clip> = clips.iter().take(4).cloned().collect();
            let mut recent_row = HStack::new().px_8().gap_6();
            for clip in recent_clips {
                let path = clip.path.to_string_lossy().to_string();
                let is_selected = self.selected_clips.contains(&path);
                recent_row = recent_row.child(Self::render_clip_card_advanced(clip, &view_handle, is_selected));
            }
            
            content = content.child(
                div()
                    .px_8()
                    .py_3()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(use_theme().tokens.muted_foreground).child("MOST RECENT"))
            ).child(recent_row);
        }

        // 2. Games Section
        content = content.child(
            div()
                .px_8()
                .pt_8()
                .pb_3()
                .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(use_theme().tokens.muted_foreground).child("GAMES"))
        );

        let game_titles: Vec<String> = game_groups.keys().cloned().collect();
        for chunk in game_titles.chunks(4) {
            let view_handle = view_handle.clone();
            let app_state = app_state.clone();
            
            // Map the chunk to a vec of (title, count)
            let titles_with_data: Vec<(String, usize)> = chunk.iter()
                .map(|title| {
                    let count = game_groups.get(title).map_or(0, |v| v.len());
                    (title.clone(), count)
                })
                .collect();

            // Trigger portrait artwork fetches for this chunk
            let chunk_titles: Vec<String> = titles_with_data.iter().map(|(t, _)| t.clone()).collect();
            self.fetch_portrait_artwork(&chunk_titles, cx);

            let row = HStack::new()
                .px_8()
                .gap_6()
                .py_3()
                .children(titles_with_data.into_iter().map(move |(title, count)| {
                    Self::render_game_card_vertical(title, count, &app_state, view_handle.clone())
                }));
            content = content.child(row);
        }

        content
    }

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

    fn render_game_card_vertical(title: String, clip_count: usize, app_state: &std::sync::Arc<crate::state::AppState>, view_handle: WeakEntity<Self>) -> impl IntoElement {
        let theme = use_theme();
        let title_for_click = title.clone();

        // Read from portrait cache (fetch was triggered earlier)
        let cached_path = app_state.portrait_cache.get(&title).map(|v| v.value().clone()).flatten();
        let final_image_path: Option<std::path::PathBuf> = cached_path.map(std::path::PathBuf::from);

        div()
            .group("game-card")
            .relative()
            .w(px(200.0))
            .h(px(300.0))
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .rounded_xl()
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let title = title_for_click.clone();
                let _ = view_handle.update(cx, |this, cx| {
                    this.selected_game_filter = Some(title);
                    this.rebuild_library_items(cx);
                    cx.notify();
                });
            })
            .child(
                div()
                    .size_full()
                    .bg(theme.tokens.muted)
                    .when_some(final_image_path, |this, img_path| {
                        let img_path_str = format!("file://{}", img_path.to_string_lossy().replace("\\", "/"));
                        this.child(
                            img(img_path_str)
                                .size_full()
                                .object_fit(gpui::ObjectFit::Cover)
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(gpui::rgba(0x000000_88))
                            .flex()
                            .flex_col()
                            .justify_end()
                            .p_6()
                            .gap_1()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(gpui::white()).child(title.clone()))
                            .child(div().text_sm().text_color(gpui::rgba(0xffffff_aa)).child(format!("{} Clips", clip_count)))
                    )
            )
    }

    fn render_clip_card_advanced(clip: Clip, view_handle: &WeakEntity<Self>, is_selected: bool) -> impl IntoElement {
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
                    .bg(rgb(0x000000))
                    .when_some(clip.thumbnail_path.as_ref(), |this, path| {
                        this.child(
                            img(path.to_string_lossy().to_string())
                                .size_full()
                                .object_fit(ObjectFit::Cover)
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
                                        move |_, _, cx| {
                                            cx.stop_propagation();
                                            let _ = view_handle.update(cx, |this, cx| {
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
                                            });
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

        if self.preview_volume_dragging {
            let view_handle = cx.entity().downgrade();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble {
                    let _ = view_handle.update(cx, |this, cx| {
                        let bounds = this.preview_volume_bounds;
                        if bounds.size.width > px(0.0) {
                            let rel_x = (event.position.x - bounds.left()).max(px(0.0));
                            let frac = (rel_x / bounds.size.width).clamp(0.0, 1.0);
                            this.preview_volume = (frac * 150.0) as f64;
                            if let Some(v) = &this.preview_video_source {
                                v.set_volume(this.preview_volume);
                            }
                            cx.notify();
                        }
                    });
                }
            });
            let view_handle = cx.entity().downgrade();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    let _ = view_handle.update(cx, |this, cx| {
                        this.preview_volume_dragging = false;
                        cx.notify();
                    });
                }
            });
        }

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
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.clip_to_preview = None;
                this.preview_video_source = None;
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
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.clip_to_preview = None;
                                        this.preview_video_source = None;
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
                                    .p_6()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div().text_sm().font_weight(FontWeight::MEDIUM).text_color(gpui::white())
                                            .child(format!("{} / {}", format_time(display_pos), format_time(dur)))
                                    )
                                    .child(
                                        HStack::new()
                                            .gap_12()
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(v) = &this.preview_video_source {
                                                            let target = (v.position().as_secs_f64() - 5.0).max(0.0);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new("rotate-ccw").size(px(32.0)).color(theme.tokens.primary))
                                            )
                                            .child({
                                                let is_paused = self.preview_video_source.as_ref().map_or(true, |v| v.paused());
                                                div()
                                                    .cursor_pointer()
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
                                                    .child(Icon::new(if is_paused { "play" } else { "pause" }).size(px(40.0)).color(theme.tokens.primary))
                                            })
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        if let Some(v) = &this.preview_video_source {
                                                            let duration = v.duration().as_secs_f64();
                                                            let target = (v.position().as_secs_f64() + 5.0).min(duration);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(target), false);
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(Icon::new("rotate-cw").size(px(32.0)).color(theme.tokens.primary))
                                            )
                                    )
                                    .child({
                                        let audio_tracks = self.preview_video_source.as_ref()
                                            .map(|v| v.audio_tracks())
                                            .unwrap_or_default();
                                        let theme = use_theme();
                                        let vol_frac = (self.preview_volume / 100.0).clamp(0.0, 1.5) as f32;
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_3()
                                            .items_center()
                                            // Volume slider
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Icon::new("speaker").size(px(16.0)).color(gpui::hsla(0.0, 0.0, 1.0, 0.67))
                                                    )
                                                    .child(
                                                        div()
                                                            .id("preview-vol-slider")
                                                            .w(px(80.0))
                                                            .h(px(16.0))
                                                            .cursor_pointer()
                                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                                                this.preview_volume_dragging = true;
                                                                let bounds = this.preview_volume_bounds;
                                                                if bounds.size.width > px(0.0) {
                                                                    let rel_x = (event.position.x - bounds.left()).max(px(0.0));
                                                                    let frac = (rel_x / bounds.size.width).clamp(0.0, 1.0);
                                                                    this.preview_volume = (frac * 150.0) as f64;
                                                                    if let Some(v) = &this.preview_video_source {
                                                                        v.set_volume(this.preview_volume);
                                                                    }
                                                                }
                                                                cx.notify();
                                                            }))
                                                            .child(
                                                                canvas(
                                                                    move |_, window, cx| {
                                                                        let layout_id = window.request_layout(Style {
                                                                            size: size(px(80.0).into(), px(4.0).into()),
                                                                            ..Default::default()
                                                                        }, [], cx);
                                                                        (layout_id, ())
                                                                    },
                                                                    {
                                                                        let view_handle = cx.entity().downgrade();
                                                                        let theme = theme.clone();
                                                                        move |bounds, _, window, cx| {
                                                                            let _ = view_handle.update(cx, |this, _cx| {
                                                                                this.preview_volume_bounds = bounds;
                                                                            });
                                                                            // Background
                                                                            window.paint_quad(fill(bounds, theme.tokens.muted).corner_radii(px(2.0)));
                                                                            // Fill
                                                                            let fill_frac = (vol_frac / 1.5).clamp(0.0, 1.0);
                                                                            let fill_w = bounds.size.width * fill_frac;
                                                                            let fill_rect = Bounds::new(bounds.origin, size(fill_w, bounds.size.height));
                                                                            window.paint_quad(fill(fill_rect, theme.tokens.primary).corner_radii(px(2.0)));
                                                                            // Thumb
                                                                            let thumb_x = (bounds.left() + fill_w - px(4.0)).max(bounds.left());
                                                                            let thumb_rect = Bounds::new(
                                                                                point(thumb_x, bounds.top() - px(3.0)),
                                                                                size(px(8.0), px(10.0))
                                                                            );
                                                                            window.paint_quad(fill(thumb_rect, gpui::white()).corner_radii(px(4.0)));
                                                                        }
                                                                    }
                                                                )
                                                            )
                                                    )
                                            )
                                            // Track toggle buttons
                                            .when(audio_tracks.len() > 1, |this| {
                                                let audio_tracks = self.preview_video_source.as_ref()
                                                    .map(|v| v.audio_tracks())
                                                    .unwrap_or_default();
                                                let theme = use_theme();
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .gap_1()
                                                        .items_center()
                                                        .children(audio_tracks.into_iter().enumerate().map(|(idx, _)| {
                                                            let enabled = self.preview_audio_enabled.get(idx).copied().unwrap_or(true);
                                                            let theme = theme.clone();
                                                            div()
                                                                .id(("track-toggle", idx))
                                                                .cursor_pointer()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .w(px(22.0))
                                                                .h(px(22.0))
                                                                .rounded(px(4.0))
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .when(enabled, |this| {
                                                                    this.bg(theme.tokens.primary)
                                                                        .text_color(theme.tokens.primary_foreground)
                                                                        .hover(|s| s.bg(gpui::hsla(258.0/360.0, 0.90, 0.56, 1.0)))
                                                                })
                                                                .when(!enabled, |this| {
                                                                    this.bg(theme.tokens.muted)
                                                                        .text_color(theme.tokens.muted_foreground)
                                                                        .hover(|s| s.text_color(gpui::white()))
                                                                })
                                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                                    this.init_preview_audio_tracks();
                                                                    if let Some(v) = this.preview_audio_enabled.get_mut(idx) {
                                                                        *v = !*v;
                                                                    }
                                                                    this.update_preview_audio_mix();
                                                                    cx.notify();
                                                                }))
                                                                .child(format!("{}", idx + 1))
                                                        }))
                                                )
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
                                        move |this, _, _, cx| {
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
