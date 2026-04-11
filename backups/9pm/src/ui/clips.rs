use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_clips(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
        VStack::new()
            .flex_1()
            .p_8()
            .gap_8()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Clips Library")
            )
            .child(
                scrollable_vertical(
                    div()
                        .flex_col()
                        .gap_4()
                        .child(self.render_clip_item("Ace in Valorant", "2026-02-24 14:20", "45 MB"))
                        .child(self.render_clip_item("League of Legends Pentakill", "2026-02-23 22:15", "120 MB"))
                        .child(self.render_clip_item("Elden Ring Boss Fight", "2026-02-22 10:05", "310 MB"))
                )
            )
    }

    fn render_clip_item(&self, title: impl Into<SharedString>, date: impl Into<SharedString>, size: impl Into<SharedString>) -> impl IntoElement {
        let theme = use_theme();
        let title = title.into();
        let date = date.into();
        let size = size.into();
        
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
                            .child(
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
                                            .id("play-icon-wrap")
                                            .child(Icon::new("play").size(px(20.0)).color(theme.tokens.muted_foreground))
                                    )
                            )
                            .child(
                                VStack::new()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(date))
                            )
                    )
                    .child(
                        HStack::new()
                            .gap_4()
                            .items_center()
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(size))
                            .child(
                                Button::new("open-folder", "")
                                    .icon(IconSource::Named("folder".to_string()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                            )
                            .child(
                                Button::new("delete-clip", "")
                                    .icon(IconSource::Named("trash".to_string()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .text_color(theme.tokens.destructive)
                            )
                    )
            )
    }
}
