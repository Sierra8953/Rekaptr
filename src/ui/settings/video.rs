use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{section_header, settings_row, settings_toggle, stepper};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_video(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();

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
                            div().w(px(180.0)).child(self.select_encoder.clone())
                        ))
                        .child(settings_row(theme, "Resolution", Option::<String>::None,
                            div().w(px(180.0)).child(self.select_resolution.clone())
                        ))
                        .child(settings_row(theme, "Framerate", Option::<String>::None,
                            div().w(px(180.0)).child(self.select_fps.clone())
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
                                    div().w(px(180.0)).child(self.select_preset.clone())
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
}
