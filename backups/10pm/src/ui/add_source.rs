use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;
use crate::state::GameSession;

impl LumaWorkspace {
    pub fn render_add_source_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.lock();
        let windows = &state.available_windows;
        let theme = use_theme();
        
        let mut window_list = VStack::new()
            .gap_1()
            .max_h(px(350.0))
            .overflow_y_scroll();

        for win in windows {
            let hwnd = win.hwnd;
            let title = win.title.clone();
            let is_selected = self.form_hwnd == Some(hwnd);
            
            window_list = window_list.child(
                div()
                    .id(("win", hwnd as usize))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_selected { theme.tokens.accent } else { gpui::transparent_black() })
                    .hover(|style| style.bg(theme.tokens.accent))
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(div().text_sm().text_color(theme.tokens.foreground).child(title.clone()))
                            .when(is_selected, |this| this.child(
                                div()
                                    .id(("check", hwnd as usize))
                                    .child(Icon::new("check").size(px(14.0)).color(theme.tokens.primary))
                            ))
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.form_hwnd = Some(hwnd);
                        this.form_title = title.clone();
                        cx.notify();
                    }))
            );
        }

        div()
            .id("add-source-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("add-source-container")
                    .w(px(550.0))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        VStack::new()
                            .p_6()
                            .gap_4()
                            .child(
                                VStack::new()
                                    .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child("Add Source"))
                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Select a window or application to begin capturing."))
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .bg(theme.tokens.background)
                                    .border_1()
                                    .border_color(theme.tokens.border)
                                    .rounded_lg()
                                    .overflow_hidden()
                                    .child(window_list)
                            )
                            .child(
                                HStack::new()
                                    .justify_end()
                                    .gap_4()
                                    .mt_4()
                                    .child(
                                        Button::new("modal-cancel", "Cancel")
                                            .variant(ButtonVariant::Ghost)
                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                this.show_add_source_modal = false;
                                                cx.notify();
                                            }))
                                    )
                                    .child(
                                        Button::new("modal-add", "Add Source")
                                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                if this.form_hwnd.is_some() {
                                                    let mut state = this.app_state.lock();
                                                    let session = GameSession {
                                                        id: state.manual_sessions.len() as i32 + 100,
                                                        title: this.form_title.clone(),
                                                        auto_record: false,
                                                        retention: 0,
                                                        bitrate: 15000,
                                                        cq: 20,
                                                    };
                                                    state.manual_sessions.insert(session.id, session);
                                                    this.selected_source = Some(this.form_title.clone());
                                                    this.show_toast("Source Added", Some(&format!("{} is now available in your gallery.", this.form_title)), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                                                }
                                                this.show_add_source_modal = false;
                                                cx.notify();
                                            }))
                                    )
                            )
                    )
            )
            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                this.show_add_source_modal = false;
                cx.notify();
            }))
    }
}
