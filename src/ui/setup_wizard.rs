use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;
use gstreamer as gst;

#[derive(Clone, PartialEq, Eq)]
pub struct DetectedEncoder {
    pub id: String,
    pub label: String,
}

pub fn detect_available_encoders() -> Vec<DetectedEncoder> {
    let _ = gst::init();
    let candidates = [
        ("nvd3d11h264enc", "nvh264enc", "NVENC H.264 (NVIDIA)"),
        ("nvd3d11h265enc", "nvh265enc", "NVENC H.265/HEVC (NVIDIA)"),
        ("nvd3d11av1enc", "nvav1enc", "NVENC AV1 (NVIDIA)"),
        ("x264enc", "x264enc", "x264 (CPU Software)"),
    ];

    candidates
        .iter()
        .filter(|(gst_element, _, _)| gst::ElementFactory::find(gst_element).is_some())
        .map(|(_, config_id, label)| DetectedEncoder {
            id: config_id.to_string(),
            label: label.to_string(),
        })
        .collect()
}

impl LumaWorkspace {
    pub fn render_setup_wizard(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let view = cx.entity().downgrade();

        let step = self.setup_wizard_step;
        let storage_path = self.setup_storage_path.clone();
        let encoders = self.setup_detected_encoders.clone();
        let selected_encoder = self.setup_selected_encoder.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_ee))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Card::new().w(px(560.0)).content(
                    VStack::new()
                        .p_6()
                        .gap_6()
                        .child(self.render_wizard_header(step, &theme))
                        .child(match step {
                            0 => self.render_wizard_welcome(&theme).into_any_element(),
                            1 => self
                                .render_wizard_storage(&storage_path, &view, &theme)
                                .into_any_element(),
                            2 => self
                                .render_wizard_encoder(&encoders, &selected_encoder, &view, &theme)
                                .into_any_element(),
                            _ => self.render_wizard_finish(&theme).into_any_element(),
                        })
                        .child(self.render_wizard_nav(step, &view, &theme)),
                ),
            )
    }

    fn render_wizard_header(&self, step: usize, theme: &Theme) -> impl IntoElement {
        let total_steps = 4;
        VStack::new()
            .gap_3()
            .child(
                HStack::new().items_center().gap_3().child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.tokens.foreground)
                        .child("Welcome to Luma"),
                ),
            )
            .child(
                HStack::new()
                    .gap_1()
                    .children((0..total_steps).map(move |i| {
                        div().h(px(3.0)).flex_1().rounded(px(2.0)).bg(if i <= step {
                            theme.tokens.primary
                        } else {
                            theme.tokens.muted
                        })
                    })),
            )
    }

    fn render_wizard_welcome(&self, theme: &Theme) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Let's get you set up")
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.muted_foreground)
                    .child("Luma is a GPU-accelerated game recorder. This wizard will help you configure the essentials: where to store recordings and which encoder to use.")
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(self.render_feature_row("Screen capture via DXGI/WGC", theme))
                    .child(self.render_feature_row("Hardware-accelerated encoding (NVENC)", theme))
                    .child(self.render_feature_row("Multi-track audio recording", theme))
                    .child(self.render_feature_row("Instant replay with HLS segments", theme))
                    .child(self.render_feature_row("Automatic game detection", theme))
            )
    }

    fn render_feature_row(&self, text: &str, theme: &Theme) -> impl IntoElement {
        HStack::new()
            .gap_2()
            .items_center()
            .child(
                Icon::new("check-circle")
                    .size(px(16.0))
                    .color(theme.tokens.primary),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.foreground)
                    .child(text.to_string()),
            )
    }

    fn render_wizard_storage(
        &self,
        storage_path: &str,
        view: &WeakEntity<Self>,
        theme: &Theme,
    ) -> impl IntoElement {
        let view_clone = view.clone();
        let current_path = storage_path.to_string();

        VStack::new()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Storage Location")
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.muted_foreground)
                    .child("Choose where Luma will save recordings and clips. Use an SSD with at least 50 GB of free space for best results.")
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child("Recording path")
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .px_3()
                                    .py_2()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(theme.tokens.border)
                                    .bg(theme.tokens.input)
                                    .text_sm()
                                    .text_color(theme.tokens.foreground)
                                    .overflow_hidden()
                                    .child(current_path.clone())
                            )
                            .child(
                                Button::new("wizard-browse", "Browse")
                                    .variant(ButtonVariant::Outline)
                                    .on_click({
                                        let view = view_clone;
                                        move |_, _window, cx| {
                                            let view = view.clone();
                                            cx.spawn(|cx: &mut AsyncApp| {
                                                let mut cx = cx.clone();
                                                async move {
                                                    if let Some(path) = rfd::AsyncFileDialog::new()
                                                        .set_title("Choose recording folder")
                                                        .pick_folder()
                                                        .await
                                                    {
                                                        let path_str = path.path().to_string_lossy().to_string();
                                                        let _ = view.update(&mut cx, |this, cx| {
                                                            this.setup_storage_path = path_str;
                                                            cx.notify();
                                                        });
                                                    }
                                                }
                                            }).detach();
                                        }
                                    })
                            )
                    )
            )
    }

    fn render_wizard_encoder(
        &self,
        encoders: &[DetectedEncoder],
        selected: &str,
        view: &WeakEntity<Self>,
        theme: &Theme,
    ) -> impl IntoElement {
        let mut encoder_list = VStack::new().gap_2();

        if encoders.is_empty() {
            encoder_list = encoder_list.child(
                div()
                    .p_3()
                    .rounded(px(6.0))
                    .bg(theme.tokens.destructive)
                    .text_sm()
                    .text_color(theme.tokens.destructive_foreground)
                    .child("No encoders detected. Make sure GStreamer is installed with the NVENC plugin or x264 plugin.")
            );
        }

        for enc in encoders.iter() {
            let is_selected = enc.id == selected;
            let enc_id = enc.id.clone();
            let enc_label = enc.label.clone();
            let view = view.clone();

            encoder_list = encoder_list.child(
                div()
                    .id(SharedString::from(format!("enc-{}", enc_id)))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(if is_selected {
                        theme.tokens.primary
                    } else {
                        theme.tokens.border
                    })
                    .bg(if is_selected {
                        theme.tokens.accent
                    } else {
                        theme.tokens.card
                    })
                    .cursor(CursorStyle::PointingHand)
                    .hover(|s| s.bg(theme.tokens.accent))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let enc_id = enc_id.clone();
                        let _ = view.update(cx, |this, cx| {
                            this.setup_selected_encoder = enc_id;
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if is_selected {
                                theme.tokens.primary
                            } else {
                                theme.tokens.muted
                            })
                            .when(is_selected, |d| {
                                d.child(
                                    div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .m(px(2.0))
                                        .rounded_full()
                                        .bg(theme.tokens.primary),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child(enc_label),
                    ),
            );
        }

        VStack::new()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.foreground)
                    .child("Encoder Selection")
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.muted_foreground)
                    .child("Luma detected the following encoders on your system. Hardware encoders (NVENC) are recommended for minimal performance impact while gaming.")
            )
            .child(encoder_list)
    }

    fn render_wizard_finish(&self, theme: &Theme) -> impl IntoElement {
        let encoder_label = self
            .setup_detected_encoders
            .iter()
            .find(|e| e.id == self.setup_selected_encoder)
            .map(|e| e.label.as_str())
            .unwrap_or("Unknown");

        VStack::new()
            .gap_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.foreground)
                    .child("You're all set!")
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.muted_foreground)
                    .child("Here's a summary of your configuration. You can change these later in Settings.")
            )
            .child(
                VStack::new()
                    .gap_3()
                    .p_4()
                    .rounded(px(6.0))
                    .bg(theme.tokens.muted)
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Storage"))
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child(self.setup_storage_path.clone()))
                    )
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Encoder"))
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child(encoder_label.to_string()))
                    )
            )
    }

    fn render_wizard_nav(
        &self,
        step: usize,
        view: &WeakEntity<Self>,
        _theme: &Theme,
    ) -> impl IntoElement {
        let total_steps: usize = 4;
        let view_back = view.clone();
        let view_next = view.clone();

        HStack::new()
            .justify_between()
            .child(if step > 0 {
                Button::new("wizard-back", "Back")
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| {
                        let _ = view_back.update(cx, |this, cx| {
                            this.setup_wizard_step = this.setup_wizard_step.saturating_sub(1);
                            cx.notify();
                        });
                    })
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if step < total_steps - 1 {
                Button::new("wizard-next", "Continue")
                    .variant(ButtonVariant::Default)
                    .on_click(move |_, _, cx| {
                        let _ = view_next.update(cx, |this, cx| {
                            this.setup_wizard_step += 1;
                            cx.notify();
                        });
                    })
                    .into_any_element()
            } else {
                Button::new("wizard-finish", "Get Started")
                    .variant(ButtonVariant::Default)
                    .on_click(move |_, window, cx| {
                        let _ = view_next.update(cx, |this, cx| {
                            this.finish_setup_wizard(window, cx);
                        });
                    })
                    .into_any_element()
            })
    }

    pub fn finish_setup_wizard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = crate::config::AppConfig::load();

        config.storage_path = self.setup_storage_path.clone();
        config.global_video.encoder = self.setup_selected_encoder.clone();
        config.first_run_completed = true;
        config.save();

        // Ensure storage directory exists
        let _ = std::fs::create_dir_all(&config.storage_path);

        self.show_setup_wizard = false;

        self.show_toast(
            "Setup Complete",
            Some("Luma is ready. Add a game source from the dashboard to start recording."),
            adabraka_ui::overlays::toast::ToastVariant::Success,
            window,
            cx,
        );

        cx.notify();
    }
}
