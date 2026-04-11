//! Clip library management — loading, filtering, and organizing saved clips.
//!
//! The library presents clips in two layouts:
//! - **Unfiltered (all games)**: Shows a "Most Recent" section (last 4 clips across all
//!   games), followed by a "Games" section with game title cards grouped alphabetically.
//!   Each game card shows clip count and artwork (cached via `artwork_cache`).
//! - **Filtered (single game)**: Shows all clips for that game in a flat grid, sorted
//!   newest-first.
//!
//! Clips are loaded asynchronously via `cx.background_executor()` to avoid blocking the
//! UI thread — scanning the Clips directory can be slow with hundreds of files. The loaded
//! clips are cached in `cached_clips` and only re-fetched when the view is entered or
//! a clip is created/deleted.

use crate::config::AppConfig;
use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;

impl LumaWorkspace {
    /// Triggers an async reload of all clips from disk.
    /// Guards against concurrent loads with `is_loading_clips`.
    pub fn refresh_clips(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_clips { return; }
        self.is_loading_clips = true;

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let clips = cx.background_executor().spawn(async move {
                    crate::utils::fetch_all_clips()
                }).await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.cached_clips = clips.clone();
                    this.is_loading_clips = false;

                    this.recalculate_library_items(cx);

                    this.clip_table.update(cx, |table, cx| {
                        table.set_data(clips, cx);
                    });

                    cx.notify();
                });
            }
        }).detach();

        cx.notify();
    }

    /// Rebuilds the library row list from the cached clips.
    ///
    /// Applies the current search query and game filter, then organizes results into
    /// `LibraryRow` items (section headers, clip chunks of 4, game title chunks of 4).
    /// This is called synchronously after clips load — the filtering itself is fast since
    /// clips are already in memory; only the disk I/O is async.
    pub fn recalculate_library_items(&mut self, cx: &mut App) {
        let clips = self.cached_clips.clone();
        let search_query = self.clips_search_input.read(cx).content.to_lowercase();

        let mut filtered_clips: Vec<_> = clips.into_iter()
            .filter(|c| {
                let matches_search = search_query.is_empty() ||
                    c.title.to_lowercase().contains(&search_query) ||
                    c.path.to_string_lossy().to_lowercase().contains(&search_query);

                let matches_game = self.selected_game_filter.is_none() ||
                    self.selected_game_filter.as_ref() == Some(&c.title);

                matches_search && matches_game
            })
            .collect();

        filtered_clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let mut rows = Vec::new();

        if let Some(_game_title) = &self.selected_game_filter {
            for chunk in filtered_clips.chunks(4) {
                rows.push(crate::ui::clips::LibraryRow::ClipChunk(chunk.to_vec()));
            }
        } else {
            if !filtered_clips.is_empty() {
                rows.push(crate::ui::clips::LibraryRow::SectionHeader("MOST RECENT".to_string()));
                let recent: Vec<_> = filtered_clips.iter().take(4).cloned().collect();
                rows.push(crate::ui::clips::LibraryRow::ClipChunk(recent));
            }

            rows.push(crate::ui::clips::LibraryRow::SectionHeader("GAMES".to_string()));

            let mut game_groups: std::collections::BTreeMap<String, Vec<crate::state::Clip>> = std::collections::BTreeMap::new();
            for clip in filtered_clips.iter() {
                game_groups.entry(clip.title.clone()).or_default().push(clip.clone());
            }

            let game_titles: Vec<String> = game_groups.keys().cloned().collect();
            for chunk in game_titles.chunks(4) {
                let titles_with_data: Vec<(String, usize, Option<String>)> = chunk.iter()
                    .map(|title| {
                        let count = game_groups.get(title).map_or(0, |v| v.len());
                        let cached_path = self.app_state.artwork_cache.get(title).map(|v| v.value().clone()).flatten();
                        (title.clone(), count, cached_path)
                    })
                    .collect();
                rows.push(crate::ui::clips::LibraryRow::GameChunk(titles_with_data));
            }
        }

        self.library_items = rows;
        self.clips_list_state.reset(self.library_items.len());
    }

    pub fn delete_session(&mut self, session_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = self.app_state.manual_sessions.get(&session_id) {
            let title = session.title.clone();
            self.app_state.manual_sessions.remove(&session_id);
            self.app_state.game_registry.remove(&title);
            let mut config = AppConfig::load();
            config.game_registry.remove(&title);
            config.save();
            self.show_toast("Source Deleted", Some(format!("Removed {} from library.", title)), adabraka_ui::overlays::toast::ToastVariant::Default, window, cx);
            self.session_to_delete = None;
            cx.notify();
        }
    }

    pub fn delete_clip(&mut self, clip: crate::state::Clip, window: &mut Window, cx: &mut Context<Self>) {
        let _ = std::fs::remove_file(&clip.path);
        if let Some(thumb) = &clip.thumbnail_path {
            let _ = std::fs::remove_file(thumb);
        }
        self.show_toast("Clip Deleted", Some("The file has been removed from disk."), adabraka_ui::overlays::toast::ToastVariant::Default, window, cx);
        self.clip_to_delete = None;
        self.refresh_clips(cx);
        cx.notify();
    }
}
