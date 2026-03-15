//! Clip export via FFmpeg.
//!
//! Luma supports two export modes:
//!
//! - **Instant copy** (`-c:v copy`): Copies the H.264/HEVC bitstream directly from the source
//!   segments without re-encoding. Sub-second export times regardless of clip length, but the
//!   output inherits the original recording's codec and quality settings.
//!
//! - **Re-encode**: Full GPU-accelerated transcode through NVENC. Lets users choose a different
//!   codec (H.264/HEVC/AV1), bitrate, and quality preset. Slower but produces a clean,
//!   standards-compliant MP4.
//!
//! ## Why ffconcat instead of HLS
//! The recording segments are stored as fragmented MP4 files on disk. For playback we serve
//! them over a local HLS server (mpv needs HTTP URLs). But for export, we use FFmpeg's
//! `ffconcat` demuxer instead — it reads files directly from disk paths without any HTTP
//! overhead, and gives FFmpeg random-access seeking into the segment chain. This makes
//! `-ss`/`-to` trimming fast and accurate.
//!
//! ## Thumbnail extraction
//! After exporting, we extract a JPEG thumbnail from the clip's temporal midpoint. This
//! thumbnail is used in the clip library grid view for visual browsing.

use crate::config::AppConfig;
use crate::ui::LumaWorkspace;
use adabraka_ui::prelude::*;
use gpui::*;

impl LumaWorkspace {
    /// Opens the export modal if in/out markers are set.
    pub fn save_clip(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.clip_start >= 0.0 && self.clip_end >= 0.0 {
            self.show_export_modal = true;
            cx.notify();
        }
    }

    /// Kicks off the FFmpeg export process on a background thread.
    ///
    /// Regenerates the ffconcat playlist first to ensure it includes the latest segments,
    /// then spawns FFmpeg with either `-c:v copy` (instant) or a full NVENC transcode.
    /// Audio is always re-encoded to AAC 320kbps for maximum compatibility.
    /// A progress simulation runs in parallel to drive the UI progress bar.
    pub fn perform_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.clip_start;
        let end = self.clip_end;
        let source_name = self
            .selected_source
            .clone()
            .unwrap_or_else(|| "monitor".to_string());

        // Ensure ffconcat is up to date before exporting
        crate::utils::generate_ffconcat_playlist(&source_name);

        let safe_title = crate::utils::clean_title(&source_name);
        let storage_root = crate::utils::get_storage_root();
        let playlist_path = storage_root.join(&safe_title).join("view.ffconcat");

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

        // Start export
        self.app_state.export_running.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.app_state.export_progress.lock() = 0.0;

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

        // Spawn a UI-thread poller to notify the view of progress changes.
        // This avoids capturing AsyncApp (which is not Send) in the background task.
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                while app_state_for_progress.export_running.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = view_handle.update(&mut cx, |_, cx| cx.notify());
                    let _ = cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                }
            }
        }).detach();

        let app_state_for_ffmpeg = self.app_state.clone();
        let ffmpeg_task = cx.background_spawn(async move {
            use std::process::{Command, Stdio};
            use std::io::{BufRead, BufReader};

            let total_duration = end - start;

            let mut cmd = Command::new(ffmpeg_path.clone());
            cmd.arg("-y")
               .arg("-hwaccel").arg("cuda")
               .arg("-hwaccel_output_format").arg("cuda")
               .arg("-ss").arg(format!("{:.3}", start))
               .arg("-to").arg(format!("{:.3}", end))
               .arg("-f").arg("concat")
               .arg("-safe").arg("0")
               .arg("-i").arg(playlist_path.clone())
               .arg("-map").arg("0:v:0");

            let mut physical_stream_idx = 0;
            for track in audio_tracks {
                if track.enabled {
                    cmd.arg("-map")
                        .arg(format!("0:a:{}?", physical_stream_idx));
                }
                physical_stream_idx += 1;
            }

            if export_reencode {
                cmd.arg("-c:v")
                    .arg(&encoder)
                    .arg("-preset")
                    .arg(&preset)
                    .arg("-b:v")
                    .arg(format!("{}K", bitrate))
                    .arg("-rc")
                    .arg("vbr");
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

            cmd.stderr(Stdio::piped());
            cmd.stdout(Stdio::null());

            eprintln!("[UI] Running FFmpeg for clip: {:?}", cmd);
            
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => return (Err(e.into()), output_path, clips_dir),
            };

            let stderr = child.stderr.take().unwrap();
            let mut reader = BufReader::new(stderr);
            let mut last_error_lines = Vec::new();

            // FFmpeg uses \r for progress updates to overwrite the same line.
            // BufReader::lines() only splits on \n, so we manually read byte-by-byte or similar.
            let mut buf = Vec::new();
            use std::io::Read;
            let mut byte_buf = [0u8; 1];
            while reader.read_exact(&mut byte_buf).is_ok() {
                let b = byte_buf[0];
                if b == b'\n' || b == b'\r' {
                    if buf.is_empty() { continue; }
                    let line = String::from_utf8_lossy(&buf).to_string();
                    buf.clear();

                    // Parse progress: time=00:00:05.12
                    if let Some(time_pos) = line.find("time=") {
                        let time_part = &line[time_pos + 5..];
                        // Get the timestamp part (until next space or end of string)
                        let timestamp = time_part.split_whitespace().next().unwrap_or("");
                        if let Some(secs) = parse_ffmpeg_timestamp(timestamp) {
                            let progress = (secs / total_duration).min(1.0) as f32;
                            *app_state_for_ffmpeg.export_progress.lock() = progress;
                        }
                    }
                    
                    // Buffer stderr for error reporting
                    last_error_lines.push(line);
                    if last_error_lines.len() > 10 {
                        last_error_lines.remove(0);
                    }
                } else {
                    buf.push(b);
                    if buf.len() > 2048 { buf.clear(); } // Safety
                }
            }

            let status = child.wait();
            let result = match status {
                Ok(s) if s.success() => Ok(last_error_lines.join("\n")),
                Ok(s) => Err(anyhow::anyhow!("FFmpeg failed with status {}: \n{}", s, last_error_lines.join("\n"))),
                Err(e) => Err(e.into()),
            };

            // Extract a thumbnail from the middle of the clip (if export succeeded)
            if result.is_ok() {
                let duration = end - start;
                let thumb_time = start + (duration / 2.0);
                let mut thumb_path = output_path.clone();
                thumb_path.set_extension("jpg");

                let mut thumb_cmd = Command::new(&ffmpeg_path);
                thumb_cmd.arg("-y")
                         .arg("-ss").arg(format!("{:.3}", thumb_time))
                         .arg("-f").arg("concat")
                         .arg("-safe").arg("0")
                         .arg("-i").arg(&playlist_path)
                         .arg("-vframes").arg("1")
                         .arg("-q:v").arg("2")
                         .arg(&thumb_path);

                eprintln!("[UI] Running FFmpeg for thumbnail: {:?}", thumb_cmd);
                let _ = thumb_cmd.output();
            }

            (result, output_path, clips_dir)
        });

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (result, output_path, clips_dir) = ffmpeg_task.await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.show_export_modal = false;
                    this.app_state.export_running.store(false, std::sync::atomic::Ordering::SeqCst);

                    if let Some(any_window) = cx.windows().first() {
                        let _ = any_window.update(cx, |_, window, cx| {
                            match result {
                                Ok(_) => {
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
                                    crate::tray::show_notification("Clip Saved", &format!("Exported to {}", output_path.display()));
                                    let _ = std::process::Command::new("explorer")
                                        .arg(&clips_dir)
                                        .spawn();
                                }
                                Err(e) => {
                                    eprintln!("[UI] Export failed: {}", e);
                                    let error_msg = e.to_string();
                                    let display_error = if error_msg.len() > 100 {
                                        format!("{}...", &error_msg[..100])
                                    } else {
                                        error_msg
                                    };
                                    
                                    this.show_toast(
                                        SharedString::from("Export Failed"),
                                        Some(SharedString::from(display_error)),
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

    pub fn render_export_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                this.child(
                                    VStack::new()
                                        .gap_4()
                                        .p_4()
                                        .bg(theme.tokens.muted.opacity(0.5))
                                        .rounded_md()
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("ENCODER"))
                                                .child(
                                                    HStack::new()
                                                        .gap_2()
                                                        .child(
                                                            Button::new("exp-enc-h264", "H.264")
                                                                .variant(if self.export_encoder == "h264_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "h264_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-enc-hevc", "HEVC")
                                                                .variant(if self.export_encoder == "hevc_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "hevc_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-enc-av1", "AV1")
                                                                .variant(if self.export_encoder == "av1_nvenc" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_encoder = "av1_nvenc".to_string(); cx.notify(); }))
                                                        )
                                                )
                                        )
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("QUALITY PRESET"))
                                                .child(
                                                    HStack::new()
                                                        .gap_2()
                                                        .child(
                                                            Button::new("exp-pre-fast", "Fast")
                                                                .variant(if self.export_preset == "p1" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p1".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-pre-bal", "Balanced")
                                                                .variant(if self.export_preset == "p4" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p4".to_string(); cx.notify(); }))
                                                        )
                                                        .child(
                                                            Button::new("exp-pre-hq", "High Quality")
                                                                .variant(if self.export_preset == "p7" { ButtonVariant::Default } else { ButtonVariant::Outline })
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_preset = "p7".to_string(); cx.notify(); }))
                                                        )
                                                )
                                        )
                                        .child(
                                            VStack::new()
                                                .gap_2()
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.muted_foreground).child("BITRATE (kbps)"))
                                                .child(
                                                    HStack::new()
                                                        .gap_4()
                                                        .items_center()
                                                        .child(
                                                            Button::new("exp-bit-dec", "-")
                                                                .variant(ButtonVariant::Outline)
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate - 5000).max(1000); cx.notify(); }))
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .p_2()
                                                                .bg(theme.tokens.background)
                                                                .rounded_md()
                                                                .child(div().text_center().font_weight(FontWeight::BOLD).child(format!("{}k", self.export_bitrate)))
                                                        )
                                                        .child(
                                                            Button::new("exp-bit-inc", "+")
                                                                .variant(ButtonVariant::Outline)
                                                                .size(ButtonSize::Sm)
                                                                .on_click(cx.listener(|this, _, _, cx| { this.export_bitrate = (this.export_bitrate + 5000).min(100000); cx.notify(); }))
                                                        )
                                                )
                                        )
                                )
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
                                let export_running = self.app_state.export_running.load(std::sync::atomic::Ordering::SeqCst);
                                let progress = *self.app_state.export_progress.lock();

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
                                            .child(
                                                Spinner::new()
                                                    .size(SpinnerSize::Xl)
                                            )
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
}

fn parse_ffmpeg_timestamp(ts: &str) -> Option<f64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 { return None; }
    
    let hours: f64 = parts[0].parse().ok()?;
    let mins: f64 = parts[1].parse().ok()?;
    let secs: f64 = parts[2].parse().ok()?;
    
    Some(hours * 3600.0 + mins * 60.0 + secs)
}
