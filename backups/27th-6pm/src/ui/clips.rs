use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, LumaWorkspace};
use crate::state::Clip;
use std::collections::BTreeMap;

impl LumaWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clips = crate::utils::fetch_all_clips();
        
        let mut total_size_mb = 0;
        let mut grouped_clips: BTreeMap<String, Vec<Clip>> = BTreeMap::new();
        
        for clip in &clips {
            let size_str = clip.size.replace(" MB", "");
            if let Ok(mb) = size_str.parse::<u64>() {
                total_size_mb += mb;
            }
            
            grouped_clips.entry(clip.title.clone())
                .or_default()
                .push(clip.clone());
        }
        
        for game_clips in grouped_clips.values_mut() {
            game_clips.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        }

        let total_size_str = if total_size_mb > 1024 {
            format!("{:.1} GB", total_size_mb as f64 / 1024.0)
        } else {
            format!("{} MB", total_size_mb)
        };

        if clips.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    VStack::new()
                        .items_center()
                        .gap_4()
                        .child(Icon::new("video").size(px(64.0)).color(theme.tokens.muted_foreground))
                        .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child("No clips found"))
                        .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Head to the Dashboard to record your first moment!"))
                        .child(
                            Button::new("go-to-dash", "Go to Dashboard")
                                .variant(ButtonVariant::Default)
                                .mt_4()
                                .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                    this.active_view = ActiveView::Dashboard;
                                    cx.notify();
                                }))
                        )
                )
                .into_any_element();
        }

        VStack::new()
            .flex_1()
            .h_full()
            .child(
                HStack::new()
                    .p_8()
                    .pb_4()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.foreground)
                            .child("Clips Library")
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("{} Clips • {} Total", clips.len(), total_size_str))
                    )
            )
            .child(
                div()
                    .id("clips-scroll-area")
                    .size_full()
                    .overflow_y_scroll()
                    .px_8()
                    .pb_8()
                    .child(
                        VStack::new()
                            .gap_10()
                            .children(grouped_clips.into_iter().map(|(game_title, game_clips)| {
                                VStack::new()
                                    .gap_4()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.tokens.foreground)
                                            .child(game_title)
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_6()
                                            .children(game_clips.into_iter().map(|clip| {
                                                self.render_clip_item(clip, window, cx)
                                            }))
                                    )
                            }))
                    )
            )
            .into_any_element()
    }

    fn render_clip_item(&self, clip: Clip, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let path = clip.path.clone();
        let clips_dir = path.parent().unwrap().to_path_buf();
        let timestamp = clip.timestamp;
        
        let group_id = SharedString::from(format!("clip-{}", timestamp));
        let filename = clip.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown.mp4".to_string());
        
        div()
            .w(px(320.0))
            .flex_col()
            .gap_3()
            .group(group_id.clone())
            .child(
                div()
                    .w(px(320.0))
                    .h(px(180.0))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_lg()
                    .relative()
                    .overflow_hidden()
                    .child({
                        if let Some(thumb) = &clip.thumbnail_path {
                            img(thumb.clone())
                                .w_full()
                                .h_full()
                                .object_fit(ObjectFit::Cover)
                                .into_any_element()
                        } else {
                            // Placeholder icon (where the thumbnail would be)
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new("video").size(px(32.0)).color(theme.tokens.muted_foreground.opacity(0.3)))
                                .into_any_element()
                        }
                    })
                    .child(
                        // Hover Overlay
                        div()
                            .absolute()
                            .inset_0()
                            .bg(gpui::rgba(0x000000B2)) // Dark semi-transparent background
                            .opacity(0.0)
                            .group_hover(group_id, |s| s.opacity(1.0))
                            // Play Button in Center
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .w(px(56.0))
                                            .h(px(56.0))
                                            .rounded_full()
                                            .bg(theme.tokens.primary)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new("play").size(px(28.0)).color(theme.tokens.primary_foreground))
                                    )
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.active_view = ActiveView::Dashboard;
                                        this.selected_source = Some(format!("Clip: {}", filename));
                                        this.load_video(&path.to_string_lossy(), window, cx);
                                        cx.notify();
                                    }))
                            )
                            // Top Right Actions
                            .child(
                                div()
                                    .absolute()
                                    .top_2()
                                    .right_2()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        Button::new(SharedString::from(format!("folder-{}", timestamp)), "")
                                            .icon(IconSource::Named("folder".to_string()))
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Sm)
                                            .on_click(move |_, _, _| {
                                                let _ = std::process::Command::new("explorer").arg(&clips_dir).spawn();
                                            })
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!("delete-{}", timestamp)), "")
                                            .icon(IconSource::Named("trash".to_string()))
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Sm)
                                            .on_click({
                                                let path = clip.path.clone();
                                                cx.listener(move |_, _, _, cx| {
                                                    let _ = std::fs::remove_file(&path);
                                                    cx.notify();
                                                })
                                            })
                                    )
                            )
                    )
            )
            .child(
                VStack::new()
                    .px_1()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            // Truncate long filenames nicely if possible, or just let them overflow hidden
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(clip.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                    )
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(clip.date.clone()))
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(clip.size.clone()))
                    )
            )
    }
}
