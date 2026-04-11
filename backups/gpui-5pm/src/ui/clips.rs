use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_clips(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        VStack::new()
            .flex_1()
            .p_8()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Clips Library")
            )
            .child(div().mt_4().text_color(theme.tokens.muted_foreground).child("Clips Content Placeholder"))
    }
}
