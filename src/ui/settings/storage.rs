use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::charts::pie_chart::{PieChart, PieChartSegment, PieChartSize, PieChartLabelPosition};
use crate::ui::RekaptrWorkspace;
use super::{settings_card, settings_row, settings_toggle};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_storage(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();
        let config = crate::config::AppConfig::load();

        let clips_gb = self.storage_clips_mb as f64 / 1024.0;
        let sessions_gb = self.storage_sessions_mb as f64 / 1024.0;
        let total_gb = clips_gb + sessions_gb;

        VStack::new()
            .gap_6()
            .child(settings_card(theme, "Usage", Some("Where your captured video is stored."),
                HStack::new()
                    .gap_8()
                    .items_center()
                    .child(
                        VStack::new()
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Total usage"))
                            .child(div().text_3xl().font_weight(FontWeight::BOLD).text_color(theme.tokens.foreground).child(format!("{:.1} GB", total_gb)))
                    )
                    .child(
                        div().flex_1().child(
                            PieChart::new(vec![
                                PieChartSegment::new("Clips", clips_gb).color(theme.tokens.primary),
                                PieChartSegment::new("Sessions", sessions_gb).color(theme.tokens.muted),
                            ])
                            .size(PieChartSize::Custom(100))
                            .label_position(PieChartLabelPosition::None)
                        )
                    )
            ))
            .child(settings_card(theme, "Location & buffer", None,
                VStack::new()
                    .child(settings_row(theme, "Base storage path", Some(config.storage_path.clone()),
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
                    .child(settings_row(theme, "Buffer size limit",
                        Some(format!("{} GB", self.form_max_buffer_size_gb)),
                        HStack::new().gap_2()
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
            ))
            .child(settings_card(theme, "Cleanup", None,
                VStack::new()
                    .child(settings_row(theme, "Auto-delete old clips", Option::<String>::None,
                        settings_toggle("toggle-autodel", self.settings.auto_delete_enabled, vh.clone(), |this, cx| {
                            this.settings.auto_delete_enabled = !this.settings.auto_delete_enabled;
                            let mut config = crate::config::AppConfig::load();
                            config.auto_delete_clips_days = if this.settings.auto_delete_enabled {
                                Some(this.settings.auto_delete_days)
                            } else { None };
                            config.save();
                            cx.notify();
                        })
                    ))
                    .when(self.settings.auto_delete_enabled, |this| {
                        this.child(settings_row(theme, "Retention (days)",
                            Some(format!("{} days", self.settings.auto_delete_days)),
                            HStack::new().gap_2()
                                .child(Button::new("days-dec", "-").size(ButtonSize::Sm).variant(ButtonVariant::Outline).on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        let _ = vh.update(cx, |this, cx| {
                                            this.settings.auto_delete_days = (this.settings.auto_delete_days - 1).max(1);
                                            let mut config = crate::config::AppConfig::load();
                                            config.auto_delete_clips_days = Some(this.settings.auto_delete_days);
                                            config.save();
                                            cx.notify();
                                        });
                                    }
                                }))
                                .child(Button::new("days-inc", "+").size(ButtonSize::Sm).variant(ButtonVariant::Outline).on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        let _ = vh.update(cx, |this, cx| {
                                            this.settings.auto_delete_days = (this.settings.auto_delete_days + 1).min(365);
                                            let mut config = crate::config::AppConfig::load();
                                            config.auto_delete_clips_days = Some(this.settings.auto_delete_days);
                                            config.save();
                                            cx.notify();
                                        });
                                    }
                                }))
                        ))
                    })
            ))
    }
}
