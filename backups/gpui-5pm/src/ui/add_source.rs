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
            .gap_2()
            .max_h(px(300.0))
            .overflow_y_scroll();

        for win in windows {
            let hwnd = win.hwnd;
            let title = win.title.clone();
            let is_selected = self.form_hwnd == Some(hwnd);
            
            window_list = window_list.child(
                div()
                    .id(("win", hwnd as usize))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_selected { theme.tokens.accent } else { theme.tokens.background })
                    .hover(|style| style.bg(theme.tokens.accent))
                    .text_color(theme.tokens.foreground)
                    .child(title.clone())
                    .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                        this.form_hwnd = Some(hwnd);
                        this.form_title = title.clone();
                        cx.notify();
                    }))
            );
        }

        div()
            .id("modal-bg")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000_aa))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Card::new()
                    .w(px(600.0))
                    .content(
                        VStack::new()
                            .p_6()
                            .gap_4()
                            .child(
                                div().text_xl().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child("Add Source")
                            )
                            .child(
                                div().text_sm().text_color(theme.tokens.muted_foreground).child("Select a window to record.")
                            )
                            .child(window_list)
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
                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
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
                                                }
                                                this.show_add_source_modal = false;
                                                cx.notify();
                                            }))
                                    )
                            )
                    )
            )
            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                this.show_add_source_modal = false;
                cx.notify();
            }))
    }
}
