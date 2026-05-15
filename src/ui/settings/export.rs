use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{settings_card, settings_row};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_export(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let config = crate::config::AppConfig::load();
        let current_fmt = config.default_export_format.clone();

        let formats: &[(&'static str, &'static str, &'static str)] = &[
            ("fmt-mp4",  "mp4",  "mp4"),
            ("fmt-mov",  "mov",  "mov"),
            ("fmt-mkv",  "mkv",  "mkv"),
            ("fmt-webm", "webm", "webm"),
        ];

        let row = HStack::new().gap_2().children(formats.iter().map(|(id, label, value)| {
            let active = *value == current_fmt;
            let value_owned = value.to_string();
            let vh = view_handle.clone();
            Button::new(SharedString::from(id.to_string()), *label)
                .variant(if active { ButtonVariant::Default } else { ButtonVariant::Outline })
                .size(ButtonSize::Sm)
                .on_click(move |_, _, cx| {
                    let value = value_owned.clone();
                    let _ = vh.update(cx, |this, cx| {
                        this.settings_form_export_format = value.clone();
                        let mut config = crate::config::AppConfig::load();
                        config.default_export_format = value;
                        config.save();
                        cx.notify();
                    });
                })
        }));

        VStack::new().gap_6().child(settings_card(
            theme,
            "Export defaults",
            Some("Applied to every clip export unless overridden."),
            VStack::new().child(settings_row(theme, "Container", Option::<String>::None, row))
        ))
    }
}
