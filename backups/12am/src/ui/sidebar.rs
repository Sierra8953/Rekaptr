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
                    .mb_10()
                    .pl_2()
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .w_8()
                                    .h_8()
                                    .bg(theme.tokens.primary)
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(div().text_color(theme.tokens.primary_foreground).font_weight(FontWeight::BOLD).child("L"))
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child("Luma")
                            )
                    )
            )
            .child(
                Button::new("nav-dash", "Dashboard")
                    .variant(if active == ActiveView::Dashboard { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .icon(IconSource::Named("layout-dashboard".to_string()))
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Dashboard, cx)))
            )
            .child(
                Button::new("nav-clips", "Clips Library")
                    .variant(if active == ActiveView::Clips { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .icon(IconSource::Named("video".to_string()))
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Clips, cx)))
            )
            .child(
                Button::new("nav-settings", "Settings")
                    .variant(if active == ActiveView::Settings { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .icon(IconSource::Named("settings".to_string()))
                    .w_full()
                    .justify_start()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| this.set_active_view(ActiveView::Settings, cx)))
            )
            .child(Spacer::new())
            .child(
                div()
                    .px_2()
                    .py_4()
                    .border_t_1()
                    .border_color(theme.tokens.border)
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(div().w_2().h_2().rounded_full().bg(gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0))) // Green
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("System Ready"))
                    )
            )
    }
}
