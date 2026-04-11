use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::{ActiveView, LumaWorkspace};

impl LumaWorkspace {
    pub fn render_sessions(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let sessions = crate::utils::fetch_all_sessions();

        div()
            .id("sessions-view")
            .size_full()
            .overflow_y_scroll()
            .child(
                VStack::new()
                    .p_8()
                    .gap_6()
                    .w_full()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child("Previous Sessions")
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("{} sessions found", sessions.len()))
                            )
                    )
                    .child(
                        div()
                            .id("sessions-grid")
                            .flex()
                            .flex_wrap()
                            .gap_4()
                            .children(sessions.into_iter().map(|session| {
                                self.render_session_card(session, window, cx)
                            }))
                    )
            )
    }

    fn render_session_card(&self, session: crate::state::SessionInfo, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let title = session.game_title.clone();
        
        // Try to find artwork
        let artwork = self.app_state.artwork_cache.get(&title).map(|v| v.value().clone()).flatten();
        let image_path = artwork.map(std::path::PathBuf::from);
        let image_exists = image_path.is_some();

        Card::new()
            .w(px(280.0))
            .content(
                VStack::new()
                    .child(
                        div()
                            .relative()
                            .h(px(160.0))
                            .bg(theme.tokens.muted)
                            .rounded_t_lg()
                            .overflow_hidden()
                            .when_some(image_path, |this, path| {
                                this.child(
                                    img(path)
                                        .size_full()
                                        .object_fit(ObjectFit::Cover)
                                )
                            })
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .bg(if image_exists { gpui::rgba(0x000000_66) } else { gpui::rgba(0x00000000).into() })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .id(("play-session", session.timestamp))
                                            .child(
                                                Button::new(("btn-play-session", session.timestamp), "")
                                                    .icon(IconSource::Named("play".to_string()))
                                                    .variant(ButtonVariant::Default)
                                                    .size(ButtonSize::Icon)
                                            )
                                            .on_mouse_down(MouseButton::Left, cx.listener({
                                                let session_path = session.path.clone();
                                                move |this, _, window, cx| {
                                                    cx.stop_propagation();
                                                    this.set_active_view(ActiveView::Dashboard, cx);
                                                    this.load_session_path(session_path.clone(), window, cx);
                                                }
                                            }))
                                    )
                            )
                    )
                    .child(
                        VStack::new()
                            .p_4()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child(title.replace('_', " ").to_uppercase())
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(session.date)
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("Path: .../{}", session.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))
                            )
                    )
            )
    }
}
