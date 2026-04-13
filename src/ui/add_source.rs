use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;
use crate::state::GameSession;
use crate::config::GameSettings;

impl LumaWorkspace {
    pub fn render_add_source_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let windows = self.app_state.available_windows.lock().clone();
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
                                    .child(Icon::new("check.svg").size(px(14.0)).color(theme.tokens.primary))
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
                                    .child(
                                        Button::new("tab-advanced", "Advanced")
                                            .variant(if active_tab == 2 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.form_active_tab = 2;
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
                                                            .child(if self.is_refreshing_windows {
                                                                div()
                                                                    .h(px(200.0))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Scanning for windows..."))
                                                                    .into_any_element()
                                                            } else {
                                                                window_list.into_any_element()
                                                            })
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
                                                                div().flex().flex_wrap().gap_2()
                                                                    .child(
                                                                        Button::new("enc-av1", "AV1")
                                                                            .variant(if self.form_encoder == "nvav1enc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvav1enc".to_string(); cx.notify(); }))
                                                                    )
                                                                    .child(
                                                                        Button::new("enc-h265", "HEVC")
                                                                            .variant(if self.form_encoder == "nvh265enc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh265enc".to_string(); this.form_cq = this.form_cq.min(51); cx.notify(); }))
                                                                    )
                                                                    .child(
                                                                        Button::new("enc-h264", "H.264")
                                                                            .variant(if self.form_encoder == "nvh264enc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                            .on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh264enc".to_string(); this.form_cq = this.form_cq.min(51); cx.notify(); }))
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
                                                HStack::new()
                                                    .gap_4()
                                                    .child(
                                                        VStack::new()
                                                            .flex_1()
                                                            .gap_1()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Resolution"))
                                                            .child(
                                                                div().flex().flex_wrap().gap_2()
                                                                    .child(Button::new("res-4k", "4K").variant(if self.form_resolution == "3840x2160" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_resolution = "3840x2160".to_string(); cx.notify(); })))
                                                                    .child(Button::new("res-1440p", "1440p").variant(if self.form_resolution == "2560x1440" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_resolution = "2560x1440".to_string(); cx.notify(); })))
                                                                    .child(Button::new("res-1080p", "1080p").variant(if self.form_resolution == "1920x1080" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_resolution = "1920x1080".to_string(); cx.notify(); })))
                                                            )
                                                    )
                                                    .child(
                                                        VStack::new()
                                                            .flex_1()
                                                            .gap_1()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("FPS"))
                                                            .child(
                                                                div().flex().flex_wrap().gap_2()
                                                                    .child(Button::new("fps-30", "30").variant(if self.form_fps == 30 { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_fps = 30; cx.notify(); })))
                                                                    .child(Button::new("fps-60", "60").variant(if self.form_fps == 60 { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_fps = 60; cx.notify(); })))
                                                                    .child(Button::new("fps-120", "120").variant(if self.form_fps == 120 { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_fps = 120; cx.notify(); })))
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
                                        1 => {
                                            if let Some(track_idx) = self.form_editing_track_index {
                                                let track = &self.form_audio_tracks[track_idx];
                                                let windows = self.app_state.available_windows.lock().clone();
                                                
                                                VStack::new()
                                                    .gap_4()
                                                    .child(
                                                        HStack::new().justify_between().items_center()
                                                            .child(div().font_weight(FontWeight::BOLD).child(format!("Select Apps for {}", track.name)))
                                                            .child(Button::new("back-to-tracks", "Back").variant(ButtonVariant::Ghost).on_click(cx.listener(|this, _, _, cx| { this.form_editing_track_index = None; cx.notify(); })))
                                                    )
                                                    .child(
                                                        div().id("app-routing-list").flex_1().max_h(px(350.0)).overflow_y_scroll().child(
                                                            VStack::new().gap_1().children(
                                                                windows.iter().map(|win| {
                                                                    let proc_name = win.process_name.clone();
                                                                    let is_selected = self.form_audio_tracks[track_idx].app_targets.contains(&proc_name);
                                                                    
                                                                    HStack::new().justify_between().p_2().rounded_md().bg(if is_selected { theme.tokens.accent } else { gpui::transparent_black() })
                                                                        .child(VStack::new().child(div().text_sm().child(win.title.clone())).child(div().text_xs().text_color(theme.tokens.muted_foreground).child(proc_name.clone())))
                                                                        .child(Button::new(SharedString::from(format!("sel-app-{}-{}", track_idx, proc_name)), if is_selected { "REMOVE" } else { "ADD" }).variant(if is_selected { ButtonVariant::Destructive } else { ButtonVariant::Outline }).size(ButtonSize::Sm).on_click(cx.listener(move |this, _, _, cx| {
                                                                            if is_selected {
                                                                                this.form_audio_tracks[track_idx].app_targets.retain(|t| t != &proc_name);
                                                                            } else {
                                                                                this.form_audio_tracks[track_idx].app_targets.push(proc_name.clone());
                                                                            }
                                                                            cx.notify();
                                                                        })))
                                                                })
                                                            )
                                                        )
                                                    ).into_any_element()
                                            } else {
                                                VStack::new()
                                                    .gap_2()
                                                    .children(
                                                        self.form_audio_tracks.iter().enumerate().map(|(i, track)| {
                                                            let _is_app = track.source_type == "app";
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
                                                                        .child(VStack::new()
                                                                            .child(div().child(track.name.clone()))
                                                                            .child(
                                                                                HStack::new().gap_1().mt_1()
                                                                                    .child(Button::new(("type-sys", i), "System").variant(if track.source_type == "System" { ButtonVariant::Secondary } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(move |this, _, _, cx| { this.form_audio_tracks[i].source_type = "System".to_string(); cx.notify(); })))
                                                                                    .child(Button::new(("type-mic", i), "Mic").variant(if track.source_type == "Mic" { ButtonVariant::Secondary } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(move |this, _, _, cx| { this.form_audio_tracks[i].source_type = "Mic".to_string(); cx.notify(); })))
                                                                                    .child(Button::new(("type-app", i), "App").variant(if track.source_type == "App" { ButtonVariant::Secondary } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(move |this, _, _, cx| { this.form_audio_tracks[i].source_type = "App".to_string(); cx.notify(); })))
                                                                            )
                                                                        )
                                                                )
                                                                .when(track.source_type == "App" && track.enabled, |this| this.child(
                                                                    Button::new(("cfg-apps", i), "Configure Apps")
                                                                        .variant(ButtonVariant::Outline)
                                                                        .size(ButtonSize::Sm)
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            this.form_editing_track_index = Some(i);
                                                                            cx.notify();
                                                                        }))
                                                                ))
                                                        })
                                                    )
                                                    .into_any_element()
                                            }
                                        },
                                        2 => {
                                            let form_preset = self.form_preset.clone();
                                            let form_zero_latency = self.form_zero_latency;
                                            let form_lookahead = self.form_lookahead;
                                            let form_spatial_aq = self.form_spatial_aq;
                                            let form_temporal_aq = self.form_temporal_aq;
                                            
                                            VStack::new()
                                                .gap_4()
                                                .child(
                                                    VStack::new()
                                                        .gap_1()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Encoding Preset"))
                                                        .child(
                                                            div().flex().flex_wrap().gap_2()
                                                                .child(Button::new("pre-p1", "P1").variant(if form_preset == "p1" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p1".to_string(); cx.notify(); })))
                                                                .child(Button::new("pre-p4", "P4").variant(if form_preset == "p4" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p4".to_string(); cx.notify(); })))
                                                                .child(Button::new("pre-p7", "P7").variant(if form_preset == "p7" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p7".to_string(); cx.notify(); })))
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Quality Suite"))
                                                        .child(
                                                            HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md()
                                                                .child(div().child("Zero Latency"))
                                                                .child(Button::new("opt-zl", if form_zero_latency { "ON" } else { "OFF" }).variant(if form_zero_latency { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_zero_latency = !this.form_zero_latency; cx.notify(); })))
                                                        )
                                                        .child(
                                                            HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md()
                                                                .child(div().child("Lookahead"))
                                                                .child(Button::new("opt-la", if form_lookahead { "ON" } else { "OFF" }).variant(if form_lookahead { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_lookahead = !this.form_lookahead; cx.notify(); })))
                                                        )
                                                        .child(
                                                            HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md()
                                                                .child(div().child("Spatial AQ"))
                                                                .child(Button::new("opt-saq", if form_spatial_aq { "ON" } else { "OFF" }).variant(if form_spatial_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_spatial_aq = !this.form_spatial_aq; cx.notify(); })))
                                                        )
                                                        .child(
                                                            HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md()
                                                                .child(div().child("Temporal AQ"))
                                                                .child(Button::new("opt-taq", if form_temporal_aq { "ON" } else { "OFF" }).variant(if form_temporal_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_temporal_aq = !this.form_temporal_aq; cx.notify(); })))
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_1()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("GOP Size (frames)"))
                                                        .child(
                                                            HStack::new()
                                                                .gap_4()
                                                                .items_center()
                                                                .child(Button::new("gop-dec", "-").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_gop = (this.form_gop - 10).max(1); cx.notify(); })))
                                                                .child(div().p_3().bg(theme.tokens.background).rounded_md().min_w(px(80.0)).child(div().text_center().child(self.form_gop.to_string())))
                                                                .child(Button::new("gop-inc", "+").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_gop = (this.form_gop + 10).min(1000); cx.notify(); })))
                                                        )
                                                )
                                                .into_any_element()
                                        },
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
                                                if let Some(hwnd) = this.form_hwnd {
                                                    let title = this.form_title.clone();
                                                    let windows = this.app_state.available_windows.lock();
                                                    let target_process = windows.iter()
                                                        .find(|w| w.hwnd == hwnd)
                                                        .map(|w| w.process_name.clone());

                                                    drop(windows); // Release lock before loading config

                                                    {
                                                        log::info!("[UI] Adding new game source: '{}' (process: {:?})", title, target_process);
                                                        let mut config = crate::config::AppConfig::load();
                                                        let settings = GameSettings {
                                                            title: title.clone(),
                                                            target_process: target_process.clone(),
                                                            auto_record: this.form_auto_record,
                                                            retention_minutes: this.form_retention as i32,
                                                            video_overrides: Some(crate::config::VideoSettings {
                                                                encoder: this.form_encoder.clone(),
                                                                rate_control_index: this.form_rate_control,
                                                                bitrate_kbps: this.form_bitrate,
                                                                cq_level: this.form_cq,
                                                                resolution: this.form_resolution.clone(),
                                                                fps: this.form_fps,
                                                                retention_minutes: this.form_retention as i32,
                                                                gop_size: this.form_gop,
                                                                bframes: this.form_bframes,
                                                                preset: this.form_preset.clone(),
                                                                zero_latency: this.form_zero_latency,
                                                                lookahead: this.form_lookahead,
                                                                lookahead_frames: this.form_lookahead_frames,
                                                                spatial_aq: this.form_spatial_aq,
                                                                temporal_aq: this.form_temporal_aq,
                                                                artwork_path: None,
                                                            }),
                                                            audio_routing: Some(this.form_audio_tracks.clone()),
                                                            record_focus_only: true,
                                                            artwork_path: None,
                                                        };

                                                        config.game_registry.insert(title.clone(), settings.clone());
                                                        config.save();

                                                        this.app_state.game_registry.insert(title.clone(), settings);
                                                        this.app_state.manual_sessions.insert(this.app_state.manual_sessions.len() as i32 + 100, GameSession {
                                                            id: this.app_state.manual_sessions.len() as i32 + 100,
                                                            title: title.clone(),
                                                            auto_record: this.form_auto_record,
                                                            retention: this.form_retention as i32,
                                                            bitrate: this.form_bitrate,
                                                            cq: this.form_cq,
                                                        });
                                                        log::info!("[UI] Successfully updated state for '{}'", title);
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
