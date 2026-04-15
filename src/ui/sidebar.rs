use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, RekaptrWorkspace};

impl RekaptrWorkspace {
    pub fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_view;
        let theme = use_theme();

        VStack::new()
            .w(px(72.0))
            .h_full()
            .bg(theme.tokens.card)
            .border_r_1()
            .border_color(theme.tokens.border)
            .pt(px(12.0))
            .px(px(8.0))
            .gap_2()
            .child(self.render_nav_item("nav-dash", "layout-dashboard", ActiveView::Dashboard, active, cx))
            .child(self.render_nav_item("nav-clips", "video", ActiveView::Clips, active, cx))
            .child(self.render_nav_item("nav-settings", "settings", ActiveView::Settings, active, cx))
            .child(Spacer::new())
            .child(
                div()
                    .py_4()
                    .border_t_1()
                    .border_color(theme.tokens.border)
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .w_2()
                            .h_2()
                            .rounded_full()
                            .bg(theme.tokens.primary)
                    )
            )
    }

    fn render_nav_item(
        &self,
        id: &'static str,
        icon_name: &'static str,
        view: ActiveView,
        active: ActiveView,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let is_active = active == view;

        div()
            .id(id)
            .w_full()
            .h(px(56.0))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                this.set_active_view(view, cx);
            }))
            .child(
                div()
                    .size(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .bg(if is_active { theme.tokens.muted } else { gpui::transparent_black() })
                    .text_color(if is_active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                    .hover(|s| s.bg(theme.tokens.muted.opacity(0.5)).text_color(theme.tokens.foreground))
                    .child(Icon::new(icon_name).size(px(24.0)))
            )
            .when(is_active, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(-8.0))
                        .top(px(16.0))
                        .w(px(3.0))
                        .h(px(24.0))
                        .rounded_r_sm()
                        .bg(theme.tokens.primary)
                )
            })
    }
}
