use super::{section_header, settings_row};
use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;

impl LumaWorkspace {
    pub(crate) fn render_settings_hotkeys(
        &self,
        theme: &Theme,
        view_handle: &WeakEntity<Self>,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let vh = view_handle.clone();

        let hotkey_slots = vec![
            (
                0,
                "Toggle Recording",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.toggle_recording_vk,
                    config.hotkeys.toggle_recording_mod,
                ),
            ),
            (
                1,
                "Save Instant Replay",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.save_clip_vk,
                    config.hotkeys.save_clip_mod,
                ),
            ),
            (
                2,
                "Toggle Mic Mute",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.toggle_mic_vk,
                    config.hotkeys.toggle_mic_mod,
                ),
            ),
            (
                3,
                "Push-to-Talk",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.push_to_talk_vk,
                    config.hotkeys.push_to_talk_mod,
                ),
            ),
            (
                4,
                "Mark Flag",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.marker_flag_vk,
                    config.hotkeys.marker_flag_mod,
                ),
            ),
            (
                5,
                "Mark Kill",
                crate::hotkeys::vk_to_string(
                    config.hotkeys.marker_kill_vk,
                    config.hotkeys.marker_kill_mod,
                ),
            ),
        ];

        VStack::new().gap_4().max_w(px(800.0)).child(
            Card::new().content(
                VStack::new()
                    .p_6()
                    .gap_1()
                    .child(section_header("Global Hotkeys"))
                    .children(hotkey_slots.into_iter().map(|(slot, label, current)| {
                        let vh = vh.clone();
                        let is_listening = self.hotkey_listening == Some(slot);
                        let current_str = current.clone();

                        settings_row(
                            theme,
                            label,
                            Option::<String>::None,
                            Button::new(
                                SharedString::from(format!("hk-{}", slot)),
                                if is_listening {
                                    "Listening...".to_string()
                                } else {
                                    current_str
                                },
                            )
                            .variant(if is_listening {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .on_click(move |_, _, cx| {
                                let _ = vh.update(cx, |this, _cx| {
                                    this.hotkey_listening = Some(slot);
                                });
                            }),
                        )
                    })),
            ),
        )
    }
}
