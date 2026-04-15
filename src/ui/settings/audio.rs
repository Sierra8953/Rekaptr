use gpui::*;
use adabraka_ui::prelude::*;
use gstreamer::prelude::*;
use crate::ui::RekaptrWorkspace;
use super::{section_header, settings_row, settings_toggle, stepper_f32};

impl RekaptrWorkspace {
    pub(crate) fn render_settings_audio(&self, theme: &Theme, view_handle: &WeakEntity<Self>, _cx: &mut Context<Self>) -> impl IntoElement {
        let vh = view_handle.clone();
        let mut devices_raw = crate::engine::enumerate_audio_devices(true);
        if devices_raw.is_empty() {
            devices_raw.push(("default".to_string(), "Default".to_string()));
        }
        let devices = devices_raw;

        let is_monitoring = self.mic_monitor_pipeline.is_some();

        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            // Microphone Source card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Microphone Source"))
                        .child(settings_row(theme, "Input Device", Option::<String>::None,
                            {
                                // Resolve the stored device ID to a friendly name for display
                                let display_name = devices.iter()
                                    .find(|(id, _)| *id == self.settings_form_mic_device)
                                    .map(|(_, label)| label.clone())
                                    .unwrap_or_else(|| self.settings_form_mic_device.clone());
                                adabraka_ui::components::dropdown::Dropdown::new(self.dd_mic.clone(),
                                    Button::new("trigger-mic", display_name).size(ButtonSize::Sm).variant(ButtonVariant::Outline))
                                    .items(devices.into_iter().map(|(id, label)| {
                                        let vh = vh.clone();
                                        let dev_id = id.clone();
                                        DropdownItem::new(id, label)
                                            .on_click(move |_, cx| {
                                                let _ = vh.update(cx, |this, cx| {
                                                    this.settings_form_mic_device = dev_id.clone();
                                                    let mut config = crate::config::AppConfig::load();
                                                    config.mic_settings.device_name = dev_id.clone();
                                                    config.save();
                                                    cx.notify();
                                                });
                                            })
                                    }).collect::<Vec<_>>())
                            }
                        ))
                        .child(settings_row(theme, "Force Mono", Option::<String>::None,
                            settings_toggle("toggle-mono", self.settings_form_mic_force_mono, vh.clone(), |this, cx| {
                                this.settings_form_mic_force_mono = !this.settings_form_mic_force_mono;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.force_mono = this.settings_form_mic_force_mono;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Gain (dB)", Some(format!("{:.1} dB", self.settings_form_mic_gain)),
                            stepper_f32("gain", self.settings_form_mic_gain, -20.0, 20.0, 0.5, vh.clone(), |this, val, cx| {
                                this.settings_form_mic_gain = val;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.gain_db = val;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Monitor Mic", Some(if is_monitoring { "Listening..." } else { "Test your mic with current settings" }),
                            Button::new("toggle-monitor", if is_monitoring { "Stop" } else { "Monitor" })
                                .variant(if is_monitoring { ButtonVariant::Destructive } else { ButtonVariant::Outline })
                                .size(ButtonSize::Sm)
                                .on_click({
                                    let vh = vh.clone();
                                    move |_, _, cx| {
                                        let _ = vh.update(cx, |this, cx| {
                                            if let Some(pipeline) = this.mic_monitor_pipeline.take() {
                                                let _ = pipeline.set_state(gstreamer::State::Null);
                                                // Unsubscribe from mic provider
                                                if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                                                    provider.subscribers.remove(&0xFFFF_FFFF_FFFF_FFFFu64);
                                                }
                                                cx.notify();
                                                return;
                                            }
                                            // Build a monitor pipeline that receives DSP-processed audio
                                            // from the mic provider via appsrc -> wasapi2sink.
                                            let pipeline_str = "appsrc name=monitor_src format=time is-live=true do-timestamp=true ! audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved ! queue max-size-time=200000000 ! audioconvert ! audioresample ! wasapi2sink low-latency=true provide-clock=true";
                                            log::info!("[MicMonitor] Launching pipeline: {}", pipeline_str);
                                            match gstreamer::parse::launch(pipeline_str) {
                                                Ok(element) => {
                                                    if let Ok(pipeline) = element.downcast::<gstreamer::Pipeline>() {
                                                        // Subscribe to the mic provider's DSP-processed output
                                                        if let Some(provider) = this.app_state.mic_provider.lock().as_ref() {
                                                            if let Some(appsrc) = pipeline.by_name("monitor_src")
                                                                .and_then(|e| e.downcast::<gstreamer_app::AppSrc>().ok())
                                                            {
                                                                let monitor_id = 0xFFFF_FFFF_FFFF_FFFFu64; // Reserved ID for monitor
                                                                provider.subscribers.insert(monitor_id, appsrc);
                                                                let _ = pipeline.set_state(gstreamer::State::Playing);
                                                                this.mic_monitor_pipeline = Some(pipeline);
                                                                log::info!("[MicMonitor] Pipeline started (receiving from mic provider)");
                                                            }
                                                        } else {
                                                            log::error!("[MicMonitor] Mic provider not running");
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("[MicMonitor] Failed to create pipeline: {}", e);
                                                }
                                            }
                                            cx.notify();
                                        });
                                    }
                                })
                        ))
                )
            )
            // Processing & FX card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Processing & FX"))
                        .child(settings_row(theme, "Noise Suppression (RNNoise)", Some("Requires mic restart"),
                            settings_toggle("toggle-ns", self.settings_form_mic_noise_suppression, vh.clone(), |this, cx| {
                                this.settings_form_mic_noise_suppression = !this.settings_form_mic_noise_suppression;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.noise_suppression = this.settings_form_mic_noise_suppression;
                                config.save();
                                // Restart the mic provider since audiornnoise can only be
                                // added/removed by rebuilding the GStreamer pipeline.
                                this.restart_mic_provider();
                                cx.notify();
                            })
                        ))
                        .child(settings_row(theme, "Noise Gate", Option::<String>::None,
                            settings_toggle("toggle-gate", self.settings_form_mic_gate_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_gate_enabled = !this.settings_form_mic_gate_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.noise_gate_enabled = this.settings_form_mic_gate_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_gate_enabled, |this| {
                            this.child(settings_row(theme, "Gate Threshold", Some(format!("{:.0} dB", self.settings_form_mic_gate_threshold)),
                                stepper_f32("gt", self.settings_form_mic_gate_threshold, -80.0, 0.0, 1.0, vh.clone(), |this, val, cx| {
                                    this.settings_form_mic_gate_threshold = val;
                                    let mut config = crate::config::AppConfig::load();
                                    config.mic_settings.noise_gate_threshold = val;
                                    config.save();
                                    this.notify_mic_dsp_changed();
                                    cx.notify();
                                })
                            ))
                        })
                )
            )
            // Compressor card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Compressor"))
                        .child(settings_row(theme, "Enable Compressor", Option::<String>::None,
                            settings_toggle("toggle-comp", self.settings_form_mic_compressor_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_compressor_enabled = !this.settings_form_mic_compressor_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.compressor_enabled = this.settings_form_mic_compressor_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_compressor_enabled, |this| {
                            this
                                .child(settings_row(theme, "Threshold", Some(format!("{:.0} dB", self.settings_form_mic_compressor_threshold)),
                                    stepper_f32("ct", self.settings_form_mic_compressor_threshold, -60.0, 0.0, 1.0, vh.clone(), |this, val, cx| {
                                        this.settings_form_mic_compressor_threshold = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.mic_settings.compressor_threshold = val;
                                        config.save();
                                        this.notify_mic_dsp_changed();
                                        cx.notify();
                                    })
                                ))
                                .child(settings_row(theme, "Ratio", Some(format!("{:.1}:1", self.settings_form_mic_compressor_ratio)),
                                    stepper_f32("cr", self.settings_form_mic_compressor_ratio, 1.0, 20.0, 0.5, vh.clone(), |this, val, cx| {
                                        this.settings_form_mic_compressor_ratio = val;
                                        let mut config = crate::config::AppConfig::load();
                                        config.mic_settings.compressor_ratio = val;
                                        config.save();
                                        this.notify_mic_dsp_changed();
                                        cx.notify();
                                    })
                                ))
                        })
                )
            )
            // Limiter card
            .child(
                Card::new().content(
                    VStack::new()
                        .p_6()
                        .gap_1()
                        .child(section_header("Limiter"))
                        .child(settings_row(theme, "Enable Limiter", Option::<String>::None,
                            settings_toggle("toggle-lim", self.settings_form_mic_limiter_enabled, vh.clone(), |this, cx| {
                                this.settings_form_mic_limiter_enabled = !this.settings_form_mic_limiter_enabled;
                                let mut config = crate::config::AppConfig::load();
                                config.mic_settings.limiter_enabled = this.settings_form_mic_limiter_enabled;
                                config.save();
                                this.notify_mic_dsp_changed();
                                cx.notify();
                            })
                        ))
                        .when(self.settings_form_mic_limiter_enabled, |this| {
                            this.child(settings_row(theme, "Threshold", Some(format!("{:.0} dB", self.settings_form_mic_limiter_threshold)),
                                stepper_f32("lt", self.settings_form_mic_limiter_threshold, -30.0, 0.0, 0.5, vh.clone(), |this, val, cx| {
                                    this.settings_form_mic_limiter_threshold = val;
                                    let mut config = crate::config::AppConfig::load();
                                    config.mic_settings.limiter_threshold = val;
                                    config.save();
                                    this.notify_mic_dsp_changed();
                                    cx.notify();
                                })
                            ))
                        })
                )
            )
    }
}
