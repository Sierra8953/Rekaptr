use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::charts::pie_chart::{PieChart, PieChartSegment, PieChartSize, PieChartLabelPosition};
use adabraka_ui::components::tooltip::{Tooltip, TooltipPlacement};
use crate::ui::LumaWorkspace;
use crate::config::VideoSettings;
use gstreamer::prelude::*;
use gstreamer;
use gstreamer_app;
use std::sync::Arc;

const TAB_GENERAL: usize = 0;
const TAB_VIDEO: usize = 1;
const TAB_AUDIO: usize = 2;
const TAB_HOTKEYS: usize = 3;
const TAB_STORAGE: usize = 4;
const TAB_ABOUT: usize = 5;

/// Format a Win32 VK code + modifier bitmask into a human-readable string.
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

impl LumaWorkspace {
    pub fn render_settings_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
        let clips_gb = self.storage_clips_mb as f64 / 1024.0;
        let sessions_gb = self.storage_sessions_mb as f64 / 1024.0;
        let total_gb = clips_gb + sessions_gb;
        
        let view_handle = cx.entity().downgrade();
        let is_calculating = self.is_calculating_storage;
        let max_buf_gb = self.form_max_buffer_size_gb;
        let current_tab = self.settings_tab_index;

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
                            .child({
                                let tabs: Vec<(usize, &str, &str)> = vec![
                                    (TAB_GENERAL, "settings", "General"),
                                    (TAB_VIDEO, "video", "Video"),
                                    (TAB_AUDIO, "mic", "Audio"),
                                    (TAB_HOTKEYS, "keyboard", "Hotkeys"),
                                    (TAB_STORAGE, "folder", "Storage"),
                                    (TAB_ABOUT, "info", "About"),
                                ];
                                HStack::new()
                                    .gap_4()
                                    .border_b_1()
                                    .border_color(theme.tokens.border)
                                    .children(tabs.into_iter().map(|(idx, icon, label)| {
                                        let is_active = current_tab == idx;
                                        let view_handle = view_handle.clone();
                                        div()
                                            .id(SharedString::from(format!("tab-{}", label)))
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
                                                    this.settings_tab_index = idx;
                                                    this.hotkey_listening = None;
                                                    if idx != TAB_AUDIO {
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
                                            .child(Icon::new(icon).size(px(16.0)))
                                            .child(label.to_string())
                                    }))
                            })
                            .child(
                                div()
                                    .id("settings-scroll-area")
                                    .flex_1()
                                    .h_0()
                                    .pt_4()
                                    .pb_8()
                                    .overflow_y_scroll()
                                    .child(match current_tab {
                                        TAB_GENERAL => self.render_settings_general(&theme, &view_handle, cx).into_any_element(),
                                        TAB_VIDEO => self.render_settings_video(&theme, &view_handle, cx).into_any_element(),
                                        TAB_AUDIO => self.render_settings_audio(&theme, &view_handle, cx).into_any_element(),
                                        TAB_HOTKEYS => self.render_settings_hotkeys(&theme, &view_handle, cx).into_any_element(),
                                        TAB_STORAGE => self.render_settings_storage(&theme, &view_handle, cx).into_any_element(),
                                        TAB_ABOUT => self.render_settings_about(&theme).into_any_element(),
                                        _ => div().into_any_element(),
                                    })
                            )
                    )
            )
    }

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
                        if name != "Clips" && name != "Cache" && !name.starts_with(".") {
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

    // ── General Tab ─────────────────────────────────────────────────────

    fn render_settings_general(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let vh = view_handle.clone();

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Application"))
                        .child(settings_row(theme, "Start with Windows", Option::<String>::None,
                            settings_toggle("toggle-startup", crate::utils::is_startup_with_windows(), vh.clone(), |_this, cx| {
                                let new_state = !crate::utils::is_startup_with_windows();
                                crate::utils::set_startup_with_windows(new_state);
                                let mut config = crate::config::AppConfig::load();
                                config.startup_with_windows = new_state;
                                config.save();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Minimize to Tray", Option::<String>::None,
                            settings_toggle("toggle-tray", config.minimize_to_tray, vh.clone(), |_this, cx| {
                                let mut config = crate::config::AppConfig::load();
                                config.minimize_to_tray = !config.minimize_to_tray;
                                config.save();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Check for Updates", Option::<String>::None,
                            settings_toggle("toggle-updates", config.check_for_updates, vh.clone(), |_this, cx| {
                                let mut config = crate::config::AppConfig::load();
                                config.check_for_updates = !config.check_for_updates;
                                config.save();
                                cx.notify();
                            })
                        ))
                )
            )
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Storage & Buffer"))
                        .child(settings_row(theme, "Base Storage Path", Some(config.storage_path.clone()),
                            Button::new("change-storage", "Change")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                            let mut config = crate::config::AppConfig::load();
                                            config.storage_path = path.to_string_lossy().to_string();
                                            config.save();
                                            let _ = vh.update(cx, |_, cx| cx.notify());
                                        }
                                    }
                                })
                        ))
                        .child(settings_row(theme, "Buffer Size Limit", Some(format!("{} GB", self.form_max_buffer_size_gb)),
                            HStack::new()
                                .gap_2()
                                .child(
                                    Button::new("buf-dec", "-")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .on_click({
                                            let vh = vh.clone();
                                            move |_, _, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.form_max_buffer_size_gb = (this.form_max_buffer_size_gb - 5).max(10);
                                                    let mut config = crate::config::AppConfig::load();
                                                    config.max_buffer_size_gb = this.form_max_buffer_size_gb;
                                                    config.save();
                                                    cx.notify();
                                                });
                                            }
                                        })
                                )
                                .child(
                                    Button::new("buf-inc", "+")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .on_click({
                                            let vh = vh.clone();
                                            move |_, _, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.form_max_buffer_size_gb = (this.form_max_buffer_size_gb + 5).min(500);
                                                    let mut config = crate::config::AppConfig::load();
                                                    config.max_buffer_size_gb = this.form_max_buffer_size_gb;
                                                    config.save();
                                                    cx.notify();
                                                });
                                            }
                                        })
                                )
                        ))
                )
            )
    }

    // ── Video Tab ──────────────────────────────────────────────────────

    fn render_settings_video(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();

        let encoders: Vec<(&str, &str)> = vec![
            ("h264_nvenc", "H.264"),
            ("hevc_nvenc", "HEVC"),
            ("av1_nvenc", "AV1"),
            ("x264", "x264"),
        ];

        let resolutions = vec!["Original", "3840x2160", "2560x1440", "1920x1080", "1280x720"];
        let fps_options: Vec<i32> = vec![30, 60, 120, 144, 165, 240];
        let presets = vec!["p1", "p2", "p3", "p4", "p5", "p6", "p7"];

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Primary Encoder"))
                        .child(settings_row(theme, "Encoder", Option::<String>::None,
                            div().flex().flex_wrap().gap_2()
                                .children(encoders.into_iter().map(|(id, label)| {
                                    let vh = vh.clone();
                                    let id_str = id.to_string();
                                    let is_active = self.settings_form_encoder == id;
                                    Button::new(SharedString::from(format!("enc-{}", id)), label)
                                        .size(ButtonSize::Sm)
                                        .variant(if is_active { ButtonVariant::Default } else { ButtonVariant::Outline })
                                        .on_click(move |_, _, cx| {
                                            let _ = vh.update(cx, |this, cx| {
                                                this.settings_form_encoder = id_str.clone();
                                                let mut config = crate::config::AppConfig::load();
                                                config.global_video.encoder = id_str.clone();
                                                config.save();
                                                cx.notify();
                                            });
                                        })
                                }))
                        ))
                        .child(settings_row(theme, "Resolution", Option::<String>::None,
                            div().flex().flex_wrap().gap_2()
                                .children(resolutions.into_iter().map(|res| {
                                    let vh = vh.clone();
                                    let res_str = res.to_string();
                                    let is_active = self.settings_form_resolution == res;
                                    Button::new(SharedString::from(format!("res-{}", res)), res)
                                        .size(ButtonSize::Sm)
                                        .variant(if is_active { ButtonVariant::Default } else { ButtonVariant::Outline })
                                        .on_click(move |_, _, cx| {
                                            let _ = vh.update(cx, |this, cx| {
                                                this.settings_form_resolution = res_str.clone();
                                                let mut config = crate::config::AppConfig::load();
                                                config.global_video.resolution = res_str.clone();
                                                config.save();
                                                cx.notify();
                                            });
                                        })
                                }))
                        ))
                        .child(settings_row(theme, "Framerate", Option::<String>::None,
                            div().flex().flex_wrap().gap_2()
                                .children(fps_options.into_iter().map(|fps| {
                                    let vh = vh.clone();
                                    let is_active = self.settings_form_fps == fps;
                                    Button::new(SharedString::from(format!("fps-{}", fps)), format!("{}", fps))
                                        .size(ButtonSize::Sm)
                                        .variant(if is_active { ButtonVariant::Default } else { ButtonVariant::Outline })
                                        .on_click(move |_, _, cx| {
                                            let _ = vh.update(cx, |this, cx| {
                                                this.settings_form_fps = fps;
                                                let mut config = crate::config::AppConfig::load();
                                                config.global_video.fps = fps;
                                                config.save();
                                                cx.notify();
                                            });
                                        })
                                }))
                        ))
                )
            )
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Rate Control"))
                        .child(settings_row(theme, "Bitrate (kbps)", Some(format!("{} kbps", self.settings_form_bitrate)),
                            stepper("bit", self.settings_form_bitrate, 1000, 100000, 1000, vh.clone(), |this, val, cx| {
                                this.settings_form_bitrate = val;
                                let mut config = crate::config::AppConfig::load();
                                config.global_video.bitrate_kbps = val;
                                config.save();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "CQ Level", Some(format!("{}", self.settings_form_cq)),
                            stepper("cq", self.settings_form_cq, 0, 51, 1, vh.clone(), |this, val, cx| {
                                this.settings_form_cq = val;
                                let mut config = crate::config::AppConfig::load();
                                config.global_video.cq_level = val;
                                config.save();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Replay Retention", Some(format!("{} minutes", self.settings_form_retention)),
                            stepper("ret", self.settings_form_retention, 1, 120, 1, vh.clone(), |this, val, cx| {
                                this.settings_form_retention = val;
                                let mut config = crate::config::AppConfig::load();
                                config.global_video.retention_minutes = val;
                                config.save();
                                cx.notify();
                            })
                        ))
                )
            )
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(
                            HStack::new()
                                .justify_between()
                                .items_center()
                                .child(section_header("Advanced"))
                                .child(
                                    Button::new("toggle-adv-video", if self.settings_show_advanced_video { "Hide" } else { "Show" })
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .on_click({
                                            let vh = vh.clone();
                                            move |_, _, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.settings_show_advanced_video = !this.settings_show_advanced_video;
                                                    cx.notify();
                                                });
                                            }
                                        })
                                )
                        )
                        .when(self.settings_show_advanced_video, |this| {
                            this
                                .child(settings_row(theme, "Preset", Option::<String>::None,
                                    div().flex().flex_wrap().gap_2()
                                        .children(presets.into_iter().map(|p| {
                                            let vh = vh.clone();
                                            let ps = p.to_string();
                                            let is_active = self.settings_form_preset == p;
                                            Button::new(SharedString::from(format!("preset-{}", p)), p)
                                                .size(ButtonSize::Sm)
                                                .variant(if is_active { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                .on_click(move |_, _, cx| {
                                                    let _ = vh.update(cx, |this, cx| {
                                                        this.settings_form_preset = ps.clone();
                                                        let mut config = crate::config::AppConfig::load();
                                                        config.global_video.preset = ps.clone();
                                                        config.save();
                                                        cx.notify();
                                                    });
                                                })
                                        }))
                                ))
                                .child(settings_row(theme, "GOP Size", Some(format!("{}", self.settings_form_gop)),
                                    stepper("gop", self.settings_form_gop, 0, 600, 10, vh.clone(), |this, val, cx| {
                                        this.settings_form_gop = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.gop_size = val;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "B-Frames", Some(format!("{}", self.settings_form_bframes)),
                                    stepper("bf", self.settings_form_bframes, 0, 4, 1, vh.clone(), |this, val, cx| {
                                        this.settings_form_bframes = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.bframes = val;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "Zero Latency", Option::<String>::None,
                                    settings_toggle("toggle-zl", self.settings_form_zero_latency, vh.clone(), |this, cx| {
                                        this.settings_form_zero_latency = !this.settings_form_zero_latency;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.zero_latency = this.settings_form_zero_latency;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "Lookahead", Option::<String>::None,
                                    settings_toggle("toggle-la", self.settings_form_lookahead, vh.clone(), |this, cx| {
                                        this.settings_form_lookahead = !this.settings_form_lookahead;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.lookahead = this.settings_form_lookahead;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                                .when(self.settings_form_lookahead, |this| {
                                    this.child(settings_row(theme, "Lookahead Frames", Some(format!("{}", self.settings_form_lookahead_frames)),
                                        stepper("laf", self.settings_form_lookahead_frames, 0, 32, 1, vh.clone(), |this, val, cx| {
                                            this.settings_form_lookahead_frames = val;
                                            let mut config = crate::config::AppConfig::load();
                                            config.global_video.lookahead_frames = val;
                                            config.save();
                                            cx.notify();
                                        })
                                    ))
                                })
                                .child(settings_row(theme, "Spatial AQ", Option::<String>::None,
                                    settings_toggle("toggle-saq", self.settings_form_spatial_aq, vh.clone(), |this, cx| {
                                        this.settings_form_spatial_aq = !this.settings_form_spatial_aq;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.spatial_aq = this.settings_form_spatial_aq;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "Temporal AQ", Option::<String>::None,
                                    settings_toggle("toggle-taq", self.settings_form_temporal_aq, vh.clone(), |this, cx| {
                                        this.settings_form_temporal_aq = !this.settings_form_temporal_aq;
                                        let mut config = crate::config::AppConfig::load();
                                        config.global_video.temporal_aq = this.settings_form_temporal_aq;
                                        config.save();
                                        cx.notify();
                                    })
                                ))
                        })
                )
            )
    }

    // ── Audio Tab ──────────────────────────────────────────────────────

    fn render_settings_audio(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();
        let mut devices_raw = crate::engine::enumerate_audio_devices(true);
        if devices_raw.is_empty() {
            devices_raw.push(("default".to_string(), "Default".to_string()));
        }
        let devices = devices_raw;

        let is_monitoring = self.mic_monitor_pipeline.is_some();

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            // Microphone Source card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Microphone Source"))
                        .child(settings_row(theme, "Input Device", Option::<String>::None,
                            {
                                // Resolve the stored device ID to a friendly name for display
                                let display_name = devices.iter()
                                    .find(|(id, _)| *id == self.settings_form_mic_device)
                                    .map(|(_, label)| label.clone())
                                    .unwrap_or_else(|| self.settings_form_mic_device.clone());
                                adabraka_ui::components::dropdown::Dropdown::new(self.dd_mic.clone(),
                                    Button::new("trigger-mic", display_name).size(ButtonSize::Sm).variant(ButtonVariant::Outline))
                                    .items(devices.into_iter().map(|(id, label)| {
                                        let vh = vh.clone();
                                        let dev_id = id.clone();
                                        DropdownItem::new(id, label)
                                            .on_click(move |_, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.settings_form_mic_device = dev_id.clone();
                                                    let mut config = crate::config::AppConfig::load();
                                                    config.mic_settings.device_name = dev_id.clone();
                                                    config.save();
                                                    cx.notify();
                                                });
                                            })
                                    }).collect::<Vec<_>>())
                            }
                        ))
                        .child(settings_row(theme, "Force Mono", Option::<String>::None,
                            settings_toggle("toggle-mono", self.settings_form_mic_force_mono, vh.clone(), |this, cx| {
                                this.settings_form_mic_force_mono = !this.settings_form_mic_force_mono;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.force_mono = this.settings_form_mic_force_mono;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Gain (dB)", Some(format!("{:.1} dB", self.settings_form_mic_gain)),
                            stepper_f32("gain", self.settings_form_mic_gain, -20.0, 20.0, 0.5, vh.clone(), |this, val, cx| {
                                this.settings_form_mic_gain = val;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.gain_db = val;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Monitor Mic", Some(if is_monitoring { "Listening..." } else { "Test your mic with current settings" }),
                            Button::new("toggle-monitor", if is_monitoring { "Stop" } else { "Monitor" })
                                .variant(if is_monitoring { ButtonVariant::Destructive } else { ButtonVariant::Outline })
                                .size(ButtonSize::Sm)
                                .on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        let _ = vh.update(cx, |this, cx| {
                                            if let Some(pipeline) = this.mic_monitor_pipeline.take() {
                                                let _ = pipeline.set_state(gstreamer::State::Null);
                                                // Unsubscribe from mic provider
                                                if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                                                    provider.subscribers.remove(&0xFFFF_FFFF_FFFF_FFFFu64);
                                                }
                                                cx.notify();
                                                return;
                                            }
                                            // Build a monitor pipeline that receives DSP-processed audio
                                            // from the mic provider via appsrc -> wasapi2sink.
                                            let pipeline_str = "appsrc name=monitor_src format=time is-live=true do-timestamp=true ! audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved ! queue max-size-time=200000000 ! audioconvert ! audioresample ! wasapi2sink low-latency=true provide-clock=true";
                                            log::info!("[MicMonitor] Launching pipeline: {}", pipeline_str);
                                            match gstreamer::parse::launch(pipeline_str) {
                                                Ok(element) => {
                                                    if let Ok(pipeline) = element.downcast::<gstreamer::Pipeline>() {
                                                        // Subscribe to the mic provider's DSP-processed output
                                                        if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                                                            if let Some(appsrc) = pipeline.by_name("monitor_src")
                                                                .and_then(|e| e.downcast::<gstreamer_app::AppSrc>().ok())
                                                            {
                                                                let monitor_id = 0xFFFF_FFFF_FFFF_FFFFu64; // Reserved ID for monitor
                                                                provider.subscribers.insert(monitor_id, appsrc);
                                                                let _ = pipeline.set_state(gstreamer::State::Playing);
                                                                this.mic_monitor_pipeline = Some(pipeline);
                                                                log::info!("[MicMonitor] Pipeline started (receiving from mic provider)");
                                                            }
                                                        } else {
                                                            log::error!("[MicMonitor] Mic provider not running");
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("[MicMonitor] Failed to create pipeline: {}", e);
                                                }
                                            }
                                            cx.notify();
                                        });
                                    }
                                })
                        ))
                )
            )
            // Processing & FX card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Processing & FX"))
                        .child(settings_row(theme, "Noise Suppression (RNNoise)", Some("Requires mic restart"),
                            settings_toggle("toggle-ns", self.settings_form_mic_noise_suppression, vh.clone(), |this, cx| {
                                this.settings_form_mic_noise_suppression = !this.settings_form_mic_noise_suppression;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.noise_suppression = this.settings_form_mic_noise_suppression;
                                config.save();
                                // Restart the mic provider since audiornnoise can only be
                                // added/removed by rebuilding the GStreamer pipeline.
                                this.restart_mic_provider();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Noise Gate", Option::<String>::None,
                            settings_toggle("toggle-gate", self.settings_form_mic_gate_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_gate_enabled = !this.settings_form_mic_gate_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.noise_gate_enabled = this.settings_form_mic_gate_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_gate_enabled, |this| {
                            this.child(settings_row(theme, "Gate Threshold", Some(format!("{:.0} dB", self.settings_form_mic_gate_threshold)),
                                stepper_f32("gt", self.settings_form_mic_gate_threshold, -80.0, 0.0, 1.0, vh.clone(), |this, val, cx| {
                                    this.settings_form_mic_gate_threshold = val;
                                    let mut config = crate::config::AppConfig::load();
                                    config.mic_settings.noise_gate_threshold = val;
                                    config.save();
                                    this.notify_mic_dsp_changed();
                                    cx.notify();
                                })
                            ))
                        })
                )
            )
            // Compressor card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Compressor"))
                        .child(settings_row(theme, "Enable Compressor", Option::<String>::None,
                            settings_toggle("toggle-comp", self.settings_form_mic_compressor_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_compressor_enabled = !this.settings_form_mic_compressor_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.compressor_enabled = this.settings_form_mic_compressor_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_compressor_enabled, |this| {
                            this
                                .child(settings_row(theme, "Threshold", Some(format!("{:.0} dB", self.settings_form_mic_compressor_threshold)),
                                    stepper_f32("ct", self.settings_form_mic_compressor_threshold, -60.0, 0.0, 1.0, vh.clone(), |this, val, cx| {
                                        this.settings_form_mic_compressor_threshold = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.mic_settings.compressor_threshold = val;
                                        config.save();
                                        this.notify_mic_dsp_changed();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "Ratio", Some(format!("{:.1}:1", self.settings_form_mic_compressor_ratio)),
                                    stepper_f32("cr", self.settings_form_mic_compressor_ratio, 1.0, 20.0, 0.5, vh.clone(), |this, val, cx| {
                                        this.settings_form_mic_compressor_ratio = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.mic_settings.compressor_ratio = val;
                                        config.save();
                                        this.notify_mic_dsp_changed();
                                        cx.notify();
                                    })
                                ))
                        })
                )
            )
            // Limiter card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Limiter"))
                        .child(settings_row(theme, "Enable Limiter", Option::<String>::None,
                            settings_toggle("toggle-lim", self.settings_form_mic_limiter_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_limiter_enabled = !this.settings_form_mic_limiter_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.limiter_enabled = this.settings_form_mic_limiter_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_limiter_enabled, |this| {
                            this.child(settings_row(theme, "Threshold", Some(format!("{:.0} dB", self.settings_form_mic_limiter_threshold)),
                                stepper_f32("lt", self.settings_form_mic_limiter_threshold, -30.0, 0.0, 0.5, vh.clone(), |this, val, cx| {
                                    this.settings_form_mic_limiter_threshold = val;
                                    let mut config = crate::config::AppConfig::load();
                                    config.mic_settings.limiter_threshold = val;
                                    config.save();
                                    this.notify_mic_dsp_changed();
                                    cx.notify();
                                })
                            ))
                        })
                )
            )
    }

    // ── Hotkeys Tab ────────────────────────────────────────────────────

    fn render_settings_hotkeys(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let vh = view_handle.clone();

        let hotkey_slots = vec![
            (0, "Toggle Recording", crate::hotkeys::vk_to_string(config.hotkeys.toggle_recording_vk, config.hotkeys.toggle_recording_mod)),
            (1, "Save Instant Replay", crate::hotkeys::vk_to_string(config.hotkeys.save_clip_vk, config.hotkeys.save_clip_mod)),
            (2, "Toggle Mic Mute", crate::hotkeys::vk_to_string(config.hotkeys.toggle_mic_vk, config.hotkeys.toggle_mic_mod)),
            (3, "Push-to-Talk", crate::hotkeys::vk_to_string(config.hotkeys.push_to_talk_vk, config.hotkeys.push_to_talk_mod)),
            (4, "Mark Flag", crate::hotkeys::vk_to_string(config.hotkeys.marker_flag_vk, config.hotkeys.marker_flag_mod)),
            (5, "Mark Kill", crate::hotkeys::vk_to_string(config.hotkeys.marker_kill_vk, config.hotkeys.marker_kill_mod)),
        ];

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Global Hotkeys"))
                        .children(hotkey_slots.into_iter().map(|(slot, label, current)| {
                            let vh = vh.clone();
                            let is_listening = self.hotkey_listening == Some(slot);
                            let current_str = current.clone();

                            settings_row(theme, label, Option::<String>::None,
                                Button::new(SharedString::from(format!("hk-{}", slot)), if is_listening { "Listening...".to_string() } else { current_str })
                                    .variant(if is_listening { ButtonVariant::Default } else { ButtonVariant::Outline })
                                    .size(ButtonSize::Sm)
                                    .on_click(move |_, _, cx| {
                                        let _ = vh.update(cx, |this, _cx| {
                                            this.hotkey_listening = Some(slot);
                                        });
                                    })
                            )
                        }))
                )
            )
    }

    // ── Storage Tab ────────────────────────────────────────────────────

    fn render_settings_storage(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();

        let clips_gb = self.storage_clips_mb as f64 / 1024.0;
        let sessions_gb = self.storage_sessions_mb as f64 / 1024.0;
        let total_gb = clips_gb + sessions_gb;

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_6()
                        .child(section_header("Usage Statistics"))
                        .child(
                            HStack::new()
                                .gap_8()
                                .child(
                                    VStack::new()
                                        .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Total Usage"))
                                        .child(div().text_3xl().font_weight(FontWeight::BOLD).child(format!("{:.1} GB", total_gb)))
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(
                                            PieChart::new(vec![
                                                PieChartSegment::new("Clips", clips_gb).color(theme.tokens.primary),
                                                PieChartSegment::new("Sessions", sessions_gb).color(theme.tokens.muted),
                                            ])
                                            .size(PieChartSize::Custom(100))
                                            .label_position(PieChartLabelPosition::None)
                                        )
                                )
                        )
                )
            )
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Cleanup Rules"))
                        .child(settings_row(theme, "Auto-Delete Old Clips", Option::<String>::None,
                            settings_toggle("toggle-autodel", self.settings_form_auto_delete_enabled, vh.clone(), |this, cx| {
                                this.settings_form_auto_delete_enabled = !this.settings_form_auto_delete_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.auto_delete_clips_days = if this.settings_form_auto_delete_enabled { Some(this.settings_form_auto_delete_days) } else { None };
                                config.save();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_auto_delete_enabled, |this| {
                            this.child(settings_row(theme, "Retention (Days)", Some(format!("{} days", self.settings_form_auto_delete_days)),
                                HStack::new()
                                    .gap_2()
                                    .child(Button::new("days-dec", "-").size(ButtonSize::Sm).variant(ButtonVariant::Outline).on_click({
                                        let vh = vh.clone();
                                        move |_, _, cx| {
                                            let _ = vh.update(cx, |this, cx| {
                                                this.settings_form_auto_delete_days = (this.settings_form_auto_delete_days - 1).max(1);
                                                let mut config = crate::config::AppConfig::load();
                                                config.auto_delete_clips_days = Some(this.settings_form_auto_delete_days);
                                                config.save();
                                                cx.notify();
                                            });
                                        }
                                    }))
                                    .child(Button::new("days-inc", "+").size(ButtonSize::Sm).variant(ButtonVariant::Outline).on_click({
                                        let vh = vh.clone();
                                        move |_, _, cx| {
                                            let _ = vh.update(cx, |this, cx| {
                                                this.settings_form_auto_delete_days = (this.settings_form_auto_delete_days + 1).min(365);
                                                let mut config = crate::config::AppConfig::load();
                                                config.auto_delete_clips_days = Some(this.settings_form_auto_delete_days);
                                                config.save();
                                                cx.notify();
                                            });
                                        }
                                    }))
                            ))
                        })
                )
            )
    }

    // ── About Tab ──────────────────────────────────────────────────────

    fn render_settings_about(&self, theme: &Theme) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_12()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .size(px(80.0))
                                .rounded_2xl()
                                .bg(theme.tokens.primary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new("play").size(px(40.0)).text_color(gpui::white()))
                        )
                        .child(
                            VStack::new()
                                .items_center()
                                .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Luma Replay"))
                                .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Version 0.1.0 (Early Access)"))
                        )
                        .child(
                            div()
                                .max_w(px(400.0))
                                .text_center()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground)
                                .child("A high-performance gaming DVR and instant replay engine built with Rust and GPUI.")
                        )
                        .child(
                            HStack::new()
                                .gap_4()
                                .mt_4()
                                .child(Button::new("about-web", "Website").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-gh", "GitHub").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-discord", "Discord").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                        )
                )
            )
            .child(
                div()
                    .text_center()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground.opacity(0.5))
                    .child("© 2024 Luma Research & Development. All rights reserved.")
            )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn section_header(title: &str) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(use_theme().tokens.primary)
        .mb_2()
        .child(title.to_uppercase())
}

fn settings_row(theme: &Theme, label: impl Into<SharedString>, description: Option<impl Into<SharedString>>, control: impl IntoElement) -> impl IntoElement {
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

fn settings_toggle<V: 'static>(
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

fn stepper<V: 'static>(
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

fn stepper_f32<V: 'static>(
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
