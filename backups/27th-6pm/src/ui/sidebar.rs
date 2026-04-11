use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, LumaWorkspace};

impl LumaWorkspace {
    pub fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_view;
        let theme = use_theme();

        VStack::new()
            .w(px(72.0))
            .h_full()
            .bg(theme.tokens.card)
            .border_r_1()
            .border_color(theme.tokens.border)
            .p_3()
            .gap_4()
            .child(
                self.render_nav_item("nav-dash", "layout-dashboard", ActiveView::Dashboard, active, cx)
            )
            .child(
                self.render_nav_item("nav-clips", "video", ActiveView::Clips, active, cx)
            )
            .child(
                self.render_nav_item("nav-settings", "settings", ActiveView::Settings, active, cx)
            )
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
                            .w_3()
                            .h_3()
                            .rounded_full()
                            .bg(gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0)) // Green
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
            .h(px(48.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .bg(if is_active { theme.tokens.muted } else { gpui::rgba(0x00000000).into() })
            .text_color(if is_active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                this.set_active_view(view, cx);
            }))
            .child(Icon::new(icon_name).size(px(24.0)))
    }
}
