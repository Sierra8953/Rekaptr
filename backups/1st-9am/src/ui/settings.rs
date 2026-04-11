use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;
use crate::config::{GameSettings, VideoSettings};

impl LumaWorkspace {
    pub fn render_settings_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
        let clips_gb = self.storage_clips_mb as f64 / 1024.0;
        let sessions_gb = self.storage_sessions_mb as f64 / 1024.0;
        let total_gb = clips_gb + sessions_gb;
        
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
                            .font_weight(FontWeight::SEMIBOLD)
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
                                        .child(div().font_weight(FontWeight::SEMIBOLD).text_lg().child("General"))
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
                    .child(
                        div()
                            .child(
                                Card::new().content(
                                    VStack::new()
                                        .p_6()
                                        .gap_6()
                                        .child(div().font_weight(FontWeight::SEMIBOLD).text_lg().child("Storage Management"))
                                        .child(
                                            VStack::new()
                                                .gap_4()
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(
                                                            HStack::new()
                                                                .justify_between()
                                                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Disk Usage Breakdown"))
                                                                .child(
                                                                    div().text_sm().text_color(theme.tokens.muted_foreground)
                                                                        .child(if self.is_calculating_storage { "Calculating...".to_string() } else { format!("{:.1} GB Total", total_gb) })
                                                                )
                                                        )
                                                        .child(
                                                            // Multi-segment usage bar
                                                            HStack::new()
                                                                .w_full()
                                                                .h(px(16.0))
                                                                .rounded_full()
                                                                .overflow_hidden()
                                                                .bg(theme.tokens.muted)
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(relative(if total_gb > 0.0 { (clips_gb / total_gb.max(0.1)) as f32 } else { 0.0 }))
                                                                        .bg(theme.tokens.primary)
                                                                )
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(relative(if total_gb > 0.0 { (sessions_gb / total_gb.max(0.1)) as f32 } else { 0.0 }))
                                                                        .bg(gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0)) // Green for sessions
                                                                )
                                                        )
                                                        .child(
                                                            HStack::new()
                                                                .justify_between()
                                                                .child(
                                                                    HStack::new().gap_4()
                                                                        .child(
                                                                            HStack::new().gap_2().items_center()
                                                                                .child(div().w_3().h_3().rounded_full().bg(theme.tokens.primary))
                                                                                .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(format!("Clips ({:.1} GB)", clips_gb)))
                                                                        )
                                                                        .child(
                                                                            HStack::new().gap_2().items_center()
                                                                                .child(div().w_3().h_3().rounded_full().bg(gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0)))
                                                                                .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(format!("Buffers ({:.1} GB)", sessions_gb)))
                                                                        )
                                                                )
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Max Background Buffer Size (GB)"))
                                                        .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("If game sessions exceed this limit, oldest buffers are deleted."))
                                                        .child(
                                                            HStack::new()
                                                                .gap_4()
                                                                .items_center()
                                                                .child(Button::new("buf-dec", "-").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_max_buffer_size_gb = (this.form_max_buffer_size_gb - 5).max(5); cx.notify(); })))
                                                                .child(div().p_3().bg(theme.tokens.background).rounded_md().min_w(px(100.0)).child(div().text_center().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.primary).child(self.form_max_buffer_size_gb.to_string())))
                                                                .child(Button::new("buf-inc", "+").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_max_buffer_size_gb = (this.form_max_buffer_size_gb + 5).min(500); cx.notify(); })))
                                                                .child(
                                                                    Button::new("buf-save", "Apply")
                                                                        .variant(ButtonVariant::Secondary)
                                                                        .on_click(cx.listener(|this, _, window, cx| {
                                                                            let mut config = crate::config::AppConfig::load();
                                                                            config.max_buffer_size_gb = this.form_max_buffer_size_gb;
                                                                            config.save();
                                                                            this.show_toast("Storage Updated", Some("Max buffer size limit saved."), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                                                                            cx.notify();
                                                                        }))
                                                                )
                                                        )
                                                )
                                        )
                                )
                            )
                    )
                    .child(
                        div()
                            .child(
                                Card::new().content(
                                    VStack::new()
                                        .p_6()
                                        .gap_6()
                                        .child(div().font_weight(FontWeight::SEMIBOLD).text_lg().child("Game Detection Mapping"))
                                        .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Manually link a process executable (e.g. 'hl2.exe') to a Game Title so Luma knows to start recording when it opens."))
                                        .child(
                                            HStack::new()
                                                .gap_4()
                                                .items_center()
                                                .child(
                                                    adabraka_ui::components::input::Input::new(&self.form_custom_process_exe)
                                                        .placeholder("Process Name (e.g. game.exe)")
                                                        .w(px(200.0))
                                                )
                                                .child(
                                                    adabraka_ui::components::input::Input::new(&self.form_custom_process_title)
                                                        .placeholder("Game Title (e.g. Half-Life 2)")
                                                        .w(px(200.0))
                                                )
                                                .child(
                                                    Button::new("add-mapping", "Add Mapping")
                                                        .variant(ButtonVariant::Default)
                                                        .on_click(cx.listener(|this, _, window, cx| {
                                                            let exe = this.form_custom_process_exe.read(cx).content.clone().to_string();
                                                            let title = this.form_custom_process_title.clone().read(cx).content.clone().to_string();
                                                            if !exe.is_empty() && !title.is_empty() {
                                                                let mut config = crate::config::AppConfig::load();
                                                                config.custom_process_mapping.insert(exe, title);
                                                                config.save();
                                                                this.form_custom_process_exe.update(cx, |s, cx| s.set_value("", window, cx));
                                                                this.form_custom_process_title.update(cx, |s, cx| s.set_value("", window, cx));
                                                                cx.notify();
                                                            }
                                                        }))
                                                )
                                        )
                                        .child({
                                            let config = crate::config::AppConfig::load();
                                            if config.custom_process_mapping.is_empty() {
                                                div().text_sm().text_color(theme.tokens.muted_foreground).child("No custom mappings.").into_any_element()
                                            } else {
                                                VStack::new()
                                                    .gap_2()
                                                    .children(
                                                        config.custom_process_mapping.into_iter().map(|(exe, title)| {
                                                            HStack::new()
                                                                .justify_between()
                                                                .items_center()
                                                                .p_3()
                                                                .border_1()
                                                                .border_color(theme.tokens.border)
                                                                .rounded_md()
                                                                .bg(theme.tokens.background)
                                                                .child(
                                                                    VStack::new()
                                                                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                                                                        .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(exe.clone()))
                                                                )
                                                                .child(
                                                                    Button::new(SharedString::from(format!("del-map-{}", exe)), "Remove")
                                                                        .variant(ButtonVariant::Destructive)
                                                                        .size(ButtonSize::Sm)
                                                                        .on_click({
                                                                            let exe_to_remove = exe.clone();
                                                                            cx.listener(move |_, _, _, cx| {
                                                                                let mut cfg = crate::config::AppConfig::load();
                                                                                cfg.custom_process_mapping.remove(&exe_to_remove);
                                                                                cfg.save();
                                                                                cx.notify();
                                                                            })
                                                                        })
                                                                )
                                                        })
                                                    )
                                                    .into_any_element()
                                            }
                                        })
                                )
                            )
                    )
            )
    }

    pub fn render_advanced_settings_dialog(&self, source: &str, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let source_name = source.to_string();
        let active_tab = self.form_active_tab;
        
        let windows = self.app_state.available_windows.lock().clone();

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
                                HStack::new()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        VStack::new()
                                            .child(div().text_xl().font_weight(FontWeight::BOLD).child("Source Settings"))
                                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(source_name.clone()))
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
                                HStack::new()
                                    .gap_2()
                                    .child(
                                        Button::new("tab-video", "Video")
                                            .variant(if active_tab == 0 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| { this.form_active_tab = 0; cx.notify(); }))
                                    )
                                    .child(
                                        Button::new("tab-audio", "Audio")
                                            .variant(if active_tab == 1 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| { this.form_active_tab = 1; this.form_editing_track_index = None; cx.notify(); }))
                                    )
                                    .child(
                                        Button::new("tab-advanced", "Advanced")
                                            .variant(if active_tab == 2 { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                            .on_click(cx.listener(|this, _, _, cx| { this.form_active_tab = 2; cx.notify(); }))
                                    )
                            )
                            .child(
                                div()
                                    .id("settings-tab-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(match active_tab {
                                        0 => VStack::new()
                                            .gap_4()
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
                                                                    .child(Button::new("enc-av1", "AV1").variant(if self.form_encoder == "nvav1enc" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvav1enc".to_string(); cx.notify(); })))
                                                                    .child(Button::new("enc-h265", "HEVC").variant(if self.form_encoder == "nvh265enc" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh265enc".to_string(); cx.notify(); })))
                                                                    .child(Button::new("enc-h264", "H.264").variant(if self.form_encoder == "nvh264enc" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh264enc".to_string(); cx.notify(); })))
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
                                                                    .child(Button::new("rc-cqp", "CQP").variant(if self.form_rate_control == 0 { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_rate_control = 0; cx.notify(); })))
                                                                    .child(Button::new("rc-vbr", "VBR").variant(if self.form_rate_control == 1 { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_rate_control = 1; cx.notify(); })))
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
                                                            .child(Button::new("val-dec", "-").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { if this.form_rate_control == 0 { this.form_cq = (this.form_cq - 1).max(0); } else { this.form_bitrate = (this.form_bitrate - 1000).max(1000); } cx.notify(); })))
                                                            .child(div().p_3().bg(theme.tokens.background).rounded_md().min_w(px(100.0)).child(div().text_center().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.primary).child(if self.form_rate_control == 0 { self.form_cq.to_string() } else { self.form_bitrate.to_string() })))
                                                            .child(Button::new("val-inc", "+").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { if this.form_rate_control == 0 { this.form_cq = (this.form_cq + 1).min(51); } else { this.form_bitrate = (this.form_bitrate + 1000).min(100000); } cx.notify(); })))
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
                                                            .child(Button::new("ret-dec", "-").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_retention = (this.form_retention - 1).max(1); cx.notify(); })))
                                                            .child(div().p_3().bg(theme.tokens.background).rounded_md().min_w(px(100.0)).child(div().text_center().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.primary).child(self.form_retention.to_string())))
                                                            .child(Button::new("ret-inc", "+").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_retention = (this.form_retention + 1).min(600); cx.notify(); })))
                                                    )
                                            )
                                            .into_any_element(),
                                        1 => {
                                            if let Some(track_idx) = self.form_editing_track_index {
                                                let track = &self.form_audio_tracks[track_idx];
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
                                                                            if is_selected { this.form_audio_tracks[track_idx].app_targets.retain(|t| t != &proc_name); } else { this.form_audio_tracks[track_idx].app_targets.push(proc_name.clone()); }
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
                                                                                .on_click(cx.listener(move |this, _, _, cx| { this.form_audio_tracks[i].enabled = !this.form_audio_tracks[i].enabled; cx.notify(); }))
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
                                                                        .on_click(cx.listener(move |this, _, _, cx| { this.form_editing_track_index = Some(i); cx.notify(); }))
                                                                ))
                                                        })
                                                    )
                                                    .into_any_element()
                                            }
                                        },
                                        2 => {
                                            VStack::new()
                                                .gap_4()
                                                .child(
                                                    VStack::new()
                                                        .gap_1()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Encoding Preset"))
                                                        .child(
                                                            div().flex().flex_wrap().gap_2()
                                                                .child(Button::new("pre-p1", "P1").variant(if self.form_preset == "p1" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p1".to_string(); cx.notify(); })))
                                                                .child(Button::new("pre-p4", "P4").variant(if self.form_preset == "p4" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p4".to_string(); cx.notify(); })))
                                                                .child(Button::new("pre-p7", "P7").variant(if self.form_preset == "p7" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_preset = "p7".to_string(); cx.notify(); })))
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_2()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Quality Suite"))
                                                        .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Zero Latency")).child(Button::new("opt-zl", if self.form_zero_latency { "ON" } else { "OFF" }).variant(if self.form_zero_latency { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_zero_latency = !this.form_zero_latency; cx.notify(); }))))
                                                        .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Lookahead")).child(Button::new("opt-la", if self.form_lookahead { "ON" } else { "OFF" }).variant(if self.form_lookahead { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_lookahead = !this.form_lookahead; cx.notify(); }))))
                                                        .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Spatial AQ")).child(Button::new("opt-saq", if self.form_spatial_aq { "ON" } else { "OFF" }).variant(if self.form_spatial_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_spatial_aq = !this.form_spatial_aq; cx.notify(); })))).child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Temporal AQ")).child(Button::new("opt-taq", if self.form_temporal_aq { "ON" } else { "OFF" }).variant(if self.form_temporal_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_temporal_aq = !this.form_temporal_aq; cx.notify(); }))))
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
                    )
                    .child(
                        // Footer
                        HStack::new()
                            .justify_between()
                            .p_6()
                            .border_t_1()
                            .border_color(theme.tokens.border)
                            .child({
                                let is_monitor = source_name == "monitor";
                                let source_name_del = source_name.clone();
                                div().child(
                                    if !is_monitor {
                                        Button::new("delete-session", "Delete Session")
                                            .variant(ButtonVariant::Destructive)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                // 1. Find the session ID by name
                                                let mut session_id_to_remove = None;
                                                for session in this.app_state.manual_sessions.iter() {
                                                    if session.value().title == source_name_del {
                                                        session_id_to_remove = Some(*session.key());
                                                        break;
                                                    }
                                                }
                                                
                                                // 2. Remove from state
                                                if let Some(id) = session_id_to_remove {
                                                    this.app_state.manual_sessions.remove(&id);
                                                }

                                                // 3. Remove from config
                                                let mut config = crate::config::AppConfig::load();
                                                config.game_registry.remove(&source_name_del);
                                                config.save();

                                                // 4. Cleanup UI state
                                                this.advanced_settings_source = None;
                                                if this.selected_source.as_deref() == Some(source_name_del.as_str()) {
                                                    this.selected_source = None;
                                                    this.video_source = None;
                                                }
                                                
                                                cx.notify();
                                            }))
                                            .into_any_element()
                                    } else {
                                        div().into_any_element()
                                    }
                                )
                            })
                            .child(
                                HStack::new()
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
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let mut config = crate::config::AppConfig::load();
                                                if source_name == "monitor" {
                                                    config.global_video = VideoSettings {
                                                        encoder: this.form_encoder.clone(),
                                                        rate_control_index: this.form_rate_control,
                                                        bitrate_kbps: this.form_bitrate,
                                                        cq_level: this.form_cq,
                                                        resolution: this.form_resolution.clone(),
                                                        fps: this.form_fps,
                                                        retention_minutes: this.form_retention,
                                                        gop_size: this.form_gop,
                                                        bframes: this.form_bframes,
                                                        preset: this.form_preset.clone(),
                                                        zero_latency: this.form_zero_latency,
                                                        lookahead: this.form_lookahead,
                                                        lookahead_frames: this.form_lookahead_frames,
                                                        spatial_aq: this.form_spatial_aq,
                                                        temporal_aq: this.form_temporal_aq,
                                                        artwork_path: None,
                                                    };
                                                    config.global_audio_tracks = this.form_audio_tracks.clone();  
                                                } else {
                                                    if let Some(settings) = config.game_registry.get_mut(&source_name) {
                                                        settings.video_overrides = Some(VideoSettings {
                                                            encoder: this.form_encoder.clone(),
                                                            rate_control_index: this.form_rate_control,
                                                            bitrate_kbps: this.form_bitrate,
                                                            cq_level: this.form_cq,
                                                            resolution: this.form_resolution.clone(),
                                                            fps: this.form_fps,
                                                            retention_minutes: this.form_retention,
                                                            gop_size: this.form_gop,
                                                            bframes: this.form_bframes,
                                                            preset: this.form_preset.clone(),
                                                            zero_latency: this.form_zero_latency,
                                                            lookahead: this.form_lookahead,
                                                            lookahead_frames: this.form_lookahead_frames,
                                                            spatial_aq: this.form_spatial_aq,
                                                            temporal_aq: this.form_temporal_aq,
                                                            artwork_path: None,
                                                        });
                                                        settings.audio_routing = Some(this.form_audio_tracks.clone());
                                                        settings.retention_minutes = this.form_retention;
                                                    }
                                                }
                                                config.save();
                                                this.advanced_settings_source = None;
                                                this.show_toast("Settings Saved", Some("Source overrides have been updated."), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                                                cx.notify();
                                            }))
                                    )
                            )
                    )
            )
    }
}
