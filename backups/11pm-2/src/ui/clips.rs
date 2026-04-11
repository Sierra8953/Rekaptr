use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;
use crate::state::Clip;

impl LumaWorkspace {
    pub fn render_clips(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let clips = crate::utils::fetch_all_clips();
        
        VStack::new()
            .flex_1()
            .p_8()
            .gap_8()
            .child(
                HStack::new()
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
                        div().text_sm().text_color(theme.tokens.muted_foreground).child(format!("{} Clips", clips.len()))
                    )
            )
            .child(
                scrollable_vertical(
                    div()
                        .flex_col()
                        .gap_4()
                        .map(|mut this| {
                            for clip in clips {
                                this = this.child(self.render_clip_item(clip, window, cx));
                            }
                            this
                        })
                )
            )
    }

    fn render_clip_item(&self, clip: Clip, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let path = clip.path.clone();
        let clips_dir = path.parent().unwrap().to_path_buf();
        let timestamp = clip.timestamp;
        
        Card::new()
            .content(
                HStack::new()
                    .p_4()
                    .justify_between()
                    .items_center()
                    .child(
                        HStack::new()
                            .gap_4()
                            .items_center()
                            .child({
                                let path = path.clone();
                                // Thumbnail Placeholder
                                div()
                                    .w(px(120.0))
                                    .h(px(68.0))
                                    .bg(theme.tokens.background)
                                    .border_1()
                                    .border_color(theme.tokens.border)
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("play-clip-{}", timestamp)))
                                            .child(Icon::new("play").size(px(20.0)).color(theme.tokens.muted_foreground))
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.active_view = crate::ui::ActiveView::Dashboard;
                                        let filename = path.file_name().unwrap().to_string_lossy().to_string();
                                        this.selected_source = Some(format!("Clip: {}", filename));
                                        this.load_video(&path.to_string_lossy(), window, cx);
                                        cx.notify();
                                    }))
                            })
                            .child(
                                VStack::new()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(clip.title.clone()))
                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(clip.date.clone()))
                            )
                    )
                    .child(
                        HStack::new()
                            .gap_4()
                            .items_center()
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(clip.size.clone()))
                            .child(
                                Button::new(SharedString::from(format!("folder-{}", timestamp)), "")
                                    .icon(IconSource::Named("folder".to_string()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .on_click(move |_, _, _| {
                                        let _ = std::process::Command::new("explorer").arg(&clips_dir).spawn();
                                    })
                            )
                            .child(
                                Button::new(SharedString::from(format!("delete-{}", timestamp)), "")
                                    .icon(IconSource::Named("trash".to_string()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .text_color(theme.tokens.destructive)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        let _ = std::fs::remove_file(&path);
                                        cx.notify();
                                    }))
                            )
                    )
            )
    }
}
