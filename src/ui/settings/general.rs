use super::{section_header, settings_row, settings_toggle};
use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;

impl LumaWorkspace {
    pub(crate) fn render_settings_general(
        &self,
        theme: &Theme,
        view_handle: &WeakEntity<Self>,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                        .child(settings_row(
                            theme,
                            "Start with Windows",
                            Option::<String>::None,
                            settings_toggle(
                                "toggle-startup",
                                crate::utils::is_startup_with_windows(),
                                vh.clone(),
                                |_this, cx| {
                                    let new_state = !crate::utils::is_startup_with_windows();
                                    crate::utils::set_startup_with_windows(new_state);
                                    let mut config = crate::config::AppConfig::load();
                                    config.startup_with_windows = new_state;
                                    config.save();
                                    cx.notify();
                                },
                            ),
                        ))
                        .child(settings_row(
                            theme,
                            "Minimize to Tray",
                            Option::<String>::None,
                            settings_toggle(
                                "toggle-tray",
                                config.minimize_to_tray,
                                vh.clone(),
                                |_this, cx| {
                                    let mut config = crate::config::AppConfig::load();
                                    config.minimize_to_tray = !config.minimize_to_tray;
                                    config.save();
                                    cx.notify();
                                },
                            ),
                        ))
                        .child(settings_row(
                            theme,
                            "Check for Updates",
                            Option::<String>::None,
                            settings_toggle(
                                "toggle-updates",
                                config.check_for_updates,
                                vh.clone(),
                                |_this, cx| {
                                    let mut config = crate::config::AppConfig::load();
                                    config.check_for_updates = !config.check_for_updates;
                                    config.save();
                                    cx.notify();
                                },
                            ),
                        )),
                ),
            )
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Storage & Buffer"))
                        .child(settings_row(
                            theme,
                            "Base Storage Path",
                            Some(config.storage_path.clone()),
                            Button::new("change-storage", "Change")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                            let mut config = crate::config::AppConfig::load();
                                            config.storage_path =
                                                path.to_string_lossy().to_string();
                                            config.save();
                                            let _ = vh.update(cx, |_, cx| cx.notify());
                                        }
                                    }
                                }),
                        ))
                        .child(settings_row(
                            theme,
                            "Buffer Size Limit",
                            Some(format!("{} GB", self.form_max_buffer_size_gb)),
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
                                                    this.form_max_buffer_size_gb =
                                                        (this.form_max_buffer_size_gb - 5).max(10);
                                                    let mut config =
                                                        crate::config::AppConfig::load();
                                                    config.max_buffer_size_gb =
                                                        this.form_max_buffer_size_gb;
                                                    config.save();
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("buf-inc", "+")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .on_click({
                                            let vh = vh.clone();
                                            move |_, _, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.form_max_buffer_size_gb =
                                                        (this.form_max_buffer_size_gb + 5).min(500);
                                                    let mut config =
                                                        crate::config::AppConfig::load();
                                                    config.max_buffer_size_gb =
                                                        this.form_max_buffer_size_gb;
                                                    config.save();
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        )),
                ),
            )
    }
}
