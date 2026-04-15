use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::charts::pie_chart::{PieChart, PieChartSegment, PieChartSize, PieChartLabelPosition};
use crate::ui::RekaptrWorkspace;
use super::{section_header, settings_row, settings_toggle};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_storage(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
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
}
