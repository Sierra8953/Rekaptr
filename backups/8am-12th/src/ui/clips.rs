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
        let all_clips = crate::utils::fetch_all_clips();
        
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
                                                        let clips = crate::utils::fetch_all_clips();
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
                            .overflow_y_scroll()
                            .child(
                                match self.clips_view_mode {
                                    ClipsViewMode::Grid => {
                                        if filtered_clips.is_empty() {
                                            div().p_20().text_center().text_color(theme.tokens.muted_foreground).child("No clips match your search.").into_any_element()
                                        } else {
                                            self.render_clips_grid_new_style(filtered_clips, cx).into_any_element()
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

    fn render_clips_grid_new_style(&self, clips: Vec<Clip>, cx: &mut Context<Self>) -> impl IntoElement {
        let view_handle = cx.entity().downgrade();
        let app_state = self.app_state.clone();

        if let Some(game_title) = &self.selected_game_filter {
            let game_clips: Vec<Clip> = clips.into_iter().filter(|c| &c.title == game_title).collect();

            return VStack::new()
                .gap_4()
                .pb_8()
                .children(game_clips.chunks(4).map(|chunk| {
                    let view_handle = view_handle.clone();
                    HStack::new()
                        .px_8()
                        .gap_6()
                        .children(chunk.iter().map(move |clip| {
                            Self::render_clip_card_advanced(clip.clone(), &view_handle)
                        }))
                }));
        }

        let recent_clips = clips.iter().take(4).cloned().collect::<Vec<_>>();

        let mut game_groups: BTreeMap<String, Vec<Clip>> = BTreeMap::new();
        for clip in clips.iter() {
            game_groups.entry(clip.title.clone()).or_default().push(clip.clone());
        }

        let mut content = VStack::new().gap_0().pb_8();

        if !recent_clips.is_empty() {
            content = content.child(
                div()
                    .px_8()
                    .py_3()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(use_theme().tokens.muted_foreground).child("MOST RECENT"))
            ).child(
                HStack::new()
                    .px_8()
                    .gap_6()
                    .children(recent_clips.iter().map(|clip| {
                        Self::render_clip_card_advanced(clip.clone(), &view_handle)
                    }))
            );
        }

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
            let row = HStack::new()
                .px_8()
                .gap_6()
                .py_3()
                .children(chunk.iter().map(|title| {
                    let count = game_groups.get(title).map_or(0, |v| v.len());
                    Self::render_game_card_vertical(title.clone(), count, app_state.clone(), view_handle.clone(), cx)
                }));
            content = content.child(row);
        }

        content
    }

    fn render_game_card_vertical(title: String, clip_count: usize, app_state: std::sync::Arc<crate::state::AppState>, view_handle: WeakEntity<Self>, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let title_for_click = title.clone();

        let cached_path = app_state.artwork_cache.get(&title).map(|v| v.value().clone()).flatten();
        if cached_path.is_none() && !app_state.artwork_cache.contains_key(&title) {
            let artwork_url_or_path = crate::utils::find_steam_artwork(&title);
            if let Some(source) = artwork_url_or_path {
                if !source.starts_with("http") {
                    app_state.artwork_cache.insert(title.clone(), Some(source));
                } else {
                    let url = source.clone();
                    let handle = cx.weak_entity();
                    let title_cache = title.clone();
                    let title_log = title.clone();

                    app_state.artwork_cache.insert(title_cache.clone(), None);

                    cx.spawn(move |_, cx: &mut AsyncApp| {
                        let app_state = app_state.clone();
                        let handle = handle.clone();
                        let mut cx = cx.clone();
                        let url = url.clone();
                        let title_cache = title_cache.clone();
                        let title_log = title_log.clone();
                        async move {
                            eprintln!("[UI] Starting download for '{}' from {}", title_log, url);
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
                                    eprintln!("[UI] Saved artwork for '{}' to {:?}", title_log, local_path);
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
                app_state.artwork_cache.insert(title.clone(), None);
            }
        }

        let final_image_path: Option<std::path::PathBuf> = cached_path.map(std::path::PathBuf::from);

        div()
            .group("game-card")
            .relative()
            .w(px(280.0))
            .h(px(320.0))
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
    fn render_clip_card_advanced(clip: Clip, view_handle: &WeakEntity<Self>) -> impl IntoElement {
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
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .rounded_xl()
            .overflow_hidden()
            .cursor_pointer()
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
                    .h(px(157.0))
                    .bg(rgb(0x000000))
                    .when_some(clip.thumbnail_path.as_ref(), |this, path| {
                        this.child(
                            img(path.to_string_lossy().to_string())
                                .size_full()
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(gpui::rgba(0x000000_66))
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.0)
                            .group_hover("clip-card", |this| this.opacity(1.0))
                            .child(
                                Button::new(("preview-btn", clip.timestamp), "")
                                    .icon(IconSource::Named("play".to_string()))
                                    .variant(ButtonVariant::Default)
                                    .on_click({
                                        let clip = clip.clone();
                                        let view_handle = view_handle_click.clone();
                                        move |_, _, cx| {
                                            let _ = view_handle.update(cx, |this, cx| {
                                                this.clip_to_preview = Some(clip.clone());
                                                this.last_preview_mouse_move = std::time::Instant::now();
                                                this.show_preview_controls = true;
                                                let url = clip.path.to_string_lossy().to_string();
                                                let d3d_device_handle = this.app_state.d3d11_device.lock().unwrap().0;
                                                if let Ok(video) = crate::video_player::Video::new_with_options(
                                                    &url,
                                                    crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                    Some(d3d_device_handle.0),
                                                ) {
                                                    this.preview_video_source = Some(video);
                                                }
                                                cx.notify();
                                            });
                                        }
                                    })
                            )
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

        let format_time = |s: f64| {
            let total_seconds = s as i64;
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{}:{:02}", minutes, seconds)
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
                                    .child(div().w(px(100.0))) // Spacer to balance timestamp
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
                                            let d3d_device_handle = this.app_state.d3d11_device.lock().unwrap().0;
                                            if let Ok(video) = crate::video_player::Video::new_with_options(
                                                &url,
                                                crate::video_player::VideoOptions { source_name: Some("preview".to_string()), ..Default::default() },
                                                Some(d3d_device_handle.0),
                                            ) {
                                                this.preview_video_source = Some(video);
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

enum NewStyleItem {
    SectionHeader(String),
    RecentRow(Vec<Clip>),
    GameRow(Vec<String>),
}

enum GroupItem {
    Header(String),
    Row(Vec<Clip>),
}
