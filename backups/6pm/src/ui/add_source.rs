use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;
use crate::state::GameSession;
use crate::config::GameSettings;

impl LumaWorkspace {
    pub fn render_add_source_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.lock();
        let windows = state.available_windows.clone();
        let theme = use_theme();
        
        let mut window_list = div()
            .id("add-source-window-list")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(200.0))
            .overflow_y_scroll();

        for win in &windows {
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
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.form_hwnd = Some(hwnd);
                        this.form_title = title.clone();
                        cx.notify();
                    }))
            );
        }

        let active_tab = self.form_active_tab;

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
                    .w(px(650.0))
                    .max_h(relative(0.9))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        VStack::new()
                            .p_6()
                            .gap_4()
                            .flex_1()
                            .child(
                                VStack::new()
                                    .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child("Add Game Capture"))
                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Configure your recording settings for this source."))
                            )
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .child(
                                        Button::new("tab-video", "Core Video")
                                            .variant(if active_tab == 0 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.form_active_tab = 0;
                                                cx.notify();
                                            }))
                                    )
                                    .child(
                                        Button::new("tab-audio", "Audio Routing")
                                            .variant(if active_tab == 1 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.form_active_tab = 1;
                                                cx.notify();
                                            }))
                                    )
                            )
                            .child(
                                div()
                                    .id("add-source-tab-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(match active_tab {
                                        0 => VStack::new()
                                            .gap_4()
                                            .child(
                                                VStack::new()
                                                    .gap_1()
                                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Target Window"))
                                                    .child(
                                                        div()
                                                            .bg(theme.tokens.background)
                                                            .border_1()
                                                            .border_color(theme.tokens.border)
                                                            .rounded_lg()
                                                            .overflow_hidden()
                                                            .child(window_list)
                                                    )
                                            )
                                            .child(
                                                HStack::new()
                                                    .gap_4()
                                                    .child(
                                                        VStack::new()
                                                            .flex_1()
                                                            .gap_1()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Encoder"))
                                                            .child(
                                                                HStack::new()
                                                                    .gap_2()
                                                                    .child(
                                                                        Button::new("enc-av1", "AV1")
                                                                            .variant(if self.form_encoder == "nvav1enc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvav1enc".to_string(); cx.notify(); }))
                                                                    )
                                                                    .child(
                                                                        Button::new("enc-h264", "H.264")
                                                                            .variant(if self.form_encoder == "nvh264enc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh264enc".to_string(); cx.notify(); }))
                                                                    )
                                                            )
                                                    )
                                                    .child(
                                                        VStack::new()
                                                            .flex_1()
                                                            .gap_1()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Rate Control"))
                                                            .child(
                                                                HStack::new()
                                                                    .gap_2()
                                                                    .child(
                                                                        Button::new("rc-cqp", "CQP")
                                                                            .variant(if self.form_rate_control == 0 { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_rate_control = 0; cx.notify(); }))
                                                                    )
                                                                    .child(
                                                                        Button::new("rc-vbr", "VBR")
                                                                            .variant(if self.form_rate_control == 1 { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_rate_control = 1; cx.notify(); }))
                                                                    )
                                                            )
                                                    )
                                            )
                                            .child(
                                                VStack::new()
                                                    .gap_1()
                                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(if self.form_rate_control == 0 { "Quality (CQ)" } else { "Bitrate (kbps)" }))
                                                    .child(
                                                        HStack::new()
                                                            .gap_4()
                                                            .items_center()
                                                            .child(
                                                                Button::new("val-dec", "-")
                                                                    .variant(ButtonVariant::Outline)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        if this.form_rate_control == 0 {
                                                                            this.form_cq = (this.form_cq - 1).max(0);
                                                                        } else {
                                                                            this.form_bitrate = (this.form_bitrate - 1000).max(1000);
                                                                        }
                                                                        cx.notify();
                                                                    }))
                                                            )
                                                            .child(
                                                                div()
                                                                    .p_3()
                                                                    .bg(theme.tokens.background)
                                                                    .rounded_md()
                                                                    .min_w(px(100.0))
                                                                    .child(div().text_center().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.primary).child(if self.form_rate_control == 0 { self.form_cq.to_string() } else { self.form_bitrate.to_string() }))
                                                            )
                                                            .child(
                                                                Button::new("val-inc", "+")
                                                                    .variant(ButtonVariant::Outline)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        if this.form_rate_control == 0 {
                                                                            this.form_cq = (this.form_cq + 1).min(51);
                                                                        } else {
                                                                            this.form_bitrate = (this.form_bitrate + 1000).min(100000);
                                                                        }
                                                                        cx.notify();
                                                                    }))
                                                            )
                                                    )
                                            )
                                            .child(
                                                VStack::new()
                                                    .gap_1()
                                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Retention (minutes)"))
                                                    .child(
                                                        HStack::new()
                                                            .gap_4()
                                                            .items_center()
                                                            .child(
                                                                Button::new("ret-dec", "-")
                                                                    .variant(ButtonVariant::Outline)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.form_retention = (this.form_retention - 1).max(1);
                                                                        cx.notify();
                                                                    }))
                                                            )
                                                            .child(
                                                                div()
                                                                    .p_3()
                                                                    .bg(theme.tokens.background)
                                                                    .rounded_md()
                                                                    .min_w(px(100.0))
                                                                    .child(div().text_center().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.primary).child(self.form_retention.to_string()))
                                                            )
                                                            .child(
                                                                Button::new("ret-inc", "+")
                                                                    .variant(ButtonVariant::Outline)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.form_retention = (this.form_retention + 1).min(600);
                                                                        cx.notify();
                                                                    }))
                                                            )
                                                    )
                                            )
                                            .into_any_element(),
                                        1 => VStack::new()
                                            .gap_2()
                                            .children(
                                                self.form_audio_tracks.iter().enumerate().map(|(i, track)| {
                                                    HStack::new()
                                                        .justify_between()
                                                        .p_3()
                                                        .bg(theme.tokens.background)
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(theme.tokens.border)
                                                        .child(
                                                            HStack::new()
                                                                .gap_3()
                                                                .child(
                                                                    Button::new(("track-toggle", i), if track.enabled { "ON" } else { "OFF" })
                                                                        .variant(if track.enabled { ButtonVariant::Default } else { ButtonVariant::Ghost })
                                                                        .size(ButtonSize::Sm)
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            this.form_audio_tracks[i].enabled = !this.form_audio_tracks[i].enabled;
                                                                            cx.notify();
                                                                        }))
                                                                )
                                                                .child(div().child(track.name.clone()))
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.tokens.muted_foreground)
                                                                .child(track.source_type.clone())
                                                        )
                                                })
                                            )
                                            .into_any_element(),
                                        _ => div().into_any_element(),
                                    })
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
                                        Button::new("modal-add", "Add Game Source")
                                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                if let Some(_hwnd) = this.form_hwnd {
                                                    let title = this.form_title.clone();
                                                    {
                                                        let mut config = crate::config::AppConfig::load();
                                                        let settings = GameSettings {
                                                            title: title.clone(),
                                                            auto_record: true,
                                                            retention_minutes: this.form_retention as i32,
                                                            video_overrides: Some(crate::config::VideoSettings {
                                                                encoder: this.form_encoder.clone(),
                                                                rate_control_index: this.form_rate_control,
                                                                bitrate_kbps: this.form_bitrate,
                                                                cq_level: this.form_cq,
                                                                resolution: this.form_resolution.clone(),
                                                                fps: this.form_fps,
                                                                retention_minutes: this.form_retention as i32,
                                                                gop_size: 60,
                                                                bframes: 0,
                                                                preset: "p4".to_string(),
                                                                zero_latency: true,
                                                                lookahead: true,
                                                                lookahead_frames: 32,
                                                                spatial_aq: true,
                                                                temporal_aq: true,
                                                                artwork_path: None,
                                                            }),
                                                            audio_routing: Some(this.form_audio_tracks.clone()),
                                                            record_focus_only: true,
                                                            artwork_path: None,
                                                        };
                                                        config.game_registry.insert(title.clone(), settings);
                                                        config.save();
                                                        
                                                        let state = this.app_state.lock();
                                                        state.manual_sessions.insert(state.manual_sessions.len() as i32 + 100, GameSession {
                                                            id: state.manual_sessions.len() as i32 + 100,
                                                            title: title.clone(),
                                                            auto_record: true,
                                                            retention: this.form_retention as i32,
                                                            bitrate: this.form_bitrate,
                                                            cq: this.form_cq,
                                                        });
                                                    }
                                                    
                                                    this.selected_source = Some(title.clone());
                                                    this.load_video(&title, window, cx);
                                                    this.show_toast("Source Added", Some(&format!("{} is now available in your gallery.", title)), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
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
