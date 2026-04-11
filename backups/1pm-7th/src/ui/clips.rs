use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, LumaWorkspace, ClipsViewMode};
use crate::state::Clip;
use std::collections::BTreeMap;
use chrono::{Local, NaiveDateTime, Datelike};
use adabraka_ui::display::data_table::{DataTable, ColumnDef};
use adabraka_ui::components::scrollable::scrollable_vertical;
use adabraka_ui::components::input::Input;
use adabraka_ui::virtual_list::vlist_uniform;
use std::sync::Arc;

impl LumaWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clips = crate::utils::fetch_all_clips();
        
        let search_query = self.clips_search_input.read(cx).content.to_lowercase();
        let mut filtered_clips: Vec<Clip> = clips.into_iter()
            .filter(|c| {
                if search_query.is_empty() { return true; }
                c.title.to_lowercase().contains(&search_query) || 
                c.path.to_string_lossy().to_lowercase().contains(&search_query)
            })
            .collect();

        // Ensure clips are sorted by timestamp DESC globally
        filtered_clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        VStack::new()
            .size_full()
            .child(
                HStack::new()
                    .p_8()
                    .pb_4()
                    .justify_between()
                    .items_center()
                    .child(
                        VStack::new()
                            .gap_1()
                            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("Clips Library"))
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(format!("{} Clips Found", filtered_clips.len())))
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
                    .flex_1()
                    .child(
                        match self.clips_view_mode {
                            ClipsViewMode::Grid => {
                                if filtered_clips.is_empty() {
                                    div().p_20().text_center().text_color(theme.tokens.muted_foreground).child("No clips match your search.").into_any_element()
                                } else {
                                    // Group clips into rows for virtualization
                                    let items_per_row = 4;
                                    
                                    // Pre-group by date for internal labels if we wanted, but for simple grid:
                                    let row_count = (filtered_clips.len() + items_per_row - 1) / items_per_row;
                                    let row_height = px(280.0);
                                    let clips_shared = Arc::new(filtered_clips);
                                    let view_handle = cx.entity().downgrade();

                                    vlist_uniform(
                                        "clips-vlist",
                                        row_count,
                                        row_height,
                                        move |range, _window, _cx| {
                                            let mut rows = Vec::new();
                                            for row_idx in range {
                                                let start = row_idx * items_per_row;
                                                let end = (start + items_per_row).min(clips_shared.len());
                                                let row_clips = &clips_shared[start..end];
                                                
                                                rows.push(
                                                    HStack::new()
                                                        .px_8()
                                                        .py_3()
                                                        .gap_6()
                                                        .children(row_clips.iter().enumerate().map(|(i, clip): (usize, &Clip)| {
                                                            let clip_idx = start + i;
                                                            let view_handle = view_handle.clone();
                                                            let clip = clip.clone();
                                                            Self::render_clip_card_simple(&clip, clip_idx, &view_handle)
                                                        }))
                                                );
                                            }
                                            rows
                                        }
                                    )
                                    .track_scroll(&self.clips_scroll_handle)
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
    }

    pub fn create_clip_columns() -> Vec<ColumnDef<Clip>> {
        vec![
            ColumnDef::new("title", "Title", |c: &Clip| c.title.clone().into()).width(px(300.0)),
            ColumnDef::new("date", "Date", |c: &Clip| c.date.clone().into()).width(px(150.0)),
            ColumnDef::new("size", "Size", |c: &Clip| c.size.clone().into()).width(px(100.0)),
        ]
    }

    fn render_clip_card_simple(clip: &Clip, idx: usize, view_handle: &WeakEntity<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clip_clone = clip.clone();
        let view_handle_play = view_handle.clone();
        let view_handle_actions = view_handle.clone();
        
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
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(157.0))
                    .bg(rgb(0x000000))
                    .children(
                        clip.thumbnail_path.as_ref().map(|path| {
                            img(path.to_string_lossy().to_string())
                                .size_full()
                        })
                    )
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
                                Button::new(("play-btn", idx), "")
                                    .icon(IconSource::Named("play".to_string()))
                                    .variant(ButtonVariant::Default)
                                    .on_click(move |_, window, cx| {
                                        let clip = clip_clone.clone();
                                        let _ = view_handle_play.update(cx, |this, cx| {
                                            this.set_active_view(ActiveView::Dashboard, cx);
                                            this.load_video(&clip.path.to_string_lossy(), window, cx);
                                        });
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
                                        Button::new(("actions-btn", idx), "")
                                            .icon(IconSource::Named("plus".to_string()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
                                            .on_click({
                                                let clip = clip.clone();
                                                move |_, window, cx| {
                                                    let mouse_pos = window.mouse_position();
                                                    let clip = clip.clone();
                                                    let _ = view_handle_actions.update(cx, |this, cx| {
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
}
