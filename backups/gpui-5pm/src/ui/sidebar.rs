use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, LumaWorkspace};

impl LumaWorkspace {
    pub fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_view;
        let theme = use_theme();

        VStack::new()
            .w(px(240.0))
            .h_full()
            .bg(theme.tokens.card)
            .border_r_1()
            .border_color(theme.tokens.border)
            .p_4()
            .gap_2()
            .child(
                div()
                    .mb_6()
                    .pl_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.foreground)
                            .child("Luma")
                    )
            )
            .child(
                Button::new("nav-dash", "Dashboard")
                    .variant(if active == ActiveView::Dashboard { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Dashboard, cx)))
            )
            .child(
                Button::new("nav-clips", "Clips Library")
                    .variant(if active == ActiveView::Clips { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Clips, cx)))
            )
            .child(
                Button::new("nav-settings", "Settings")
                    .variant(if active == ActiveView::Settings { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Settings, cx)))
            )
    }
}
