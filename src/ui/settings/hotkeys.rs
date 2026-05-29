use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{settings_card, settings_row};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_hotkeys(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let vh = view_handle.clone();

        let hk = |slot: usize, label: &'static str, vk: u32, modifiers: u32| {
            let current = crate::hotkeys::vk_to_string(vk, modifiers);
            let is_listening = self.hotkey_listening == Some(slot);
            let vh = vh.clone();
            let current_str = current.clone();
            settings_row(theme, label, Option::<String>::None,
                Button::new(SharedString::from(format!("hk-{}", slot)),
                    if is_listening { "Listening...".to_string() } else { current_str })
                    .variant(if is_listening { ButtonVariant::Default } else { ButtonVariant::Outline })
                    .size(ButtonSize::Sm)
                    .on_click(move |_, _, cx| {
                        let _ = vh.update(cx, |this, _cx| {
                            this.hotkey_listening = Some(slot);
                        });
                    })
            )
        };

        let reset_vh = vh.clone();

        VStack::new()
            .gap_6()
            .child(settings_card(theme, "Capture",
                Some("Active system-wide. Click a binding and press the new combination. Esc cancels."),
                VStack::new()
                    .child(hk(0, "Toggle recording", config.hotkeys.toggle_recording_vk, config.hotkeys.toggle_recording_mod))
                    .child(hk(1, "Save instant replay", config.hotkeys.save_clip_vk, config.hotkeys.save_clip_mod))
            ))
            .child(settings_card(theme, "Microphone", None,
                VStack::new()
                    .child(hk(2, "Toggle mic mute", config.hotkeys.toggle_mic_vk, config.hotkeys.toggle_mic_mod))
                    .child(hk(3, "Push-to-talk", config.hotkeys.push_to_talk_vk, config.hotkeys.push_to_talk_mod))
            ))
            .child(settings_card(theme, "Markers",
                Some("Tag moments during recording for quick lookup later."),
                VStack::new()
                    .child(hk(4, "Mark flag", config.hotkeys.marker_flag_vk, config.hotkeys.marker_flag_mod))
                    .child(hk(5, "Mark kill", config.hotkeys.marker_kill_vk, config.hotkeys.marker_kill_mod))
                    .child(hk(6, "Mark death", config.hotkeys.marker_death_vk, config.hotkeys.marker_death_mod))
                    .child(hk(7, "Mark highlight", config.hotkeys.marker_highlight_vk, config.hotkeys.marker_highlight_mod))
            ))
            .child(
                HStack::new()
                    .justify_end()
                    .child(
                        Button::new("reset-hotkeys", "Reset to defaults")
                            .icon(IconSource::Named("rotate-ccw".into()))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .on_click(move |_, _, cx| {
                                let _ = reset_vh.update(cx, |this, cx| {
                                    let mut config = crate::config::AppConfig::load();
                                    config.hotkeys = crate::config::HotkeyConfig::defaults();
                                    config.save();
                                    crate::hotkeys::reload_hotkeys();
                                    this.hotkey_listening = None;
                                    cx.notify();
                                });
                            })
                    )
            )
    }
}
