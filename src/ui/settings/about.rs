use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    pub(crate) fn render_settings_about(&self, theme: &Theme) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_12()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .size(px(80.0))
                                .rounded_2xl()
                                .bg(theme.tokens.primary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new("play").size(px(40.0)).text_color(gpui::white()))
                        )
                        .child(
                            VStack::new()
                                .items_center()
                                .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Rekaptr"))
                                .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Version 0.1.0 (Early Access)"))
                        )
                        .child(
                            div()
                                .max_w(px(400.0))
                                .text_center()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground)
                                .child("A high-performance gaming DVR and instant replay engine built with Rust and GPUI.")
                        )
                        .child(
                            HStack::new()
                                .gap_4()
                                .mt_4()
                                .child(Button::new("about-web", "Website").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-gh", "GitHub").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-discord", "Discord").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                        )
                )
            )
    }
}
