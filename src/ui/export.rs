use crate::config::AppConfig;
use crate::state::ExportStage;
use crate::ui::RekaptrWorkspace;
use adabraka_ui::components::input::Input;
use adabraka_ui::prelude::*;
use gpui::*;

const SUCCESS: u32 = 0x22C55EFF;
const SUCCESS_DIM: u32 = 0x22C55E40;

impl RekaptrWorkspace {
    /// Prepare export state (duration, audio-track snapshot + frozen stream
    /// mapping, destination, cleared title) from the current clip marks and
    /// selected source. Returns false (with a toast) if IN/OUT aren't both set.
    /// Shared by the desktop dialog (`save_clip`) and the overlay export
    /// (`export_from_overlay`).
    pub fn setup_export(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (in_mark, out_mark) = match (self.clip_start_mark.clone(), self.clip_end_mark.clone()) {
            (Some(i), Some(o)) => (i, o),
            _ => {
                self.show_toast(
                    "Set IN and OUT points first",
                    Some("Use the timeline to mark the start and end of your clip."),
                    adabraka_ui::overlays::toast::ToastVariant::Warning,
                    window,
                    cx,
                );
                return false;
            }
        };

        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        self.export_clip_duration =
            crate::utils::clip_duration_from_marks(&source_name, &in_mark, &out_mark).unwrap_or(0.0);

        // Snapshot the audio tracks for this source so the dialog can toggle them
        // independently. The order must match what perform_export maps to ffmpeg.
        let config = AppConfig::load();
        self.export_audio_tracks = if source_name == "monitor" {
            config.global_audio_tracks.clone()
        } else {
            config
                .game_registry
                .get(&source_name)
                .and_then(|g| g.audio_routing.as_ref())
                .cloned()
                .unwrap_or_else(|| config.global_audio_tracks.clone())
        };

        // Freeze the physical audio-stream layout. The recorder only writes a
        // stream for tracks that were enabled (see engine::generate_pipeline_string),
        // so streams are densely packed in enabled order. Capture that mapping now,
        // before the user can toggle tracks in the dialog.
        self.export_track_stream_idx = {
            let mut phys = 0usize;
            self.export_audio_tracks
                .iter()
                .map(|t| {
                    if t.enabled {
                        let idx = phys;
                        phys += 1;
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect()
        };

        let safe_title = crate::utils::clean_title(&source_name);
        self.export_destination = crate::utils::get_storage_root()
            .join("Clips")
            .join(&safe_title);

        self.export_title_input
            .update(cx, |input, cx| input.set_value(SharedString::from(""), window, cx));
        true
    }

    pub fn save_clip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.setup_export(window, cx) {
            return;
        }
        self.export_stage = ExportStage::Configure;
        self.export_result_path = None;
        self.export_result_size_mb = None;
        self.show_export_modal = true;
        cx.notify();
    }

    /// Export a clip marked in the in-game overlay, applying the options chosen in
    /// the overlay's own dialog and running the shared `perform_export` backend
    /// headlessly (no desktop modal). Progress/result flow through the shared
    /// `app_state.export` state and a `ClipSaved` event back to the overlay.
    pub fn export_from_overlay(
        &mut self,
        req: crate::state::OverlayClipRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_source = Some(req.source);
        self.clip_start_mark = Some(req.in_mark);
        self.clip_end_mark = Some(req.out_mark);
        if !self.setup_export(window, cx) {
            return;
        }
        self.export_reencode = req.reencode;
        self.export_encoder = req.encoder;
        self.export_bitrate = req.bitrate;
        self.export_container = req.container;
        // Apply the overlay's per-track include choices on top of the frozen
        // record-time stream mapping computed in setup_export.
        for (i, enabled) in req.audio_enabled.iter().enumerate() {
            if let Some(t) = self.export_audio_tracks.get_mut(i) {
                t.enabled = *enabled;
            }
        }
        if !req.title.trim().is_empty() {
            self.export_title_input
                .update(cx, |input, cx| input.set_value(SharedString::from(req.title), window, cx));
        }
        self.perform_export(window, cx);
    }

    fn close_export_modal(&mut self, cx: &mut Context<Self>) {
        self.show_export_modal = false;
        self.export_stage = ExportStage::Configure;
        self.export_result_path = None;
        self.export_result_size_mb = None;
        cx.notify();
    }

    pub fn perform_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let in_mark = match self.clip_start_mark.clone() {
            Some(m) => m,
            None => {
                self.show_toast("IN point not set", None::<&str>,
                    adabraka_ui::overlays::toast::ToastVariant::Warning, window, cx);
                return;
            }
        };
        let out_mark = match self.clip_end_mark.clone() {
            Some(m) => m,
            None => {
                self.show_toast("OUT point not set", None::<&str>,
                    adabraka_ui::overlays::toast::ToastVariant::Warning, window, cx);
                return;
            }
        };
        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        let safe_title = crate::utils::clean_title(&source_name);

        let (concat_path, in_offset, out_offset) =
            match crate::utils::build_clip_concat_list_from_marks(&source_name, &in_mark, &out_mark) {
                Some(t) => t,
                None => {
                    self.show_toast(
                        "Source Error",
                        Some("One or both marked segments are no longer on disk."),
                        adabraka_ui::overlays::toast::ToastVariant::Error,
                        window,
                        cx,
                    );
                    return;
                }
            };
        log::info!(
            "[Export] in=(sid={:?}, idx={}, off={:.3}) out=(sid={:?}, idx={}, off={:.3}) concat_list={} in_offset={:.3} out_offset={:.3}",
            in_mark.session_id, in_mark.segment_index, in_mark.offset_in_segment,
            out_mark.session_id, out_mark.segment_index, out_mark.offset_in_segment,
            concat_path.display(), in_offset, out_offset
        );

        let clips_dir = self.export_destination.clone();
        let _ = std::fs::create_dir_all(&clips_dir);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let title_text = self.export_title_input.read(cx).content().trim().to_string();
        let file_stem = if title_text.is_empty() {
            format!("clip_{}_{}", safe_title, timestamp)
        } else {
            crate::utils::clean_title(&title_text)
        };
        let container = self.export_container.clone();
        let output_path = clips_dir.join(format!("{}.{}", file_stem, container));

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
        let audio_tracks = self.export_audio_tracks.clone();
        let track_stream_idx = self.export_track_stream_idx.clone();

        self.export_stage = ExportStage::Exporting;
        cx.notify();

        let app_state_for_progress = self.app_state.clone();
        let app_state_for_ffmpeg = self.app_state.clone();
        let view_handle = cx.entity().downgrade();

        // UI repaint ticker: the real progress value is written from the ffmpeg
        // reader thread into `export.progress`; this just repaints the bar while
        // an export is in flight.
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                while *app_state_for_progress.export.phase.lock() == crate::state::ExportPhase::Exporting {
                    let _ = view_handle.update(&mut cx, |_, cx| cx.notify());
                    let _ = cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                }
            }
        }).detach();

        let total_dur_secs = (out_offset - in_offset).max(0.001);

        let ffmpeg_task = cx.background_spawn(async move {
            use std::os::windows::process::CommandExt;
            use std::process::{Command, Stdio};

            // Physical stream indices for the tracks the user kept enabled, using
            // each track's frozen record-time mapping (None = no stream existed).
            let enabled_streams: Vec<usize> = audio_tracks
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    if t.enabled {
                        track_stream_idx.get(i).copied().flatten()
                    } else {
                        None
                    }
                })
                .collect();

            let build_cmd = |hwaccel: bool| {
                let mut cmd = Command::new(ffmpeg_path.clone());
                cmd.creation_flags(0x08000000);
                cmd.arg("-y");
                // Emit machine-readable progress on stdout; ffmpeg's normal logs
                // stay on stderr, so the two never collide.
                cmd.arg("-progress").arg("pipe:1").arg("-nostats");
                if hwaccel {
                    cmd.arg("-hwaccel").arg("cuda")
                       .arg("-hwaccel_output_format").arg("cuda");
                }
                cmd.arg("-f").arg("concat")
                   .arg("-safe").arg("0")
                   .arg("-i").arg(concat_path.clone())
                   .arg("-ss").arg(format!("{:.3}", in_offset))
                   .arg("-to").arg(format!("{:.3}", out_offset))
                   .arg("-map").arg("0:v:0");

                // Combine the enabled audio tracks into a single output track:
                // one stream maps straight through; several are mixed with `amix`
                // (normalize=0 preserves each source's level instead of dividing
                // the volume by the input count).
                match enabled_streams.as_slice() {
                    [] => {}
                    [phys] => {
                        // Trailing `?` makes the map optional: if this track's
                        // stream isn't actually present in the spanned segments
                        // (e.g. an older clip recorded before the track existed),
                        // ffmpeg drops it instead of aborting the whole export.
                        cmd.arg("-map").arg(format!("0:a:{}?", phys));
                    }
                    streams => {
                        let mut filter = String::new();
                        for &phys in streams {
                            filter.push_str(&format!("[0:a:{}]", phys));
                        }
                        filter.push_str(&format!(
                            "amix=inputs={}:normalize=0:duration=longest[aout]",
                            streams.len()
                        ));
                        cmd.arg("-filter_complex").arg(filter).arg("-map").arg("[aout]");
                    }
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

                cmd.arg("-c:a").arg("aac")
                    .arg("-b:a").arg("320k")
                    .arg("-ar").arg("48000");
                if container == "mp4" || container == "mov" {
                    cmd.arg("-movflags").arg("+faststart");
                }
                cmd.arg(&output_path);
                cmd
            };

            // Spawn ffmpeg, stream `-progress` from stdout to drive the real
            // progress bar, and collect stderr for error reporting. Mirrors
            // `Command::output()` semantics but with live progress.
            let run = |mut cmd: Command| -> std::io::Result<std::process::Output> {
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                let mut child = cmd.spawn()?;

                let reader = child.stdout.take().map(|stdout| {
                    let progress_state = app_state_for_ffmpeg.clone();
                    std::thread::spawn(move || {
                        use std::io::{BufRead, BufReader};
                        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                            // ffmpeg reports position as out_time_us (newer) or
                            // out_time_ms (older builds emit microseconds here too).
                            let micros = line.strip_prefix("out_time_us=")
                                .or_else(|| line.strip_prefix("out_time_ms="))
                                .and_then(|v| v.trim().parse::<i64>().ok());
                            if let Some(us) = micros {
                                let secs = us as f64 / 1_000_000.0;
                                let p = (secs / total_dur_secs).clamp(0.0, 0.99) as f32;
                                *progress_state.export.progress.lock() = p;
                            }
                        }
                    })
                });

                let output = child.wait_with_output()?;
                if let Some(t) = reader { let _ = t.join(); }
                Ok(output)
            };

            // Try with CUDA hardware decoding first, fall back to software
            let cmd = build_cmd(true);
            log::info!("[Export] Running FFmpeg (hwaccel cuda): {:?}", cmd);
            let clip_output = match run(cmd) {
                Ok(out) if out.status.success() => Ok(out),
                _ => {
                    log::warn!("[Export] CUDA decode failed, retrying with software decoder");
                    *app_state_for_ffmpeg.export.progress.lock() = 0.0;
                    let cmd = build_cmd(false);
                    log::info!("[Export] Running FFmpeg (software): {:?}", cmd);
                    run(cmd)
                }
            };

            // Extract a thumbnail from the middle of the clip
            let thumb_time = (out_offset - in_offset) / 2.0;
            let mut thumb_path = output_path.clone();
            thumb_path.set_extension("jpg");

            if clip_output.as_ref().map_or(false, |o| o.status.success()) {
                let mut thumb_cmd = Command::new(&ffmpeg_path);
                thumb_cmd.creation_flags(0x08000000);
                thumb_cmd.arg("-y")
                         .arg("-ss").arg(format!("{:.3}", thumb_time))
                         .arg("-i").arg(&output_path)
                         .arg("-vframes").arg("1")
                         .arg("-q:v").arg("2")
                         .arg(&thumb_path);

                log::info!("[Export] Generating thumbnail: {:?}", thumb_cmd);
                if let Ok(out) = thumb_cmd.output() {
                    if !out.status.success() {
                        log::warn!("[Export] Thumbnail generation failed: {}", String::from_utf8_lossy(&out.stderr));
                    }
                }
            }

            let _ = std::fs::remove_file(&concat_path);

            (clip_output, output_path)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (result, output_path) = ffmpeg_task.await;

                let _ = this.update(&mut cx, |this, cx| {
                    self_reset_encoder(this);
                    *this.app_state.export.phase.lock() = crate::state::ExportPhase::Idle;

                    if let Some(any_window) = cx.windows().first() {
                        let _ = any_window.update(cx, |_, window, cx| {
                            match result {
                                Ok(output) => {
                                    if output.status.success() {
                                        this.clip_start = -1.0;
                                        this.clip_end = -1.0;
                                        this.clip_start_mark = None;
                                        this.clip_end_mark = None;
                                        this.export_result_path = Some(output_path.clone());
                                        this.export_result_size_mb = std::fs::metadata(&output_path)
                                            .ok()
                                            .map(|m| m.len() as f32 / 1024.0 / 1024.0);
                                        // Invalidate cached source stats so the
                                        // dashboard's clip count refreshes.
                                        this.app_state.source_stats.clear();
                                        this.export_stage = ExportStage::Done;
                                        crate::overlay::send(
                                            &this.app_state,
                                            crate::overlay::OverlayEvent::ClipSaved {
                                                title: source_name.clone(),
                                            },
                                        );
                                        this.show_toast(
                                            SharedString::from("Clip Saved"),
                                            Some(SharedString::from(format!("Exported to {:?}", output_path))),
                                            adabraka_ui::overlays::toast::ToastVariant::Success,
                                            window,
                                            cx,
                                        );
                                    } else {
                                        let err = String::from_utf8_lossy(&output.stderr);
                                        log::error!("[Export] FFmpeg failed: {}", err);
                                        let err_summary = err.lines().rev()
                                            .find(|l| !l.trim().is_empty())
                                            .unwrap_or("FFmpeg returned an error.")
                                            .to_string();
                                        this.export_stage = ExportStage::Configure;
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
                                    this.export_stage = ExportStage::Configure;
                                    this.show_toast(
                                        SharedString::from("FFmpeg Error"),
                                        Some(SharedString::from("Could not locate or run ffmpeg.exe")),
                                        adabraka_ui::overlays::toast::ToastVariant::Error,
                                        window,
                                        cx,
                                    );
                                }
                            }
                        });
                    }
                    cx.notify();
                });
            }
        }).detach();
    }

    // ── Modal ───────────────────────────────────────────────────────────
    pub fn render_export_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(720.0))
                    .max_h(px(820.0))
                    .bg(theme.tokens.card)
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(self.render_export_header(&theme, cx))
                    .child(match self.export_stage {
                        ExportStage::Configure => self.render_export_configure(cx).into_any_element(),
                        ExportStage::Exporting => self.render_export_progress(cx).into_any_element(),
                        ExportStage::Done => self.render_export_done(cx).into_any_element(),
                    }),
            )
    }

    fn render_export_header(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (label, icon) = match self.export_stage {
            ExportStage::Configure => ("Export clip", "scissors"),
            ExportStage::Exporting => ("Exporting…", "download"),
            ExportStage::Done => ("Clip exported", "check"),
        };
        HStack::new()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(theme.tokens.border)
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded_md()
                            .bg(theme.tokens.primary.opacity(0.15))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named(icon.into()))
                                    .size(px(16.0))
                                    .color(theme.tokens.primary.into()),
                            ),
                    )
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(label)),
            )
            .child(
                Button::new("export-close", "")
                    .icon(IconSource::Named("x".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this, _, _, cx| { this.close_export_modal(cx); })),
            )
    }

    // ── Stage: Configure ────────────────────────────────────────────────
    fn render_export_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("export-cfg-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .p_6()
                            .gap_6()
                            .child(self.render_export_preview())
                            .child(self.render_export_mode_cards(cx))
                            .when(self.export_reencode, |this| {
                                this.child(self.render_export_reencode_panel(cx))
                            })
                            .child(self.render_export_audio_panel(cx))
                            .child(self.render_export_destination(cx)),
                    ),
            )
            .child(self.render_export_footer(cx))
    }

    fn render_export_preview(&self) -> impl IntoElement {
        let theme = use_theme();
        let source = self.selected_source.clone().unwrap_or_else(|| "Monitor".to_string());
        let est = self.estimated_size_mb();
        HStack::new()
            .gap_4()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme.tokens.border)
            .bg(theme.tokens.muted.opacity(0.4))
            .child(
                div()
                    .w(px(200.0))
                    .h(px(112.0))
                    .rounded_md()
                    .bg(theme.tokens.background)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .relative()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named("video".into()))
                            .size(px(28.0))
                            .color(theme.tokens.muted_foreground.into()),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(6.0))
                            .right(px(6.0))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(gpui::rgba(0x000000CC))
                            .text_xs()
                            .text_color(gpui::rgba(0xFAFAFAFF))
                            .font_weight(FontWeight::MEDIUM)
                            .child(fmt_duration(self.export_clip_duration as f32)),
                    ),
            )
            .child(
                VStack::new()
                    .flex_1()
                    .gap_2()
                    .child(section_label("CLIP TITLE", &theme))
                    .child(Input::new(&self.export_title_input).placeholder("Clutch 1v4 on Mirage"))
                    .child(
                        HStack::new()
                            .pt_2()
                            .gap_6()
                            .child(kv("Source", &source, &theme))
                            .child(kv("Duration", &fmt_duration(self.export_clip_duration as f32), &theme))
                            .child(kv("Estimated size", &format!("{:.1} MB", est), &theme)),
                    ),
            )
    }

    fn render_export_mode_cards(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        VStack::new()
            .gap_2()
            .child(section_label("MODE", &theme))
            .child(
                HStack::new()
                    .gap_2()
                    .child(
                        Button::new("exp-mode-instant", "Instant copy")
                            .variant(if !self.export_reencode { ButtonVariant::Default } else { ButtonVariant::Outline })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| { this.export_reencode = false; cx.notify(); })),
                    )
                    .child(
                        Button::new("exp-mode-reencode", "Re-encode")
                            .variant(if self.export_reencode { ButtonVariant::Default } else { ButtonVariant::Outline })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| { this.export_reencode = true; cx.notify(); })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child(if self.export_reencode {
                        "Re-encodes the clip. Choose codec, quality and container."
                    } else {
                        "Lossless stream copy. Saves in under a second."
                    }),
            )
    }

    fn render_export_reencode_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        VStack::new()
            .gap_5()
            .p_5()
            .rounded_lg()
            .bg(theme.tokens.muted.opacity(0.4))
            .border_1()
            .border_color(theme.tokens.border)
            .child(
                VStack::new()
                    .gap_2()
                    .child(section_label("ENCODER", &theme))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.enc_chip("HEVC", "hevc_nvenc", cx))
                            .child(self.enc_chip("AV1", "av1_nvenc", cx))
                            .child(self.enc_chip("H.264", "h264_nvenc", cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(encoder_detail(&self.export_encoder)),
                    ),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_end()
                            .child(section_label("QUALITY", &theme))
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("exp-bit-dec", "-")
                                            .variant(ButtonVariant::Outline)
                                            .size(ButtonSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate - 1000).max(1000); cx.notify(); })),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(72.0))
                                            .text_center()
                                            .text_xs()
                                            .text_color(theme.tokens.muted_foreground)
                                            .child(format!("{} kbps", self.export_bitrate)),
                                    )
                                    .child(
                                        Button::new("exp-bit-inc", "+")
                                            .variant(ButtonVariant::Outline)
                                            .size(ButtonSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate + 1000).min(100000); cx.notify(); })),
                                    ),
                            ),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.qty_chip("Smaller", 8000, cx))
                            .child(self.qty_chip("Balanced", 20000, cx))
                            .child(self.qty_chip("Max quality", 50000, cx)),
                    ),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .child(section_label("CONTAINER", &theme))
                    .child(
                        HStack::new()
                            .gap_2()
                            .child(self.container_chip("mp4", cx))
                            .child(self.container_chip("mov", cx))
                            .child(self.container_chip("mkv", cx)),
                    ),
            )
    }

    fn enc_chip(&self, label: &'static str, value: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let active = self.export_encoder == value;
        div()
            .id(SharedString::from(format!("exp-enc-{}", value)))
            .px_4()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
            .bg(if active { theme.tokens.primary.opacity(0.15) } else { theme.tokens.card })
            .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
            .font_weight(FontWeight::SEMIBOLD)
            .text_sm()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.export_encoder = value.to_string();
                cx.notify();
            }))
            .child(label)
    }

    fn qty_chip(&self, label: &'static str, bitrate: i32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let active = self.export_bitrate == bitrate;
        div()
            .id(SharedString::from(format!("exp-q-{}", bitrate)))
            .flex_1()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
            .bg(if active { theme.tokens.primary.opacity(0.15) } else { theme.tokens.card })
            .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
            .text_center()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
            .text_sm()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.export_bitrate = bitrate;
                cx.notify();
            }))
            .child(label)
    }

    fn container_chip(&self, ext: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let active = self.export_container == ext;
        div()
            .id(SharedString::from(format!("exp-ct-{}", ext)))
            .px_3()
            .py_1()
            .rounded_sm()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .bg(if active { theme.tokens.primary } else { theme.tokens.card })
            .text_color(if active { theme.tokens.primary_foreground } else { theme.tokens.muted_foreground })
            .border_1()
            .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.export_container = ext.to_string();
                cx.notify();
            }))
            .child(ext.to_uppercase())
    }

    fn render_export_audio_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        if self.export_audio_tracks.is_empty() {
            return VStack::new()
                .gap_2()
                .child(section_label("AUDIO TRACKS", &theme))
                .child(div().text_xs().text_color(theme.tokens.muted_foreground).child("No audio tracks configured for this source."));
        }
        let mut row = HStack::new().gap_6().flex_wrap();
        for (idx, track) in self.export_audio_tracks.iter().enumerate() {
            row = row.child(self.audio_toggle(idx, track, cx));
        }
        VStack::new()
            .gap_2()
            .child(section_label("AUDIO TRACKS", &theme))
            .child(row)
    }

    fn audio_toggle(&self, idx: usize, track: &crate::config::AudioRouting, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let enabled = track.enabled;
        let icon = crate::ui::audio_track_icon(&track.source_type);
        let name = crate::ui::audio_track_display_name(track);
        let detail = track.device_name.clone();
        div()
            .id(SharedString::from(format!("exp-at-{}", idx)))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .py_1()
            .child(crate::ui::toggle_switch(
                &theme,
                cx,
                SharedString::from(format!("exp-at-tg-{}", idx)),
                enabled,
                true,
                move |this| {
                    if let Some(t) = this.export_audio_tracks.get_mut(idx) {
                        t.enabled = !t.enabled;
                    }
                },
            ))
            .child(
                Icon::new(IconSource::Named(icon.into()))
                    .size(px(14.0))
                    .color(if enabled { theme.tokens.muted_foreground.into() } else { theme.tokens.border.into() }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if enabled { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                    .child(name),
            )
            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(detail))
    }

    fn render_export_destination(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let dest = self.export_destination.to_string_lossy().to_string();
        VStack::new()
            .gap_2()
            .child(section_label("DESTINATION", &theme))
            .child(
                HStack::new()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .bg(theme.tokens.muted.opacity(0.4))
                    .border_1()
                    .border_color(theme.tokens.border)
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("folder".into()))
                            .size(px(16.0))
                            .color(theme.tokens.muted_foreground.into()),
                    )
                    .child(div().flex_1().text_sm().text_color(theme.tokens.muted_foreground).child(dest))
                    .child(
                        Button::new("exp-pick-dest", "Change")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| { this.pick_export_destination(cx); })),
                    ),
            )
    }

    fn pick_export_destination(&self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        cx.spawn(|_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if let Some(path) = rfd::AsyncFileDialog::new()
                    .set_title("Choose export folder")
                    .pick_folder()
                    .await
                {
                    let p = path.path().to_path_buf();
                    let _ = view.update(&mut cx, |this, cx| {
                        this.export_destination = p;
                        cx.notify();
                    });
                }
            }
        }).detach();
    }

    fn render_export_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        HStack::new()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(theme.tokens.border)
            .items_center()
            .justify_between()
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("hard-drive".into()))
                            .size(px(14.0))
                            .color(theme.tokens.muted_foreground.into()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!(
                                "{} · {:.1} MB estimated",
                                fmt_duration(self.export_clip_duration as f32),
                                self.estimated_size_mb(),
                            )),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(
                        Button::new("exp-cancel", "Cancel")
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| { this.close_export_modal(cx); })),
                    )
                    .child(
                        Button::new("exp-start", "Export clip")
                            .icon(IconSource::Named("download".into()))
                            .on_click(cx.listener(|this, _, window, cx| { this.perform_export(window, cx); })),
                    ),
            )
    }

    // ── Stage: Exporting ────────────────────────────────────────────────
    fn render_export_progress(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let progress = *self.app_state.export.progress.lock();
        let pct = (progress * 100.0).round() as i32;
        VStack::new()
            .flex_1()
            .p_8()
            .gap_6()
            .items_center()
            .child(
                div()
                    .pt_4()
                    .child(
                        div()
                            .size(px(72.0))
                            .rounded_full()
                            .bg(theme.tokens.primary.opacity(0.15))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Spinner::new().size(SpinnerSize::Xl)),
                    ),
            )
            .child(
                VStack::new()
                    .gap_1()
                    .items_center()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Rendering your clip"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(if self.export_reencode {
                                "Re-encoding with NVENC. This can take a moment…"
                            } else {
                                "Stream-copying segments. Just a moment…"
                            }),
                    ),
            )
            .child(
                VStack::new()
                    .w(px(480.0))
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .h(px(10.0))
                            .rounded_full()
                            .bg(theme.tokens.muted)
                            .border_1()
                            .border_color(theme.tokens.border)
                            .overflow_hidden()
                            .child(div().h_full().w(relative(progress)).bg(theme.tokens.primary).rounded_full()),
                    )
                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(format!("{}%", pct))),
            )
            .child(
                div()
                    .pt_4()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child("You can close this — the export keeps running in the background."),
            )
    }

    // ── Stage: Done ──────────────────────────────────────────────────────
    fn render_export_done(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let path = self.export_result_path.clone();
        let filename = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "clip".to_string());
        let folder = path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        VStack::new()
            .flex_1()
            .p_8()
            .gap_6()
            .items_center()
            .child(
                div()
                    .size(px(72.0))
                    .rounded_full()
                    .bg(gpui::rgba(SUCCESS_DIM))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named("check".into()))
                            .size(px(32.0))
                            .color(gpui::rgba(SUCCESS).into()),
                    ),
            )
            .child(
                VStack::new()
                    .gap_1()
                    .items_center()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Clip saved"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("{} · {:.1} MB", fmt_duration(self.export_clip_duration as f32), self.export_result_size_mb.unwrap_or_else(|| self.estimated_size_mb()))),
                    ),
            )
            .child(
                div()
                    .w(px(520.0))
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .bg(theme.tokens.muted.opacity(0.4))
                    .border_1()
                    .border_color(theme.tokens.border)
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                Icon::new(IconSource::Named("video".into()))
                                    .size(px(18.0))
                                    .color(theme.tokens.muted_foreground.into()),
                            )
                            .child(
                                VStack::new()
                                    .flex_1()
                                    .gap_0p5()
                                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(filename))
                                    .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(folder)),
                            ),
                    ),
            )
            .child(
                HStack::new()
                    .pt_2()
                    .gap_3()
                    .child(
                        Button::new("exp-done-reveal", "Show in folder")
                            .icon(IconSource::Named("folder".into()))
                            .variant(ButtonVariant::Outline)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(p) = this.export_result_path.clone() {
                                    let _ = std::process::Command::new("explorer")
                                        .arg("/select,")
                                        .arg(&p)
                                        .spawn();
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("exp-done-close", "Done")
                            .on_click(cx.listener(|this, _, _, cx| { this.close_export_modal(cx); })),
                    ),
            )
    }

    fn estimated_size_mb(&self) -> f32 {
        let kbps = if self.export_reencode {
            self.export_bitrate as f32
        } else {
            // Approximate source bitrate from the configured recording bitrate.
            AppConfig::load().global_video.bitrate_kbps as f32
        };
        self.export_clip_duration as f32 * kbps / 8000.0
    }
}

// Reset re-encode config back to defaults after an export attempt completes.
fn self_reset_encoder(this: &mut RekaptrWorkspace) {
    this.export_reencode = false;
    this.export_encoder = "h264_nvenc".to_string();
    this.export_bitrate = 50000;
    this.export_preset = "p4".to_string();
    this.export_container = "mp4".to_string();
}

fn section_label(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.tokens.muted_foreground)
        .child(text.to_string())
}

fn kv(label: &str, value: &str, theme: &Theme) -> impl IntoElement {
    VStack::new()
        .gap_0p5()
        .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(label.to_string()))
        .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child(value.to_string()))
}

fn encoder_detail(encoder: &str) -> &'static str {
    match encoder {
        "hevc_nvenc" => "Best quality per bit. Plays on most devices.",
        "av1_nvenc" => "Smallest files. Newer players only.",
        _ => "Maximum compatibility. Larger files.",
    }
}

fn fmt_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}
