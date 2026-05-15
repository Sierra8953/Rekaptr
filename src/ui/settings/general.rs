use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{settings_card, settings_row, settings_toggle};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_general(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let vh = view_handle.clone();

        VStack::new()
            .gap_6()
            .child(settings_card(theme, "General behavior", Some("How Rekaptr runs in the background."),
                VStack::new()
                    .child(settings_row(theme, "Minimize to tray",
                        Some("Keep Rekaptr running in the system tray when the window is closed."),
                        settings_toggle("toggle-tray", config.minimize_to_tray, vh, |_this, cx| {
                            let mut config = crate::config::AppConfig::load();
                            config.minimize_to_tray = !config.minimize_to_tray;
                            config.save();
                            cx.notify();
                        })
                    ))
            ))
    }
}
