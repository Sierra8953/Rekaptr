use crate::config::AppConfig;
use crate::ui::RekaptrWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;

impl RekaptrWorkspace {
    pub fn save_clip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clip_start >= 0.0 && self.clip_end >= 0.0 {
            self.show_export_modal = true;
            cx.notify();
        } else {
            self.show_toast(
                "Set IN and OUT points first",
                Some("Use the timeline to mark the start and end of your clip."),
                adabraka_ui::overlays::toast::ToastVariant::Warning,
                window,
                cx,
            );
        }
    }

    pub fn perform_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.clip_start;
        let end = self.clip_end;
        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        crate::utils::generate_master_playlist(&source_name);

        let safe_title = crate::utils::clean_title(&source_name);
        let storage_root = crate::utils::get_storage_root();
        let playlist_path = storage_root.join(&safe_title).join("master.m3u8");

        if !playlist_path.exists() {
            self.show_toast(
                "Source Error",
                Some("Recording segments not found."),
                adabraka_ui::overlays::toast::ToastVariant::Error,
                window,
                cx,
            );
            return;
        }

        let clips_dir = storage_root.join("Clips").join(&safe_title);
        let _ = std::fs::create_dir_all(&clips_dir);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let output_path = clips_dir.join(format!("clip_{}_{}.mp4", safe_title, timestamp));

        let ffmpeg_path = crate::utils::get_ffmpeg_path();

        if ffmpeg_path.to_str() != Some("ffmpeg") && !ffmpeg_path.exists() {
            self.show_toast(
                "FFmpeg Not Found",
                Some("Place ffmpeg.exe in the bin/ folder next to Rekaptr, or install it to PATH."),
                adabraka_ui::overlays::toast::ToastVariant::Error,
                window,
                cx,
            );
            return;
        }

        *self.app_state.export.phase.lock() = crate::state::ExportPhase::Exporting;
        *self.app_state.export.progress.lock() = 0.0;

        let encoder = self.export_encoder.clone();
        let bitrate = self.export_bitrate;
        let preset = self.export_preset.clone();
        let export_reencode = self.export_reencode;

        cx.notify();

        let config = AppConfig::load();
        let audio_tracks = if source_name == "monitor" {
            config.global_audio_tracks.clone()
        } else {
            config
                .game_registry
                .get(&source_name)
                .and_then(|g| g.audio_routing.as_ref())
                .cloned()
                .unwrap_or(config.global_audio_tracks.clone())
        };

        let app_state_for_progress = self.app_state.clone();
        let view_handle = cx.entity().downgrade();

        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                for i in 1..=100 {
                    let progress = i as f32 / 100.0;
                    *app_state_for_progress.export.progress.lock() = progress;
                    let _ = view_handle.update(&mut cx, |_, cx| cx.notify());
                    let _ = cx
                        .background_executor()
                        .timer(std::time::Duration::from_millis(if export_reencode {
                            50
                        } else {
                            5
                        }))
                        .await;
                    if *app_state_for_progress.export.phase.lock()
                        != crate::state::ExportPhase::Exporting
                    {
                        break;
                    }
                }
            }
        })
        .detach();

        let ffmpeg_task = cx.background_spawn(async move {
            use std::process::Command;

            let build_cmd = |hwaccel: bool| {
                let mut cmd = Command::new(ffmpeg_path.clone());
                cmd.arg("-y");
                if hwaccel {
                    cmd.arg("-hwaccel")
                        .arg("cuda")
                        .arg("-hwaccel_output_format")
                        .arg("cuda");
                }
                cmd.arg("-ss")
                    .arg(format!("{:.3}", start))
                    .arg("-to")
                    .arg(format!("{:.3}", end))
                    .arg("-allowed_extensions")
                    .arg("ALL")
                    .arg("-i")
                    .arg(playlist_path.clone())
                    .arg("-map")
                    .arg("0:v:0");

                let mut physical_stream_idx = 0;
                for track in &audio_tracks {
                    if track.enabled {
                        cmd.arg("-map").arg(format!("0:a:{}?", physical_stream_idx));
                    }
                    physical_stream_idx += 1;
                }

                if export_reencode {
                    cmd.arg("-c:v")
                        .arg(&encoder)
                        .arg("-preset")
                        .arg(&preset)
                        .arg("-b:v")
                        .arg(format!("{}k", bitrate));
                } else {
                    cmd.arg("-c:v").arg("copy");
                }

                cmd.arg("-c:a")
                    .arg("aac")
                    .arg("-b:a")
                    .arg("320k")
                    .arg("-ar")
                    .arg("48000")
                    .arg("-movflags")
                    .arg("+faststart")
                    .arg(&output_path);
                cmd
            };

            // Try with CUDA hardware decoding first, fall back to software
            let mut cmd = build_cmd(true);
            log::info!("[Export] Running FFmpeg (hwaccel cuda): {:?}", cmd);
            let clip_output = match cmd.output() {
                Ok(out) if out.status.success() => Ok(out),
                _ => {
                    log::warn!("[Export] CUDA decode failed, retrying with software decoder");
                    let mut cmd = build_cmd(false);
                    log::info!("[Export] Running FFmpeg (software): {:?}", cmd);
                    cmd.output()
                }
            };

            // Extract a thumbnail from the middle of the clip
            let thumb_time = (end - start) / 2.0;
            let mut thumb_path = output_path.clone();
            thumb_path.set_extension("jpg");

            if clip_output.as_ref().map_or(false, |o| o.status.success()) {
                let mut thumb_cmd = Command::new(&ffmpeg_path);
                thumb_cmd
                    .arg("-y")
                    .arg("-ss")
                    .arg(format!("{:.3}", thumb_time))
                    .arg("-i")
                    .arg(&output_path)
                    .arg("-vframes")
                    .arg("1")
                    .arg("-q:v")
                    .arg("2")
                    .arg(&thumb_path);

                log::info!("[Export] Generating thumbnail: {:?}", thumb_cmd);
                if let Ok(out) = thumb_cmd.output() {
                    if !out.status.success() {
                        log::warn!(
                            "[Export] Thumbnail generation failed: {}",
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                }
            }

            (clip_output, output_path, clips_dir)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (result, output_path, clips_dir) = ffmpeg_task.await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.show_export_modal = false;
                    this.export_reencode = false;
                    this.export_encoder = "h264_nvenc".to_string();
                    this.export_bitrate = 50000;
                    this.export_preset = "p4".to_string();
                    this.export_crf = 23;
                    *this.app_state.export.phase.lock() = crate::state::ExportPhase::Idle;

                    if let Some(any_window) = cx.windows().first() {
                        let _ = any_window.update(cx, |_, window, cx| match result {
                            Ok(output) => {
                                if output.status.success() {
                                    this.clip_start = -1.0;
                                    this.clip_end = -1.0;
                                    this.show_toast(
                                        SharedString::from("Clip Saved"),
                                        Some(SharedString::from(format!(
                                            "Exported to {:?}",
                                            output_path
                                        ))),
                                        adabraka_ui::overlays::toast::ToastVariant::Success,
                                        window,
                                        cx,
                                    );
                                    let _ = std::process::Command::new("explorer")
                                        .arg(&clips_dir)
                                        .spawn();
                                } else {
                                    let err = String::from_utf8_lossy(&output.stderr);
                                    log::error!("[Export] FFmpeg failed: {}", err);
                                    let err_summary = err
                                        .lines()
                                        .rev()
                                        .find(|l| !l.trim().is_empty())
                                        .unwrap_or("FFmpeg returned an error.")
                                        .to_string();
                                    this.show_toast(
                                        SharedString::from("Export Failed"),
                                        Some(SharedString::from(err_summary)),
                                        adabraka_ui::overlays::toast::ToastVariant::Error,
                                        window,
                                        cx,
                                    );
                                }
                            }
                            Err(e) => {
                                log::error!("[Export] Failed to run FFmpeg: {}", e);
                                this.show_toast(
                                    SharedString::from("FFmpeg Error"),
                                    Some(SharedString::from("Could not locate or run ffmpeg.exe")),
                                    adabraka_ui::overlays::toast::ToastVariant::Error,
                                    window,
                                    cx,
                                );
                            }
                        });
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn render_export_modal(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Card::new()
                    .w(px(400.0))
                    .content(
                        VStack::new()
                            .p_6()
                            .gap_6()
                            .child(
                                VStack::new()
                                    .gap_1()
                                    .child(div().text_xl().font_weight(FontWeight::BOLD).child("Export Settings"))
                                    .child(div().text_sm().text_color(theme.tokens.muted_foreground).child("Choose how you want to save this clip."))
                            )
                            .child(
                                VStack::new()
                                    .gap_4()
                                    .child(
                                        HStack::new()
                                            .gap_3()
                                            .items_center()
                                            .child({
                                                let view = cx.entity().downgrade();
                                                adabraka_ui::components::radio::Radio::new("instant-copy")
                                                    .checked(!self.export_reencode)
                                                    .on_click(move |_, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            this.export_reencode = false;
                                                            cx.notify();
                                                        });
                                                    })
                                            })
                                            .child(
                                                VStack::new()
                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Instant Copy (Recommended)"))
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Lossless, saves in less than a second."))
                                            )
                                    )
                                    .child(
                                        HStack::new()
                                            .gap_3()
                                            .items_center()
                                            .child({
                                                let view = cx.entity().downgrade();
                                                adabraka_ui::components::radio::Radio::new("re-encode")
                                                    .checked(self.export_reencode)
                                                    .on_click(move |_, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            this.export_reencode = true;
                                                            cx.notify();
                                                        });
                                                    })
                                            })
                                            .child(
                                                VStack::new()
                                                    .child(div().font_weight(FontWeight::MEDIUM).child("Re-encode (Complete MP4)"))
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("Choose quality and format for best compatibility."))
                                            )
                                    )
                            )
                            .when(self.export_reencode, |this| {
                                this.child(self.render_reencode_options(cx))
                            })
                            .child(
                                HStack::new()
                                    .justify_end()
                                    .gap_3()
                                    .child(
                                        Button::new("cancel-export", "Cancel")
                                            .variant(ButtonVariant::Ghost)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_export_modal = false;
                                                cx.notify();
                                            }))
                                    )
                                    .child(
                                        Button::new("start-export", "Start Export")
                                            .variant(ButtonVariant::Default)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.perform_export(window, cx);
                                            }))
                                    )
                            )
                            .child({
                                let export_running = *self.app_state.export.phase.lock() == crate::state::ExportPhase::Exporting;
                                let progress = *self.app_state.export.progress.lock();

                                div()
                                    .when(export_running, |this| {
                                        this.absolute()
                                            .inset_0()
                                            .bg(theme.tokens.card)
                                            .rounded_xl()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .p_6()
                                            .gap_4()
                                            .child(Spinner::new().size(SpinnerSize::Xl))
                                            .child(div().text_lg().font_weight(FontWeight::BOLD).child("Exporting Clip..."))
                                            .child(
                                                VStack::new()
                                                    .w_full()
                                                    .gap_1()
                                                    .child(
                                                        adabraka_ui::components::progress::ProgressBar::new(progress)
                                                            .h(px(8.0))
                                                    )
                                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).text_center().child(format!("{:.0}%", progress * 100.0)))
                                            )
                                    })
                            })
                    )
            )
    }

    fn render_reencode_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        VStack::new()
            .gap_4()
            .p_4()
            .bg(theme.tokens.muted.opacity(0.5))
            .rounded_md()
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.muted_foreground)
                            .child("ENCODER"),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(Self::encoder_button(
                                "exp-enc-h264",
                                "H.264",
                                "h264_nvenc",
                                &self.export_encoder,
                                cx,
                            ))
                            .child(Self::encoder_button(
                                "exp-enc-hevc",
                                "HEVC",
                                "hevc_nvenc",
                                &self.export_encoder,
                                cx,
                            ))
                            .child(Self::encoder_button(
                                "exp-enc-av1",
                                "AV1",
                                "av1_nvenc",
                                &self.export_encoder,
                                cx,
                            )),
                    ),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.muted_foreground)
                            .child("QUALITY PRESET"),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(Self::preset_button(
                                "exp-pre-fast",
                                "Fast",
                                "p1",
                                &self.export_preset,
                                cx,
                            ))
                            .child(Self::preset_button(
                                "exp-pre-bal",
                                "Balanced",
                                "p4",
                                &self.export_preset,
                                cx,
                            ))
                            .child(Self::preset_button(
                                "exp-pre-hq",
                                "High Quality",
                                "p7",
                                &self.export_preset,
                                cx,
                            )),
                    ),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.muted_foreground)
                            .child("BITRATE (kbps)"),
                    )
                    .child(
                        HStack::new()
                            .gap_4()
                            .items_center()
                            .child(
                                Button::new("exp-bit-dec", "-")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_bitrate =
                                            (this.export_bitrate - 5000).max(1000);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p_2()
                                    .bg(theme.tokens.background)
                                    .rounded_md()
                                    .child(
                                        div()
                                            .text_center()
                                            .font_weight(FontWeight::BOLD)
                                            .child(format!("{}k", self.export_bitrate)),
                                    ),
                            )
                            .child(
                                Button::new("exp-bit-inc", "+")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_bitrate =
                                            (this.export_bitrate + 5000).min(100000);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    fn encoder_button(
        id: &str,
        label: &'static str,
        value: &str,
        current: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let value_owned = value.to_string();
        Button::new(SharedString::from(id.to_string()), label)
            .variant(if current == value {
                ButtonVariant::Default
            } else {
                ButtonVariant::Outline
            })
            .size(ButtonSize::Sm)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.export_encoder = value_owned.clone();
                cx.notify();
            }))
    }

    fn preset_button(
        id: &str,
        label: &'static str,
        value: &str,
        current: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let value_owned = value.to_string();
        Button::new(SharedString::from(id.to_string()), label)
            .variant(if current == value {
                ButtonVariant::Default
            } else {
                ButtonVariant::Outline
            })
            .size(ButtonSize::Sm)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.export_preset = value_owned.clone();
                cx.notify();
            }))
    }
}
