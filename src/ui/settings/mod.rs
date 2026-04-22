mod general;
mod video;
mod audio;
mod hotkeys;
mod storage;
mod about;

use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::components::tooltip::{Tooltip, TooltipPlacement};
use crate::ui::{RekaptrWorkspace, SettingsTab};
use crate::config::VideoSettings;
use gstreamer::prelude::*;
use gstreamer;
use std::sync::Arc;

/// Format a Win32 VK code + modifier bitmask into a human-readable string.
#[allow(dead_code)]
fn format_hotkey(vk: u32, modifiers: u32) -> String {
    let mut parts = Vec::new();
    if modifiers & 2 != 0 { parts.push("Ctrl".to_string()); }
    if modifiers & 1 != 0 { parts.push("Alt".to_string()); }
    if modifiers & 4 != 0 { parts.push("Shift".to_string()); }
    let key_name = match vk {
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x30..=0x39 => format!("{}", (vk - 0x30)),
        0x41..=0x5A => format!("{}", (vk as u8) as char),
        0x6A => "*".to_string(),
        0x6B => "+".to_string(),
        0x6D => "-".to_string(),
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x09 => "Tab".to_string(),
        0x14 => "CapsLock".to_string(),
        0xC0 => "`".to_string(),
        0xBD => "-".to_string(),
        0xBB => "=".to_string(),
        0xDB => "[".to_string(),
        0xDD => "]".to_string(),
        0xDC => "\\".to_string(),
        0xBA => ";".to_string(),
        0xDE => "'".to_string(),
        0xBC => ",".to_string(),
        0xBE => ".".to_string(),
        0xBF => "/".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x90 => "NumLock".to_string(),
        0x91 => "ScrollLock".to_string(),
        0x13 => "Pause".to_string(),
        _ => format!("Key(0x{:02X})", vk),
    };
    parts.push(key_name);
    parts.join(" + ")
}

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

        let clips_gb = self.storage_clips_mb as f64 / 1024.0;
        let sessions_gb = self.storage_sessions_mb as f64 / 1024.0;
        let _total_gb = clips_gb + sessions_gb;

        let view_handle = cx.entity().downgrade();
        let _is_calculating = self.is_calculating_storage;
        let _max_buf_gb = self.form_max_buffer_size_gb;
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
                        this.hotkey_listening = None;
                        cx.notify();
                    }
                }))
        }

        root.child(
                VStack::new()
                    .flex_1()
                    .h_0()
                    .child(
                        div()
                            .px_8()
                            .pt_8()
                            .pb_4()
                            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("Settings"))
                    )
                    .child(
                        VStack::new()
                            .flex_1()
                            .h_0()
                            .px_8()
                            .child(
                                HStack::new()
                                    .gap_4()
                                    .border_b_1()
                                    .border_color(theme.tokens.border)
                                    .children(SettingsTab::ALL.iter().map(|&tab| {
                                        let is_active = current_tab == tab;
                                        let view_handle = view_handle.clone();
                                        div()
                                            .id(SharedString::from(format!("tab-{}", tab.label())))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px_4()
                                            .py_2()
                                            .cursor(CursorStyle::PointingHand)
                                            .border_b_2()
                                            .border_color(if is_active { theme.tokens.primary } else { gpui::transparent_black() })
                                            .text_color(if is_active { theme.tokens.primary } else { theme.tokens.muted_foreground })
                                            .hover(|s| s.text_color(theme.tokens.primary))
                                            .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
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
                                            .child(Icon::new(tab.icon()).size(px(16.0)))
                                            .child(tab.label())
                                    }))
                            )
                            .child(
                                div()
                                    .id("settings-scroll-area")
                                    .flex_1()
                                    .h_0()
                                    .pt_4()
                                    .pb_8()
                                    .overflow_y_scroll()
                                    .child(match current_tab {
                                        SettingsTab::General => self.render_settings_general(&theme, &view_handle, cx).into_any_element(),
                                        SettingsTab::Video => self.render_settings_video(&theme, &view_handle, cx).into_any_element(),
                                        SettingsTab::Audio => self.render_settings_audio(&theme, &view_handle, cx).into_any_element(),
                                        SettingsTab::Hotkeys => self.render_settings_hotkeys(&theme, &view_handle, cx).into_any_element(),
                                        SettingsTab::Storage => self.render_settings_storage(&theme, &view_handle, cx).into_any_element(),
                                        SettingsTab::About => self.render_settings_about(&theme).into_any_element(),
                                    })
                            )
                    )
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

    #[allow(dead_code)]
    pub fn refresh_storage_info(&mut self, cx: &mut Context<Self>) {
        if self.is_calculating_storage {
            return;
        }
        self.is_calculating_storage = true;
        let task = cx.background_spawn(async move {
            let root = crate::utils::get_storage_root();
            let clips_dir = root.join("Clips");

            let clips_size = crate::utils::get_dir_size(&clips_dir).unwrap_or(0);
            let mut sessions_size = 0;

            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let name_lower = name.to_lowercase();
                        if name != "Clips" && name != "Cache" && !name.starts_with(".")
                           && name_lower != "target" && name_lower != "dist"
                           && !name_lower.contains("gstreamer") {
                            sessions_size += crate::utils::get_dir_size(&path).unwrap_or(0);
                        }
                    }
                }
            }
            (clips_size, sessions_size)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (clips_bytes, sessions_bytes) = task.await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.storage_clips_mb = clips_bytes / (1024 * 1024);
                    this.storage_sessions_mb = sessions_bytes / (1024 * 1024);
                    this.is_calculating_storage = false;
                    cx.notify();
                });
            }
        }).detach();
        cx.notify();
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
                                            .icon(IconSource::Named("x".into()))
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
                                                                    .child(Button::new("enc-h265", "HEVC").variant(if self.form_encoder == "nvh265enc" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh265enc".to_string(); this.form_cq = this.form_cq.min(51); cx.notify(); })))
                                                                    .child(Button::new("enc-h264", "H.264").variant(if self.form_encoder == "nvh264enc" { ButtonVariant::Default } else { ButtonVariant::Outline }).on_click(cx.listener(|this, _, _, cx| { this.form_encoder = "nvh264enc".to_string(); this.form_cq = this.form_cq.min(51); cx.notify(); })))
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
                                                                            .when(track.enabled && (track.source_type == "System" || track.source_type == "Mic"), {
                                                                                let devices = if track.source_type == "Mic" {
                                                                                    self.app_state.audio_input_devices.lock().clone()
                                                                                } else {
                                                                                    self.app_state.audio_output_devices.lock().clone()
                                                                                };
                                                                                let current_device = track.device_name.clone();
                                                                                let current_device_label = devices.iter()
                                                                                    .find(|(id, _)| *id == current_device)
                                                                                    .map(|(_, name)| name.clone())
                                                                                    .unwrap_or_else(|| current_device.clone());
                                                                                let view_handle = cx.entity().downgrade();
                                                                                let device_type_label = if track.source_type == "Mic" { "Input Device" } else { "Output Device" };
                                                                                move |row| {
                                                                                    row.child(
                                                                                        VStack::new().gap_1().mt_2()
                                                                                            .child(
                                                                                                HStack::new().gap_2().items_center()
                                                                                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(use_theme().tokens.muted_foreground).child(device_type_label.to_string()))
                                                                                                    .child(div().text_xs().text_color(use_theme().tokens.foreground).child(current_device_label))
                                                                                            )
                                                                                            .child(
                                                                                                HStack::new().gap_1().flex_wrap()
                                                                                                    .children(devices.into_iter().map(move |(id, name)| {
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
                                                                                    )
                                                                                }
                                                                            })
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
                                                .when(source_name != "monitor", |this| {
                                                    this.child(
                                                        VStack::new()
                                                            .gap_2()
                                                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Automation"))
                                                            .child(
                                                                HStack::new()
                                                                    .justify_between()
                                                                    .p_2()
                                                                    .bg(theme.tokens.background)
                                                                    .rounded_md()
                                                                    .child(
                                                                        VStack::new()
                                                                            .child(div().child("Auto-Record"))
                                                                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Automatically start recording when this game is detected."))
                                                                    )
                                                                    .child(
                                                                        Button::new("opt-auto-record", if self.form_auto_record { "ON" } else { "OFF" })
                                                                            .variant(if self.form_auto_record { ButtonVariant::Default } else { ButtonVariant::Ghost })
                                                                            .size(ButtonSize::Sm)
                                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                                this.form_auto_record = !this.form_auto_record;
                                                                                cx.notify();
                                                                            }))
                                                                    )
                                                            )
                                                    )
                                                })
                                                .child(
                                                    VStack::new()
                                                        .gap_1()
                                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Encoding Preset"))                                                        .child(
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
                                                        .child(
                                                            Tooltip::new("Disables B-frames and reduces latency. Essential for real-time monitoring.")
                                                                .placement(TooltipPlacement::Left)
                                                                .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Zero Latency")).child(Button::new("opt-zl", if self.form_zero_latency { "ON" } else { "OFF" }).variant(if self.form_zero_latency { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_zero_latency = !this.form_zero_latency; cx.notify(); }))))
                                                        )
                                                        .child(
                                                            Tooltip::new("Enables frame lookahead. Improves compression efficiency at the cost of some latency.")
                                                                .placement(TooltipPlacement::Left)
                                                                .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Lookahead")).child(Button::new("opt-la", if self.form_lookahead { "ON" } else { "OFF" }).variant(if self.form_lookahead { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_lookahead = !this.form_lookahead; cx.notify(); }))))
                                                        )
                                                        .child(
                                                            Tooltip::new("Spatial Adaptive Quantization. Improves quality in low-detail areas by redistributing bitrate.")
                                                                .placement(TooltipPlacement::Left)
                                                                .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Spatial AQ")).child(Button::new("opt-saq", if self.form_spatial_aq { "ON" } else { "OFF" }).variant(if self.form_spatial_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_spatial_aq = !this.form_spatial_aq; cx.notify(); }))))
                                                        )
                                                        .child(
                                                            Tooltip::new("Temporal Adaptive Quantization. Improves quality in complex moving scenes.")
                                                                .placement(TooltipPlacement::Left)
                                                                .child(HStack::new().justify_between().p_2().bg(theme.tokens.background).rounded_md().child(div().child("Temporal AQ")).child(Button::new("opt-taq", if self.form_temporal_aq { "ON" } else { "OFF" }).variant(if self.form_temporal_aq { ButtonVariant::Default } else { ButtonVariant::Ghost }).size(ButtonSize::Sm).on_click(cx.listener(|this, _, _, cx| { this.form_temporal_aq = !this.form_temporal_aq; cx.notify(); }))))
                                                        )
                                                )
                                                .child(
                                                    VStack::new()
                                                        .gap_1()
                                                        .child(
                                                            Tooltip::new("GOP Size (Group of Pictures). Controls how often a full frame is stored. Standard: 60 (1s intervals).")
                                                                .placement(TooltipPlacement::Left)
                                                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("GOP Size (frames)"))
                                                        )
                                                        .child(
                                                            HStack::new()
                                                                .gap_4()
                                                                .items_center()
                                                                .child(Button::new("gop-dec", "-").variant(ButtonVariant::Outline).on_click(cx.listener(|this, _, _, cx| { this.form_gop = (this.form_gop - 10).max(1); cx.notify(); })))
                                                                .child(div().p_3().bg(theme.tokens.background).rounded_md().min_w(px(100.0)).child(div().text_center().child(self.form_gop.to_string())))
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
                                                // Find the session ID by name
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
                                                        settings.auto_record = this.form_auto_record;
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

// ── Helpers ───────���──────────────────────────────────────────────────

pub(super) fn section_header(title: &str) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(use_theme().tokens.primary)
        .mb_2()
        .child(title.to_uppercase())
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
