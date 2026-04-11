use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::navigation::tabs::TabPanel;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_settings_view(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        div()
            .id("settings-scroll-area")
            .flex_1()
            .overflow_y_scroll()
            .p_8()
            .child(
                VStack::new()
                    .gap_8()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.foreground)
                            .child("Global Settings")
                    )
                    .child(
                        div()
                            .child(
                                Card::new().content(
                                    VStack::new()
                                        .p_6()
                                        .gap_6()
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child("General"))
                                        .child(
                                            HStack::new()
                                                .justify_between()
                                                .child(div().child("Storage Path"))
                                                .child(div().text_color(theme.tokens.muted_foreground).child("E:\\LumaRecordings"))
                                        )
                                        .child(
                                            HStack::new()
                                                .justify_between()
                                                .child(div().child("Startup"))
                                                .child(div().child("Start with Windows"))
                                        )
                                )
                            )
                    )
            )
    }

    pub fn render_advanced_settings_dialog(&self, source: &str, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let source_name = source.to_string();
        let muted_color = theme.tokens.muted_foreground;

        div()
            .id("advanced-settings-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("advanced-settings-container")
                    .w(px(600.0))
                    .h(px(550.0))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, _| { /* Stop propagation */ })
                    .child(
                        // Header
                        HStack::new()
                            .justify_between()
                            .p_6()
                            .border_b_1()
                            .border_color(theme.tokens.border)
                            .child(
                                VStack::new()
                                    .child(div().text_xl().font_weight(FontWeight::BOLD).child("Source Settings"))
                                    .child(div().text_sm().text_color(muted_color).child(source_name.clone()))
                            )
                            .child(
                                Button::new("close-settings", "")
                                    .icon(IconSource::Named("x".to_string()))
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.advanced_settings_source = None;
                                        cx.notify();
                                    }))
                            )
                    )
                    .child(
                        // Content with Tabs
                        div()
                            .flex_1()
                            .p_6()
                            .child(
                                Tabs::new()
                                    .tabs(vec![
                                        TabItem::new(0, "Video"),
                                        TabItem::new(1, "Audio"),
                                        TabItem::new(2, "Advanced"),
                                    ])
                                    .panels(vec![
                                        TabPanel::new({
                                            move || VStack::new()
                                                .gap_6()
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Encoder"))
                                                        .child(div().p_2().bg(theme.tokens.background).border_1().border_color(theme.tokens.border).rounded_md().child("NVIDIA NVENC AV1 (Default)"))
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Rate Control"))
                                                        .child(
                                                            HStack::new()
                                                                .gap_2()
                                                                .child(div().px_3().py_1().bg(theme.tokens.primary).text_color(theme.tokens.primary_foreground).rounded_md().child("CQP"))
                                                                .child(div().px_3().py_1().bg(theme.tokens.muted).rounded_md().child("VBR"))
                                                                .child(div().px_3().py_1().bg(theme.tokens.muted).rounded_md().child("CBR"))
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Target Quality (CQ)"))
                                                        .child(div().h_1().w_full().bg(theme.tokens.secondary).rounded_full().child(div().h_full().w(relative(0.4)).bg(theme.tokens.primary).rounded_full()))
                                                )
                                        }),
                                        TabPanel::new(move || VStack::new()
                                            .gap_4()
                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Track Isolation"))
                                            .child(
                                                VStack::new()
                                                    .gap_2()
                                                    .child(HStack::new().justify_between().child(div().child("Track 1: System Audio")).child(div().child("ON")))
                                                    .child(HStack::new().justify_between().child(div().child("Track 2: Microphone")).child(div().child("ON")))
                                                    .child(HStack::new().justify_between().child(div().child("Track 3: Discord")).child(div().child("OFF")))
                                            )),
                                        TabPanel::new(|| div().child("Advanced pipeline properties..."))
                                    ])
                            )
                    )
                    .child(
                        // Footer
                        HStack::new()
                            .justify_end()
                            .p_6()
                            .border_t_1()
                            .border_color(theme.tokens.border)
                            .gap_4()
                            .child(
                                Button::new("cancel-settings", "Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.advanced_settings_source = None;
                                        cx.notify();
                                    }))
                            )
                            .child(
                                Button::new("save-settings", "Save Changes")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.advanced_settings_source = None;
                                        this.show_toast("Settings Saved", Some("Source overrides have been updated."), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                                        cx.notify();
                                    }))
                            )
                    )
            )
    }
}
