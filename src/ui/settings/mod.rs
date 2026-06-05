mod general;
mod startup;
mod video;
mod audio;
mod hotkeys;
mod storage;
mod export;
mod about;

use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::components::tooltip::{Tooltip, TooltipPlacement};
use crate::ui::{RekaptrWorkspace, SettingsTab, SETTINGS_NAV};
use crate::config::VideoSettings;
use gstreamer::prelude::*;
use gstreamer;
use std::sync::Arc;

/// Convert a gpui Keystroke to a Win32 VK code and modifier bitmask.
fn keystroke_to_vk(keystroke: &Keystroke) -> Option<(u32, u32)> {
    let key_str = keystroke.key.as_str();

    let vk = match key_str {
        "f1" => 0x70, "f2" => 0x71, "f3" => 0x72, "f4" => 0x73,
        "f5" => 0x74, "f6" => 0x75, "f7" => 0x76, "f8" => 0x77,
        "f9" => 0x78, "f10" => 0x79, "f11" => 0x7A, "f12" => 0x7B,
        "f13" => 0x7C, "f14" => 0x7D, "f15" => 0x7E, "f16" => 0x7F,
        "space" => 0x20,
        "enter" => 0x0D,
        "tab" => 0x09,
        "`" => 0xC0,
        "-" => 0xBD,
        "=" => 0xBB,
        "[" => 0xDB,
        "]" => 0xDD,
        "\\" => 0xDC,
        ";" => 0xBA,
        "'" => 0xDE,
        "," => 0xBC,
        "." => 0xBE,
        "/" => 0xBF,
        "insert" => 0x2D,
        "delete" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "pause" => 0x13,
        "escape" => return None, // Escape cancels
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap().to_ascii_uppercase();
            if c.is_ascii_alphanumeric() {
                c as u32
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let mut modifiers = 0u32;
    if keystroke.modifiers.control { modifiers |= 2; } // MOD_CONTROL
    if keystroke.modifiers.alt { modifiers |= 1; }     // MOD_ALT
    if keystroke.modifiers.shift { modifiers |= 4; }   // MOD_SHIFT

    Some((vk, modifiers))
}

impl RekaptrWorkspace {
    pub fn render_settings_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        let view_handle = cx.entity().downgrade();
        let current_tab = self.settings_tab;

        let mut root = div()
            .id("settings-view")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.tokens.background);

        if self.hotkey_listening.is_some() {
            root = root
                .focusable()
                .track_focus(&self.hotkey_focus_handle)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    let Some(slot) = this.hotkey_listening else { return };
                    let keystroke = &event.keystroke;

                    // Escape cancels
                    if keystroke.key.as_str() == "escape" {
                        this.hotkey_listening = None;
                        cx.notify();
                        return;
                    }

                    // Ignore bare modifier keys
                    if matches!(keystroke.key.as_str(), "shift" | "control" | "alt" | "meta") {
                        return;
                    }

                    if let Some((vk, modifiers)) = keystroke_to_vk(keystroke) {
                        let mut config = crate::config::AppConfig::load();
                        match slot {
                            0 => { config.hotkeys.toggle_recording_vk = vk; config.hotkeys.toggle_recording_mod = modifiers; }
                            1 => { config.hotkeys.save_clip_vk = vk; config.hotkeys.save_clip_mod = modifiers; }
                            2 => { config.hotkeys.toggle_mic_vk = vk; config.hotkeys.toggle_mic_mod = modifiers; }
                            3 => { config.hotkeys.push_to_talk_vk = vk; config.hotkeys.push_to_talk_mod = modifiers; }
                            4 => { config.hotkeys.marker_flag_vk = vk; config.hotkeys.marker_flag_mod = modifiers; }
                            5 => { config.hotkeys.marker_kill_vk = vk; config.hotkeys.marker_kill_mod = modifiers; }
                            6 => { config.hotkeys.marker_death_vk = vk; config.hotkeys.marker_death_mod = modifiers; }
                            7 => { config.hotkeys.marker_highlight_vk = vk; config.hotkeys.marker_highlight_mod = modifiers; }
                            _ => {}
                        }
                        config.save();
                        crate::hotkeys::reload_hotkeys();
                        this.hotkey_listening = None;
                        cx.notify();
                    }
                }))
        }

        root.child(
            HStack::new()
                .flex_1()
                .h_0()
                .child(render_settings_nav_rail(current_tab, &view_handle, &theme))
                .child(
                    VStack::new()
                        .flex_1()
                        .h_full()
                        .child(render_settings_top_bar(current_tab, &theme))
                        .child(
                            div()
                                .id("settings-scroll-area")
                                .flex_1()
                                .h_0()
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .max_w(px(880.0))
                                        .mx_auto()
                                        .px_10()
                                        .pt_6()
                                        .pb_16()
                                        .child(match current_tab {
                                            SettingsTab::General => self.render_settings_general(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Startup => self.render_settings_startup(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Video => self.render_settings_video(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Audio => self.render_settings_audio(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Hotkeys => self.render_settings_hotkeys(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Storage => self.render_settings_storage(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::Export => self.render_settings_export(&theme, &view_handle, cx).into_any_element(),
                                            SettingsTab::About => self.render_settings_about(&theme, &view_handle, cx).into_any_element(),
                                        }),
                                ),
                        ),
                ),
        )
    }

    pub fn sync_settings_form_from_config(&mut self, config: &crate::config::AppConfig) {
        self.settings_form_encoder = config.global_video.encoder.clone();
        self.settings_form_resolution = config.global_video.resolution.clone();
        self.settings_form_fps = config.global_video.fps;
        self.settings_form_rate_control = config.global_video.rate_control_index;
        self.settings_form_bitrate = config.global_video.bitrate_kbps;
        self.settings_form_cq = config.global_video.cq_level;
        self.settings_form_retention = config.global_video.retention_minutes;
        self.settings_form_preset = config.global_video.preset.clone();
        self.settings_form_gop = config.global_video.gop_size;
        self.settings_form_bframes = config.global_video.bframes;
        self.settings_form_zero_latency = config.global_video.zero_latency;
        self.settings_form_lookahead = config.global_video.lookahead;
        self.settings_form_lookahead_frames = config.global_video.lookahead_frames;
        self.settings_form_spatial_aq = config.global_video.spatial_aq;
        self.settings_form_temporal_aq = config.global_video.temporal_aq;
        self.settings_form_mic_device = config.mic_settings.device_name.clone();
        self.settings_form_mic_force_mono = config.mic_settings.force_mono;
        self.settings_form_mic_gain = config.mic_settings.gain_db;
        self.settings_form_mic_noise_suppression = config.mic_settings.noise_suppression;
        self.settings_form_mic_gate_enabled = config.mic_settings.noise_gate_enabled;
        self.settings_form_mic_gate_threshold = config.mic_settings.noise_gate_threshold;
        self.settings_form_mic_compressor_enabled = config.mic_settings.compressor_enabled;
        self.settings_form_mic_compressor_threshold = config.mic_settings.compressor_threshold;
        self.settings_form_mic_compressor_ratio = config.mic_settings.compressor_ratio;
        self.settings_form_mic_limiter_enabled = config.mic_settings.limiter_enabled;
        self.settings_form_mic_limiter_threshold = config.mic_settings.limiter_threshold;
        self.settings_form_auto_delete_enabled = config.auto_delete_clips_days.is_some();
        self.settings_form_auto_delete_days = config.auto_delete_clips_days.unwrap_or(30);
        self.settings_form_export_format = config.default_export_format.clone();
    }

    pub fn render_advanced_settings_dialog(&self, source: &str, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let source_name = source.to_string();
        let is_monitor = source_name == "monitor";
        let active_tab = self.form_active_tab;
        let windows = self.app_state.available_windows.lock().clone();

        // ── Tab body ────────────────────────────────────────────────
        let body: AnyElement = match active_tab {
            // Video
            0 => {
                let mut quality = VStack::new().child(ss_segmented_row(
                    &theme, cx, "rc", "Rate control",
                    if self.form_rate_control == 0 { "0" } else { "1" },
                    &[("CQP", "0"), ("VBR", "1")],
                    |this, v| { this.form_rate_control = if v == "1" { 1 } else { 0 }; },
                ));
                if self.form_rate_control == 0 {
                    quality = quality.child(ss_stepper_row(
                        &theme, cx, "cq", "Quality (CQ)",
                        Some("Lower is sharper. 18–24 is the sweet spot."),
                        self.form_cq, 0, 51, 1, |this, v| this.form_cq = v));
                } else {
                    quality = quality.child(ss_stepper_row(
                        &theme, cx, "br", "Bitrate (kbps)", None,
                        self.form_bitrate, 1000, 100_000, 1000, |this, v| this.form_bitrate = v));
                }

                VStack::new()
                    .gap_4()
                    .child(ss_card(&theme, "Output", VStack::new()
                        .child(ss_segmented_row(&theme, cx, "enc", "Encoder", &self.form_encoder,
                            &[("HEVC", "nvh265enc"), ("AV1", "nvav1enc"), ("H.264", "nvh264enc")],
                            |this, v| {
                                this.form_encoder = v;
                                if this.form_encoder != "nvav1enc" { this.form_cq = this.form_cq.min(51); }
                            }))
                        .child(ss_segmented_row(&theme, cx, "res", "Resolution", &self.form_resolution,
                            &[("4K", "3840x2160"), ("1440p", "2560x1440"), ("1080p", "1920x1080"), ("720p", "1280x720")],
                            |this, v| this.form_resolution = v))
                        .child(ss_segmented_row(&theme, cx, "fps", "Frame rate", &self.form_fps.to_string(),
                            &[("30", "30"), ("60", "60"), ("120", "120"), ("144", "144")],
                            |this, v| { if let Ok(n) = v.parse::<i32>() { this.form_fps = n; } }))))
                    .child(ss_card(&theme, "Quality", quality))
                    .child(ss_card(&theme, "Replay buffer", VStack::new().child(ss_stepper_row(
                        &theme, cx, "ret", "Retention", Some("minutes kept in memory"),
                        self.form_retention, 1, 600, 1, |this, v| this.form_retention = v))))
                    .into_any_element()
            }

            // Audio
            1 => {
                if let Some(track_idx) = self.form_editing_track_index {
                    let track_name = self.form_audio_tracks[track_idx].name.clone();
                    ss_card(&theme, "App routing", VStack::new()
                        .gap_3()
                        .child(
                            HStack::new().justify_between().items_center()
                                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(format!("Select apps for {}", track_name)))
                                .child(Button::new("back-to-tracks", "Back").variant(ButtonVariant::Ghost).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_editing_track_index = None; cx.notify(); })))
                        )
                        .child({
                            let selected_apps = self.form_audio_tracks[track_idx].app_targets.clone();
                            if selected_apps.is_empty() {
                                div().text_xs().text_color(theme.tokens.muted_foreground)
                                    .child("No apps selected yet. Pick from the list below.")
                                    .into_any_element()
                            } else {
                                div().flex().flex_row().flex_wrap().gap_2().children(selected_apps.iter().map(|app| {
                                    let app_owned = app.clone();
                                    HStack::new().gap_1p5().items_center().px_2().py_1().rounded_md()
                                        .bg(theme.tokens.accent).border_1().border_color(theme.tokens.primary)
                                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child(app.clone()))
                                        .child(div().id(SharedString::from(format!("sel-chip-x-{}-{}", track_idx, app)))
                                            .flex().items_center().justify_center().cursor_pointer()
                                            .text_color(theme.tokens.muted_foreground)
                                            .hover(|s| s.text_color(theme.tokens.foreground))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                                                this.form_audio_tracks[track_idx].app_targets.retain(|t| t != &app_owned);
                                                cx.notify();
                                            }))
                                            .child(Icon::new(IconSource::Named("x".into())).size(px(12.0)).color(theme.tokens.muted_foreground.into())))
                                })).into_any_element()
                            }
                        })
                        .child(
                            div().id("app-routing-list").max_h(px(320.0)).overflow_y_scroll().child(
                                VStack::new().gap_1().children(
                                    windows.iter().map(|win| {
                                        let proc_name = win.process_name.clone();
                                        let is_selected = self.form_audio_tracks[track_idx].app_targets.contains(&proc_name);
                                        let proc_for_click = proc_name.clone();
                                        HStack::new().justify_between().items_center().px_2().py_1p5().rounded_md()
                                            .bg(if is_selected { theme.tokens.accent } else { theme.tokens.card })
                                            .child(VStack::new().gap_0p5()
                                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child(win.title.clone()))
                                                .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(proc_name.clone())))
                                            .child(Button::new(SharedString::from(format!("sel-app-{}-{}", track_idx, proc_name)), if is_selected { "Remove" } else { "Add" })
                                                .variant(if is_selected { ButtonVariant::Destructive } else { ButtonVariant::Outline })
                                                .size(ButtonSize::Sm)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if is_selected { this.form_audio_tracks[track_idx].app_targets.retain(|t| t != &proc_for_click); }
                                                    else { this.form_audio_tracks[track_idx].app_targets.push(proc_for_click.clone()); }
                                                    cx.notify();
                                                })))
                                    })
                                )
                            )
                        )).into_any_element()
                } else {
                    let mut list = VStack::new().gap_2();
                    for (i, track) in self.form_audio_tracks.iter().enumerate() {
                        let enabled = track.enabled;
                        let source_type = track.source_type.clone();

                        let mut card_body = VStack::new()
                            .gap_3()
                            .child(
                                HStack::new()
                                    .gap_3()
                                    .items_center()
                                    .child(ss_switch(&theme, cx, SharedString::from(format!("trk-en-{}", i)), enabled, move |this| {
                                        this.form_audio_tracks[i].enabled = !this.form_audio_tracks[i].enabled;
                                    }))
                                    .child(div().w(px(64.0)).text_sm().font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if enabled { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                                        .child(track.name.clone()))
                                    .child(ss_source_pill(&theme, cx, i, "sys", "System", &source_type))
                                    .child(ss_source_pill(&theme, cx, i, "mic", "Mic", &source_type))
                                    .child(ss_source_pill(&theme, cx, i, "app", "App", &source_type))
                                    .child(div().flex_1()),
                            );

                        // Device picker for System / Mic
                        if enabled && (source_type == "System" || source_type == "Mic") {
                            let devices = if source_type == "Mic" {
                                self.app_state.audio_input_devices.lock().clone()
                            } else {
                                self.app_state.audio_output_devices.lock().clone()
                            };
                            let current_device = track.device_name.clone();
                            let device_type_label = if source_type == "Mic" { "Input device" } else { "Output device" };
                            let view_handle = cx.entity().downgrade();
                            card_body = card_body.child(
                                VStack::new().gap_2()
                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.muted_foreground).child(device_type_label.to_string()))
                                    .child(
                                        HStack::new().gap_1().flex_wrap().children(devices.into_iter().map(move |(id, name)| {
                                            let is_selected = current_device == id || (current_device == "Default" && id == "Default");
                                            let id_clone = id.clone();
                                            let view_handle = view_handle.clone();
                                            Button::new(SharedString::from(format!("dev-{}-{}", i, id)), name)
                                                .variant(if is_selected { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                                                .size(ButtonSize::Sm)
                                                .on_click(move |_, _, cx| {
                                                    let id_clone = id_clone.clone();
                                                    let _ = view_handle.update(cx, move |this, cx| {
                                                        this.form_audio_tracks[i].device_name = id_clone;
                                                        cx.notify();
                                                    });
                                                })
                                        }))
                                    )
                            );
                        }

                        // App routing entry for App source
                        if enabled && source_type == "App" {
                            let app_count = track.app_targets.len();
                            card_body = card_body.child(
                                Button::new(SharedString::from(format!("cfg-apps-{}", i)),
                                    if app_count == 0 { "Configure apps".to_string() } else { format!("{} apps routed", app_count) })
                                    .icon(IconSource::Named("chevron-right".into()))
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(move |this, _, _, cx| { this.form_editing_track_index = Some(i); cx.notify(); }))
                            );
                        }

                        list = list.child(
                            div().p_3().rounded_lg().bg(theme.tokens.card).border_1().border_color(theme.tokens.border).child(card_body)
                        );
                    }
                    ss_card(&theme, "Audio tracks", list).into_any_element()
                }
            }

            // Advanced
            2 => {
                let mut col = VStack::new().gap_4();
                if !is_monitor {
                    col = col.child(ss_card(&theme, "Automation", VStack::new().child(ss_toggle_row(
                        &theme, cx, "auto-rec", "Auto-record when detected",
                        Some("Automatically start recording when this game is detected."),
                        self.form_auto_record, |this| this.form_auto_record = !this.form_auto_record))));
                }
                col = col
                    .child(ss_card(&theme, "Encoder preset", VStack::new().child(ss_segmented_row(
                        &theme, cx, "pre", "Preset", &self.form_preset,
                        &[("P1", "p1"), ("P4", "p4"), ("P5", "p5"), ("P7", "p7")],
                        |this, v| this.form_preset = v))))
                    .child(ss_card(&theme, "Quality suite", VStack::new()
                        .child(Tooltip::new("Disables B-frames and reduces latency. Essential for real-time monitoring.").placement(TooltipPlacement::Left)
                            .child(ss_toggle_row(&theme, cx, "opt-zl", "Zero latency", None,
                                self.form_zero_latency, |this| this.form_zero_latency = !this.form_zero_latency)))
                        .child(Tooltip::new("Enables frame lookahead. Improves compression efficiency at the cost of some latency.").placement(TooltipPlacement::Left)
                            .child(ss_toggle_row(&theme, cx, "opt-la", "Lookahead", None,
                                self.form_lookahead, |this| this.form_lookahead = !this.form_lookahead)))
                        .child(Tooltip::new("Spatial Adaptive Quantization. Improves quality in low-detail areas by redistributing bitrate.").placement(TooltipPlacement::Left)
                            .child(ss_toggle_row(&theme, cx, "opt-saq", "Spatial AQ", None,
                                self.form_spatial_aq, |this| this.form_spatial_aq = !this.form_spatial_aq)))
                        .child(Tooltip::new("Temporal Adaptive Quantization. Improves quality in complex moving scenes.").placement(TooltipPlacement::Left)
                            .child(ss_toggle_row(&theme, cx, "opt-taq", "Temporal AQ", None,
                                self.form_temporal_aq, |this| this.form_temporal_aq = !this.form_temporal_aq)))))
                    .child(ss_card(&theme, "Keyframes", VStack::new().child(ss_stepper_row(
                        &theme, cx, "gop", "GOP size", Some("How often a full frame is stored. Standard: 60 (1s intervals)."),
                        self.form_gop, 0, 600, 10, |this, v| this.form_gop = v))));
                col.into_any_element()
            }

            _ => div().into_any_element(),
        };

        // ── Modal shell ─────────────────────────────────────────────
        div()
            .id("advanced-settings-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.advanced_settings_source = None;
                cx.notify();
            }))
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("advanced-settings-container")
                    .w(px(680.0))
                    .max_h(relative(0.9))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        HStack::new()
                            .px_6()
                            .py_5()
                            .border_b_1()
                            .border_color(theme.tokens.border)
                            .items_center()
                            .justify_between()
                            .child(
                                HStack::new()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .size(px(44.0))
                                            .rounded_xl()
                                            .bg(theme.tokens.muted)
                                            .border_1()
                                            .border_color(theme.tokens.border)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconSource::Named(if is_monitor { "video" } else { "gamepad-2" }.into()))
                                                .size(px(22.0))
                                                .color(theme.tokens.primary.into())),
                                    )
                                    .child(
                                        VStack::new()
                                            .gap_0p5()
                                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child(source_name.clone()))
                                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Source settings")),
                                    ),
                            )
                            .child(
                                Button::new("close-settings", "")
                                    .icon(IconSource::Named("x".into()))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, _, cx| { this.advanced_settings_source = None; cx.notify(); })),
                            ),
                    )
                    // Tabs
                    .child(
                        HStack::new()
                            .px_6()
                            .pt_4()
                            .gap_1()
                            .child(ss_tab(&theme, cx, 0, active_tab, "Video", "video"))
                            .child(ss_tab(&theme, cx, 1, active_tab, "Audio", "volume-2"))
                            .child(ss_tab(&theme, cx, 2, active_tab, "Advanced", "sliders-horizontal")),
                    )
                    // Body
                    .child(
                        div()
                            .id("settings-tab-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_6()
                            .py_5()
                            .child(body),
                    )
                    // Footer
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .px_6()
                            .py_4()
                            .border_t_1()
                            .border_color(theme.tokens.border)
                            .child({
                                let source_name_del = source_name.clone();
                                if !is_monitor {
                                    Button::new("delete-session", "Delete source")
                                        .icon(IconSource::Named("trash".into()))
                                        .variant(ButtonVariant::Destructive)
                                        .size(ButtonSize::Sm)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let mut session_id_to_remove = None;
                                            for session in this.app_state.manual_sessions.iter() {
                                                if session.value().title == source_name_del {
                                                    session_id_to_remove = Some(*session.key());
                                                    break;
                                                }
                                            }
                                            if let Some(id) = session_id_to_remove {
                                                this.session_to_delete = Some(id);
                                                this.advanced_settings_source = None;
                                                cx.notify();
                                            }
                                        }))
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                }
                            })
                            .child(
                                HStack::new()
                                    .gap_3()
                                    .child(
                                        Button::new("cancel-settings", "Cancel")
                                            .variant(ButtonVariant::Ghost)
                                            .on_click(cx.listener(|this, _, _, cx| { this.advanced_settings_source = None; cx.notify(); })),
                                    )
                                    .child(
                                        Button::new("save-settings", "Save changes")
                                            .icon(IconSource::Named("check".into()))
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
                                                        settings.auto_record = this.form_auto_record;
                                                    }
                                                }
                                                config.save();
                                                this.advanced_settings_source = None;
                                                this.show_toast("Settings Saved", Some("Source overrides have been updated."), adabraka_ui::overlays::toast::ToastVariant::Success, window, cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
            )
    }
}

// ── Nav rail + top bar ──────────────────────────────────────────────

fn render_settings_nav_rail(
    current: SettingsTab,
    view_handle: &WeakEntity<RekaptrWorkspace>,
    theme: &Theme,
) -> impl IntoElement {
    let mut rail = VStack::new()
        .w(px(240.0))
        .h_full()
        .bg(theme.tokens.card)
        .border_r_1()
        .border_color(theme.tokens.border)
        .pt_6()
        .pb_4()
        .px_3()
        .gap_1()
        .child(
            div()
                .px_3()
                .pb_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.tokens.foreground)
                        .child("Settings"),
                ),
        );

    for group in SETTINGS_NAV {
        rail = rail
            .child(
                div()
                    .px_3()
                    .pt_5()
                    .pb_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.muted_foreground)
                    .child(group.title),
            )
            .children(group.items.iter().map(|&tab| {
                nav_rail_item(tab, current, view_handle.clone(), theme)
            }));
    }
    rail
}

fn nav_rail_item(
    tab: SettingsTab,
    current: SettingsTab,
    view_handle: WeakEntity<RekaptrWorkspace>,
    theme: &Theme,
) -> impl IntoElement {
    let active = tab == current;
    div()
        .id(SharedString::from(format!("nav-{}", tab.label())))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(36.0))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .bg(if active { theme.tokens.accent } else { gpui::transparent_black() })
        .hover(|s| s.bg(theme.tokens.muted))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            let _ = view_handle.update(cx, |this, cx| {
                this.settings_tab = tab;
                this.hotkey_listening = None;
                if tab != SettingsTab::Audio {
                    if let Some(pipeline) = this.mic_monitor_pipeline.take() {
                        let _ = pipeline.set_state(gstreamer::State::Null);
                        if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                            provider.subscribers.remove(&0xFFFF_FFFF_FFFF_FFFFu64);
                        }
                    }
                }
                cx.notify();
            });
        })
        .child(
            Icon::new(IconSource::Named(tab.icon().into()))
                .size(px(16.0))
                .color(if active { theme.tokens.primary.into() } else { theme.tokens.muted_foreground.into() }),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                .child(tab.label()),
        )
}

fn render_settings_top_bar(current: SettingsTab, theme: &Theme) -> impl IntoElement {
    HStack::new()
        .px_10()
        .py_5()
        .border_b_1()
        .border_color(theme.tokens.border)
        .justify_between()
        .items_center()
        .child(
            VStack::new()
                .gap_1()
                .child(
                    HStack::new()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .child(current.group()),
                        )
                        .child(
                            Icon::new(IconSource::Named("chevron-right".into()))
                                .size(px(12.0))
                                .color(theme.tokens.muted_foreground.into()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .child(current.label()),
                        ),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.tokens.foreground)
                        .child(current.label()),
                ),
        )
}

// ── Helpers ─────────────────────────────────────────────────────────

/// A titled card with optional description. Replaces the older `Card::new().content(section_header(...))` pattern.
pub(super) fn settings_card(
    theme: &Theme,
    title: &str,
    description: Option<&str>,
    body: impl IntoElement,
) -> impl IntoElement {
    let desc_owned: Option<SharedString> = description.map(|d| SharedString::from(d.to_string()));
    let mut header = VStack::new()
        .px_6()
        .pt_5()
        .pb_3()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.tokens.foreground)
                .child(title.to_string()),
        );
    if let Some(d) = desc_owned {
        header = header.child(
            div().text_xs().text_color(theme.tokens.muted_foreground).child(d),
        );
    }
    VStack::new()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(theme.tokens.border)
        .bg(theme.tokens.card)
        .child(header)
        .child(div().px_6().pb_5().child(body))
}

pub(super) fn settings_row(theme: &Theme, label: impl Into<SharedString>, description: Option<impl Into<SharedString>>, control: impl IntoElement) -> impl IntoElement {
    HStack::new()
        .justify_between()
        .items_center()
        .py_2()
        .border_b_1()
        .border_color(theme.tokens.border.opacity(0.3))
        .child(
            VStack::new()
                .child(div().font_weight(FontWeight::MEDIUM).child(label.into()))
                .when_some(description, |this, desc| {
                    this.child(div().text_xs().text_color(theme.tokens.muted_foreground).child(desc.into()))
                })
        )
        .child(control)
}

pub(super) fn settings_toggle<V: 'static>(
    id: impl Into<ElementId>,
    value: bool,
    view_handle: WeakEntity<V>,
    on_click: impl Fn(&mut V, &mut Context<V>) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_click = Arc::new(on_click);
    adabraka_ui::components::checkbox::Checkbox::new(id)
        .checked(value)
        .on_click({
            let on_click = on_click.clone();
            move |_, _, cx| {
                let on_click = on_click.clone();
                let _ = view_handle.update(cx, |this, cx| {
                    on_click(this, cx);
                });
            }
        })
}

pub(super) fn stepper<V: 'static>(
    prefix: &str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    view_handle: WeakEntity<V>,
    on_change: impl Fn(&mut V, i32, &mut Context<V>) + 'static + Send + Sync + Clone,
) -> impl IntoElement {
    let on_dec = on_change.clone();
    let on_inc = on_change;
    let vh_dec = view_handle.clone();
    let vh_inc = view_handle;
    HStack::new()
        .gap_2()
        .child(
            Button::new(SharedString::from(format!("{}-dec", prefix)), "-")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    let val = (value - step).max(min);
                    let _ = vh_dec.update(cx, |this, cx| {
                        on_dec(this, val, cx);
                    });
                }),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", prefix)), "+")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    let val = (value + step).min(max);
                    let _ = vh_inc.update(cx, |this, cx| {
                        on_inc(this, val, cx);
                    });
                }),
        )
}

pub(super) fn stepper_f32<V: 'static>(
    prefix: &str,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    view_handle: WeakEntity<V>,
    on_change: impl Fn(&mut V, f32, &mut Context<V>) + 'static + Send + Sync + Clone,
) -> impl IntoElement {
    let on_dec = on_change.clone();
    let on_inc = on_change;
    let vh_dec = view_handle.clone();
    let vh_inc = view_handle;
    HStack::new()
        .gap_2()
        .child(
            Button::new(SharedString::from(format!("{}-dec", prefix)), "-")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    let val = (value - step).max(min);
                    let _ = vh_dec.update(cx, |this, cx| {
                        on_dec(this, val, cx);
                    });
                }),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", prefix)), "+")
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Outline)
                .on_click(move |_, _, cx| {
                    let val = (value + step).min(max);
                    let _ = vh_inc.update(cx, |this, cx| {
                        on_inc(this, val, cx);
                    });
                }),
        )
}

// ── Source-settings dialog: shared controls ─────────────────────────
fn ss_tab(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    index: usize,
    active_tab: usize,
    label: &'static str,
    icon: &'static str,
) -> impl IntoElement {
    let active = index == active_tab;
    div()
        .id(SharedString::from(format!("ss-tab-{}", index)))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_t_lg()
        .cursor_pointer()
        .bg(if active { theme.tokens.muted } else { gpui::transparent_black() })
        .border_b_2()
        .border_color(if active { theme.tokens.primary } else { gpui::transparent_black() })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.form_active_tab = index;
            this.form_editing_track_index = None;
            cx.notify();
        }))
        .child(Icon::new(IconSource::Named(icon.into()))
            .size(px(15.0))
            .color(if active { theme.tokens.primary.into() } else { theme.tokens.muted_foreground.into() }))
        .child(div()
            .text_sm()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
            .child(label))
}

fn ss_card(theme: &Theme, title: &str, body: impl IntoElement) -> impl IntoElement {
    VStack::new()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(theme.tokens.border)
        .bg(theme.tokens.background)
        .child(div()
            .px_5()
            .pt_4()
            .pb_2()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.tokens.muted_foreground)
            .child(title.to_string()))
        .child(div().px_5().pb_4().child(body))
}

fn ss_row(theme: &Theme, label: &str, description: Option<&str>, control: AnyElement) -> impl IntoElement {
    let mut left = VStack::new().flex_1().gap_0p5().child(div()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.tokens.foreground)
        .child(label.to_string()));
    if let Some(d) = description {
        left = left.child(div().text_xs().text_color(theme.tokens.muted_foreground).child(d.to_string()));
    }
    HStack::new()
        .w_full()
        .py_2p5()
        .gap_4()
        .items_center()
        .justify_between()
        .child(left)
        .child(div().child(control))
}

fn ss_switch(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    id: impl Into<ElementId>,
    value: bool,
    on_toggle: impl Fn(&mut RekaptrWorkspace) + 'static + Send + Sync,
) -> impl IntoElement {
    crate::ui::toggle_switch(theme, cx, id, value, false, on_toggle)
}

fn ss_toggle_row(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    id: &'static str,
    label: &str,
    description: Option<&str>,
    value: bool,
    on_toggle: impl Fn(&mut RekaptrWorkspace) + 'static + Send + Sync,
) -> impl IntoElement {
    let sw = ss_switch(theme, cx, id, value, on_toggle);
    ss_row(theme, label, description, sw.into_any_element())
}

fn ss_segmented_row(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    id_prefix: &'static str,
    label: &str,
    current: &str,
    options: &[(&'static str, &'static str)],
    on_pick: impl Fn(&mut RekaptrWorkspace, String) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_pick = Arc::new(on_pick);
    let current_owned = current.to_string();
    let mut group = div()
        .flex()
        .flex_row()
        .rounded_md()
        .bg(theme.tokens.card)
        .border_1()
        .border_color(theme.tokens.border)
        .p(px(2.0))
        .gap(px(2.0));
    for (i, (lbl, val)) in options.iter().enumerate() {
        let active = *val == current_owned;
        let val_string = val.to_string();
        let on_pick = on_pick.clone();
        group = group.child(div()
            .id(SharedString::from(format!("{}-{}", id_prefix, i)))
            .px_3()
            .py_1()
            .rounded_sm()
            .text_xs()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .cursor_pointer()
            .bg(if active { theme.tokens.primary } else { gpui::transparent_black() })
            .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
            .hover(|s| s.text_color(theme.tokens.foreground))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                on_pick(this, val_string.clone());
                cx.notify();
            }))
            .child(lbl.to_string()));
    }
    ss_row(theme, label, None, group.into_any_element())
}

fn ss_stepper_row(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    id_prefix: &'static str,
    label: &str,
    description: Option<&str>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    on_change: impl Fn(&mut RekaptrWorkspace, i32) + 'static + Send + Sync,
) -> impl IntoElement {
    let on_change = Arc::new(on_change);
    let on_dec = on_change.clone();
    let on_inc = on_change;
    let ctl = HStack::new()
        .gap_2()
        .items_center()
        .child(Button::new(SharedString::from(format!("{}-dec", id_prefix)), "")
            .icon(IconSource::Named("minus".into()))
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Outline)
            .on_click(cx.listener(move |this, _, _, cx| { on_dec(this, (value - step).max(min)); cx.notify(); })))
        .child(div()
            .min_w(px(72.0))
            .text_center()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.tokens.foreground)
            .child(format!("{}", value)))
        .child(Button::new(SharedString::from(format!("{}-inc", id_prefix)), "")
            .icon(IconSource::Named("plus".into()))
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Outline)
            .on_click(cx.listener(move |this, _, _, cx| { on_inc(this, (value + step).min(max)); cx.notify(); })));
    ss_row(theme, label, description, ctl.into_any_element())
}

fn ss_source_pill(
    theme: &Theme,
    cx: &mut Context<RekaptrWorkspace>,
    idx: usize,
    id_suffix: &'static str,
    label: &'static str,
    current: &str,
) -> impl IntoElement {
    let active = current == label;
    div()
        .id(SharedString::from(format!("ss-pill-{}-{}", idx, id_suffix)))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .cursor_pointer()
        .bg(if active { theme.tokens.accent } else { theme.tokens.background })
        .border_1()
        .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
        .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
        .hover(|s| s.text_color(theme.tokens.foreground))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.form_audio_tracks[idx].source_type = label.to_string();
            cx.notify();
        }))
        .child(label)
}
