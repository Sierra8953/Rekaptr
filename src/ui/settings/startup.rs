use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{settings_card, settings_row, settings_toggle};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_startup(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();

        VStack::new()
            .gap_6()
            .child(settings_card(theme, "Startup", None,
                VStack::new()
                    .child(settings_row(theme, "Start with Windows", Option::<String>::None,
                        settings_toggle("toggle-startup", crate::utils::is_startup_with_windows(), vh, |_this, cx| {
                            let new_state = !crate::utils::is_startup_with_windows();
                            crate::utils::set_startup_with_windows(new_state);
                            let mut config = crate::config::AppConfig::load();
                            config.startup_with_windows = new_state;
                            config.save();
                            cx.notify();
                        })
                    ))
            ))
    }
}
