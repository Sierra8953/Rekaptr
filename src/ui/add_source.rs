use gpui::*;
use adabraka_ui::prelude::*;
use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::components::input::Input;
use crate::ui::RekaptrWorkspace;
use crate::state::GameSession;
use crate::config::{AudioRouting, GameSettings};

// ── Theme constants (kept local; theme tokens cover most of this elsewhere)
const PRIMARY_DIM_A25: u32 = 0x5B3FA840;

impl RekaptrWorkspace {
    pub fn render_add_source_modal(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .id("add-source-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                this.close_add_source_modal(cx);
            }))
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("add-source-container")
                    .w(px(720.0))
                    .max_h(relative(0.92))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(self.render_modal_header(&theme, cx))
                    .child(
                        div()
                            .id("add-src-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(
                                VStack::new()
                                    .p_6()
                                    .gap_6()
                                    .child(self.render_source_section(&theme, cx))
                                    .child(self.render_details_section(&theme, cx))
                                    .child(self.render_settings_section(&theme, cx))
                                    .child(self.render_audio_section(&theme, cx))
                                    .child(self.render_auto_record(&theme, cx)),
                            ),
                    )
                    .child(self.render_modal_footer(&theme, cx))
            )
    }

    fn render_modal_header(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .bg(rgba(PRIMARY_DIM_A25))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named("plus".into()))
                                    .size(px(16.0))
                                    .color(theme.tokens.primary.into()),
                            ),
                    )
                    .child(
                        VStack::new()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child("Add game capture"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("Pick a window and we'll do the rest."),
                            ),
                    ),
            )
            .child(
                Button::new("modal-close-x", "")
                    .icon(IconSource::Named("x".into()))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                        this.close_add_source_modal(cx);
                    })),
            )
    }

    // ── Section 1: Source picker ────────────────────────────────────
    fn render_source_section(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.add_source_search_input.read(cx).content().to_lowercase();
        let windows = self.app_state.available_windows.lock().clone();
        let filtered: Vec<_> = windows
            .into_iter()
            .filter(|w| q.is_empty()
                || w.title.to_lowercase().contains(&q)
                || w.process_name.to_lowercase().contains(&q))
            .collect();

        let list_body: AnyElement = if self.is_refreshing_windows {
            div()
                .h(px(220.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.tokens.muted_foreground)
                .child("Scanning for windows...")
                .into_any_element()
        } else if filtered.is_empty() {
            div()
                .h(px(220.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.tokens.muted_foreground)
                .child("No windows match your search.")
                .into_any_element()
        } else {
            div()
                .id("win-list")
                .w_full()
                .h(px(220.0))
                .overflow_y_scroll()
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    VStack::new()
                        .p_1()
                        .gap_0p5()
                        .children(filtered.iter().map(|w| {
                            self.render_window_row(theme, w.hwnd, &w.title, &w.process_name, cx)
                        })),
                )
                .into_any_element()
        };

        VStack::new()
            .gap_2()
            .child(section_label(theme, "SOURCE"))
            .child(
                HStack::new()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(
                        Input::new(&self.add_source_search_input).placeholder("Search windows..."),
                    ))
                    .child(
                        Button::new("refresh-wins", "")
                            .icon(IconSource::Named("rotate-cw".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.refresh_available_windows(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.background)
                    .child(list_body),
            )
    }

    fn render_window_row(
        &self,
        theme: &Theme,
        hwnd: u64,
        title: &str,
        process: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.form_hwnd == Some(hwnd);
        let title_owned = title.to_string();
        let process_owned = process.to_string();
        let title_for_click = title_owned.clone();
        let process_for_click = process_owned.clone();

        div()
            .id(("win-row", hwnd as usize))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .bg(if selected { rgba(PRIMARY_DIM_A25).into() } else { theme.tokens.card })
            .hover(|s| s.bg(theme.tokens.muted))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.form_hwnd = Some(hwnd);
                    this.form_title = title_for_click.clone();
                    this.form_target_process = Some(process_for_click.clone());
                    let new_title = title_for_click.clone();
                    this.add_source_title_input.update(cx, |input, cx| {
                        input.set_value(SharedString::from(new_title), window, cx);
                    });
                    cx.notify();
                }),
            )
            // Window glyph
            .child(
                div()
                    .size(px(28.0))
                    .rounded_sm()
                    .bg(theme.tokens.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named("layout-dashboard".into()))
                            .size(px(14.0))
                            .color(theme.tokens.muted_foreground.into()),
                    ),
            )
            .child(
                VStack::new()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child(title_owned),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(process_owned),
                    ),
            )
            .when(selected, |this| {
                this.child(
                    Icon::new(IconSource::Named("check".into()))
                        .size(px(16.0))
                        .color(theme.tokens.primary.into()),
                )
            })
    }

    // ── Section 2: Details ──────────────────────────────────────────
    fn render_details_section(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        if self.form_hwnd.is_none() {
            return VStack::new()
                .gap_2()
                .child(section_label(theme, "DETAILS"))
                .child(
                    div()
                        .p_6()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.tokens.border)
                        .bg(theme.tokens.background)
                        .text_center()
                        .text_sm()
                        .text_color(theme.tokens.muted_foreground)
                        .child("Select a window above to preview."),
                )
                .into_any_element();
        }

        let process_label = self.form_target_process.clone().unwrap_or_default();

        VStack::new()
            .gap_2()
            .child(section_label(theme, "DETAILS"))
            .child(
                HStack::new()
                    .gap_4()
                    .items_center()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.background)
                    .child(
                        div()
                            .size(px(56.0))
                            .rounded_md()
                            .bg(theme.tokens.muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named("gamepad-2".into()))
                                    .size(px(24.0))
                                    .color(theme.tokens.muted_foreground.into()),
                            ),
                    )
                    .child(
                        VStack::new()
                            .flex_1()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("TITLE"),
                            )
                            .child(Input::new(&self.add_source_title_input).placeholder("Title"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(process_label),
                            ),
                    ),
            )
            .into_any_element()
    }

    // ── Section 3: Settings ─────────────────────────────────────────
    fn render_settings_section(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = self.settings_summary();

        let mut body = VStack::new()
            .gap_2()
            .child(
                HStack::new()
                    .items_center()
                    .child(section_label(theme, "SETTINGS"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(if self.add_source_show_overrides { "Overriding globals" } else { "Inheriting globals" }),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.background)
                    .child(
                        Icon::new(IconSource::Named(
                            if self.add_source_show_overrides { "sliders-horizontal" } else { "check-circle" }.into(),
                        ))
                        .size(px(16.0))
                        .color(theme.tokens.muted_foreground.into()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child(summary),
                    )
                    .child(
                        Button::new(
                            "toggle-override",
                            if self.add_source_show_overrides { "Use defaults" } else { "Override defaults" },
                        )
                        .variant(if self.add_source_show_overrides { ButtonVariant::Ghost } else { ButtonVariant::Outline })
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.add_source_show_overrides = !this.add_source_show_overrides;
                            cx.notify();
                        })),
                    ),
            );

        if self.add_source_show_overrides {
            body = body.child(self.render_override_form(theme, cx));
        }
        body
    }

    fn settings_summary(&self) -> String {
        let res = self.form_resolution.replace('x', "×");
        let quality = if self.form_rate_control == 0 {
            format!("CQ {}", self.form_cq)
        } else {
            format!("{} kbps", self.form_bitrate)
        };
        let encoder = match self.form_encoder.as_str() {
            "nvh265enc" => "HEVC",
            "nvh264enc" => "H.264",
            "nvav1enc" => "AV1",
            other => other,
        };
        format!(
            "{} · {} fps · {} · {} · {} min",
            res, self.form_fps, encoder, quality, self.form_retention
        )
    }

    fn render_override_form(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let enc_btn = |id: &'static str, label: &'static str, value: &'static str, current: &str, cx: &mut Context<Self>| {
            let value_owned = value.to_string();
            Button::new(id, label)
                .variant(if current == value { ButtonVariant::Default } else { ButtonVariant::Outline })
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    this.form_encoder = value_owned.clone();
                    if this.form_encoder != "nvav1enc" {
                        this.form_cq = this.form_cq.min(51);
                    }
                    cx.notify();
                }))
        };
        let res_btn = |id: &'static str, label: &'static str, value: &'static str, current: &str, cx: &mut Context<Self>| {
            let value_owned = value.to_string();
            Button::new(id, label)
                .variant(if current == value { ButtonVariant::Default } else { ButtonVariant::Outline })
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    this.form_resolution = value_owned.clone();
                    cx.notify();
                }))
        };
        let fps_btn = |id: &'static str, value: i32, current: i32, cx: &mut Context<Self>| {
            Button::new(id, format!("{}", value))
                .variant(if current == value { ButtonVariant::Default } else { ButtonVariant::Outline })
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    this.form_fps = value;
                    cx.notify();
                }))
        };

        VStack::new()
            .gap_4()
            .p_4()
            .rounded_lg()
            .bg(theme.tokens.background)
            .border_1()
            .border_color(theme.tokens.border)
            .child(field_row(theme, "Encoder",
                HStack::new().gap_2()
                    .child(enc_btn("enc-hevc", "HEVC", "nvh265enc", &self.form_encoder, cx))
                    .child(enc_btn("enc-av1",  "AV1",  "nvav1enc",  &self.form_encoder, cx))
                    .child(enc_btn("enc-h264", "H.264","nvh264enc", &self.form_encoder, cx))
                    .into_any_element()
            ))
            .child(field_row(theme, "Resolution",
                HStack::new().gap_2()
                    .child(res_btn("res-4k",    "4K",    "3840x2160", &self.form_resolution, cx))
                    .child(res_btn("res-1440p", "1440p", "2560x1440", &self.form_resolution, cx))
                    .child(res_btn("res-1080p", "1080p", "1920x1080", &self.form_resolution, cx))
                    .child(res_btn("res-720p",  "720p",  "1280x720",  &self.form_resolution, cx))
                    .into_any_element()
            ))
            .child(field_row(theme, "Frame rate",
                HStack::new().gap_2()
                    .child(fps_btn("fps-30",  30,  self.form_fps, cx))
                    .child(fps_btn("fps-60",  60,  self.form_fps, cx))
                    .child(fps_btn("fps-120", 120, self.form_fps, cx))
                    .into_any_element()
            ))
            .child(field_row(theme, "Rate control",
                HStack::new().gap_2()
                    .child(
                        Button::new("rc-cqp", "CQP")
                            .variant(if self.form_rate_control == 0 { ButtonVariant::Default } else { ButtonVariant::Outline })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.form_rate_control = 0;
                                cx.notify();
                            }))
                    )
                    .child(
                        Button::new("rc-vbr", "VBR")
                            .variant(if self.form_rate_control == 1 { ButtonVariant::Default } else { ButtonVariant::Outline })
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.form_rate_control = 1;
                                cx.notify();
                            }))
                    )
                    .into_any_element()
            ))
            .child({
                let (label, value, suffix, min, max, step) = if self.form_rate_control == 0 {
                    ("Quality (CQ)", self.form_cq, "", 0, 51, 1)
                } else {
                    ("Bitrate", self.form_bitrate, "kbps", 1000, 100_000, 1000)
                };
                field_row(theme, label, stepper_inline(theme, "qty", value, min, max, step, suffix, cx, |this, v| {
                    if this.form_rate_control == 0 { this.form_cq = v; } else { this.form_bitrate = v; }
                }))
            })
            .child(field_row(theme, "Retention",
                stepper_inline(theme, "ret", self.form_retention, 1, 600, 1, "min", cx, |this, v| {
                    this.form_retention = v;
                })
            ))
    }

    // ── Section 4: Audio tracks (always visible) ────────────────────
    fn render_audio_section(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        // If a track is being configured for app routing, show the picker instead.
        if let Some(track_idx) = self.form_editing_track_index {
            return self.render_audio_app_picker(theme, track_idx, cx);
        }

        let active_count = self.form_audio_tracks.iter().filter(|t| t.enabled).count();
        let total = self.form_audio_tracks.len();

        let mut list = VStack::new()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(theme.tokens.background)
            .border_1()
            .border_color(theme.tokens.border);

        for (i, track) in self.form_audio_tracks.iter().enumerate() {
            list = list.child(self.render_audio_track_row(theme, i, track, cx));
        }

        VStack::new()
            .gap_2()
            .child(
                HStack::new()
                    .items_center()
                    .child(section_label(theme, "AUDIO TRACKS"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("{} of {} active", active_count, total)),
                    ),
            )
            .child(list)
            .into_any_element()
    }

    fn render_audio_app_picker(&self, theme: &Theme, track_idx: usize, cx: &mut Context<Self>) -> AnyElement {
        let track_name = self.form_audio_tracks[track_idx].name.clone();
        let windows = self.app_state.available_windows.lock().clone();

        VStack::new()
            .gap_2()
            .child(
                HStack::new()
                    .items_center()
                    .child(section_label(theme, "AUDIO TRACKS"))
                    .child(div().flex_1())
                    .child(
                        Button::new("audio-back", "Back")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.form_editing_track_index = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                VStack::new()
                    .gap_3()
                    .p_3()
                    .rounded_lg()
                    .bg(theme.tokens.background)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(format!("Select apps for {}", track_name)),
                    )
                    .child(
                        div()
                            .id("audio-app-list")
                            .max_h(px(220.0))
                            .overflow_y_scroll()
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                            .child(if windows.is_empty() {
                                div()
                                    .py_4()
                                    .text_sm()
                                    .text_color(theme.tokens.muted_foreground)
                                    .text_center()
                                    .child("No windows found. Try refreshing.")
                                    .into_any_element()
                            } else {
                                VStack::new()
                                    .gap_1()
                                    .children(windows.iter().map(|win| {
                                        let proc_name = win.process_name.clone();
                                        let is_selected = self.form_audio_tracks[track_idx].app_targets.contains(&proc_name);
                                        let proc_for_click = proc_name.clone();
                                        HStack::new()
                                            .justify_between()
                                            .items_center()
                                            .px_2()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(if is_selected { rgba(PRIMARY_DIM_A25).into() } else { theme.tokens.card })
                                            .child(
                                                VStack::new()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.tokens.foreground)
                                                            .child(win.title.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.tokens.muted_foreground)
                                                            .child(proc_name.clone()),
                                                    ),
                                            )
                                            .child(
                                                Button::new(
                                                    SharedString::from(format!("app-sel-{}-{}", track_idx, proc_name)),
                                                    if is_selected { "Remove" } else { "Add" },
                                                )
                                                .variant(if is_selected { ButtonVariant::Destructive } else { ButtonVariant::Outline })
                                                .size(ButtonSize::Sm)
                                                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                                                    if is_selected {
                                                        this.form_audio_tracks[track_idx].app_targets.retain(|t| t != &proc_for_click);
                                                    } else {
                                                        this.form_audio_tracks[track_idx].app_targets.push(proc_for_click.clone());
                                                    }
                                                    cx.notify();
                                                }))
                                            )
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_audio_track_row(
        &self,
        theme: &Theme,
        idx: usize,
        track: &AudioRouting,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = track.enabled;
        let track_name = track.name.clone();
        let source_type = track.source_type.clone();
        let device = track.device_name.clone();
        let app_count = track.app_targets.len();

        HStack::new()
            .gap_3()
            .items_center()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            // Toggle
            .child(
                div()
                    .id(SharedString::from(format!("at-en-{}", idx)))
                    .w(px(28.0))
                    .h(px(16.0))
                    .rounded_full()
                    .relative()
                    .cursor_pointer()
                    .bg(if enabled { theme.tokens.primary } else { theme.tokens.border })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, _, cx| {
                        this.form_audio_tracks[idx].enabled = !this.form_audio_tracks[idx].enabled;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .left(if enabled { px(14.0) } else { px(2.0) })
                            .size(px(12.0))
                            .rounded_full()
                            .bg(theme.tokens.foreground),
                    ),
            )
            .child(
                div()
                    .w(px(72.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if enabled { theme.tokens.foreground } else { theme.tokens.muted_foreground })
                    .child(track_name),
            )
            .child(track_source_pill(theme, idx, "sys", "System", &source_type, cx))
            .child(track_source_pill(theme, idx, "mic", "Mic",    &source_type, cx))
            .child(track_source_pill(theme, idx, "app", "App",    &source_type, cx))
            .child(div().flex_1())
            .child(match source_type.as_str() {
                "App" => Button::new(
                    SharedString::from(format!("at-apps-{}", idx)),
                    if app_count == 0 {
                        "Configure apps".to_string()
                    } else {
                        format!("{} apps", app_count)
                    },
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .icon(IconSource::Named("chevron-right".into()))
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    this.form_editing_track_index = Some(idx);
                    cx.notify();
                }))
                .into_any_element(),
                _ => div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child(if device.is_empty() { "Default".to_string() } else { device })
                    .into_any_element(),
            })
    }

    // ── Section 5: Auto-record ──────────────────────────────────────
    fn render_auto_record(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.form_auto_record;
        div()
            .id("auto-rec")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.tokens.border)
            .bg(theme.tokens.background)
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                this.form_auto_record = !this.form_auto_record;
                cx.notify();
            }))
            .child(
                div()
                    .w(px(36.0))
                    .h(px(20.0))
                    .rounded_full()
                    .relative()
                    .bg(if on { theme.tokens.primary } else { theme.tokens.border })
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .left(if on { px(18.0) } else { px(2.0) })
                            .size(px(16.0))
                            .rounded_full()
                            .bg(theme.tokens.foreground),
                    ),
            )
            .child(
                VStack::new()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child("Auto-record when detected"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child("Start the buffer automatically whenever this window becomes focused."),
                    ),
            )
    }

    // ── Footer ──────────────────────────────────────────────────────
    fn render_modal_footer(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let has_sel = self.form_hwnd.is_some();
        HStack::new()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(theme.tokens.border)
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child(if has_sel { "Ready to add." } else { "Pick a window to continue." }),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .child(
                        Button::new("modal-cancel", "Cancel")
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.close_add_source_modal(cx);
                            }))
                    )
                    .child(
                        Button::new("modal-add", "Add game source")
                            .icon(IconSource::Named("plus".into()))
                            .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                this.submit_add_source(window, cx);
                            }))
                    )
            )
    }

    fn submit_add_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(_hwnd) = self.form_hwnd else { return; };

        // Pull edited title from the input; fall back to the auto-filled form_title.
        let edited = self.add_source_title_input.read(cx).content().to_string();
        let title = if edited.trim().is_empty() { self.form_title.clone() } else { edited };
        if title.trim().is_empty() {
            return;
        }
        self.form_title = title.clone();

        let target_process = self.form_target_process.clone();
        log::info!("[UI] Adding new game source: '{}' (process: {:?})", title, target_process);

        let mut config = crate::config::AppConfig::load();
        let settings = GameSettings {
            title: title.clone(),
            target_process: target_process.clone(),
            auto_record: self.form_auto_record,
            retention_minutes: self.form_retention,
            video_overrides: if self.add_source_show_overrides {
                Some(crate::config::VideoSettings {
                    encoder: self.form_encoder.clone(),
                    rate_control_index: self.form_rate_control,
                    bitrate_kbps: self.form_bitrate,
                    cq_level: self.form_cq,
                    resolution: self.form_resolution.clone(),
                    fps: self.form_fps,
                    retention_minutes: self.form_retention,
                    gop_size: self.form_gop,
                    bframes: self.form_bframes,
                    preset: self.form_preset.clone(),
                    zero_latency: self.form_zero_latency,
                    lookahead: self.form_lookahead,
                    lookahead_frames: self.form_lookahead_frames,
                    spatial_aq: self.form_spatial_aq,
                    temporal_aq: self.form_temporal_aq,
                    artwork_path: None,
                })
            } else {
                None
            },
            audio_routing: Some(self.form_audio_tracks.clone()),
            record_focus_only: true,
            artwork_path: None,
        };

        config.game_registry.insert(title.clone(), settings.clone());
        config.save();

        self.app_state.game_registry.insert(title.clone(), settings);
        let next_id = self.app_state.manual_sessions.len() as i32 + 100;
        self.app_state.manual_sessions.insert(next_id, GameSession {
            id: next_id,
            title: title.clone(),
            auto_record: self.form_auto_record,
            retention: self.form_retention,
            bitrate: self.form_bitrate,
            cq: self.form_cq,
        });
        log::info!("[UI] Successfully updated state for '{}'", title);

        self.selected_source = Some(title.clone());
        self.load_video(&title, window, cx);
        self.show_toast(
            "Source Added",
            Some(&format!("{} is now available in your gallery.", title)),
            adabraka_ui::overlays::toast::ToastVariant::Success,
            window,
            cx,
        );
        self.close_add_source_modal(cx);
    }

    fn close_add_source_modal(&mut self, cx: &mut Context<Self>) {
        self.show_add_source_modal = false;
        self.form_hwnd = None;
        self.form_title = String::new();
        self.form_target_process = None;
        self.form_active_tab = 0;
        self.add_source_show_overrides = false;
        // Clear inputs by overwriting with empty.
        // We don't have a Window handle here, so leave the inputs as-is —
        // they'll be reset the next time a window is selected.
        cx.notify();
    }
}

// ── Local helpers ───────────────────────────────────────────────────
fn section_label(theme: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.tokens.muted_foreground)
        .child(text.to_string())
}

fn field_row(theme: &Theme, label: &str, control: AnyElement) -> impl IntoElement {
    HStack::new()
        .gap_4()
        .items_center()
        .child(
            div()
                .w(px(120.0))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.tokens.muted_foreground)
                .child(label.to_string()),
        )
        .child(div().flex_1().child(control))
}

fn track_source_pill(
    theme: &Theme,
    idx: usize,
    id_suffix: &'static str,
    label: &'static str,
    current: &str,
    cx: &mut Context<RekaptrWorkspace>,
) -> impl IntoElement {
    let active = current == label;
    let label_owned = label.to_string();
    div()
        .id(SharedString::from(format!("ts-{}-{}", idx, id_suffix)))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .cursor_pointer()
        .bg(if active { rgba(PRIMARY_DIM_A25).into() } else { theme.tokens.card })
        .border_1()
        .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
        .text_color(if active { theme.tokens.foreground } else { theme.tokens.muted_foreground })
        .hover(|s| s.text_color(theme.tokens.foreground))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut RekaptrWorkspace, _, _, cx| {
            this.form_audio_tracks[idx].source_type = label_owned.clone();
            cx.notify();
        }))
        .child(label)
}

fn stepper_inline(
    theme: &Theme,
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    suffix: &'static str,
    cx: &mut Context<RekaptrWorkspace>,
    on_change: impl Fn(&mut RekaptrWorkspace, i32) + 'static + Send + Sync + Clone,
) -> AnyElement {
    let on_dec = on_change.clone();
    let on_inc = on_change;
    HStack::new()
        .gap_2()
        .items_center()
        .child(
            Button::new(SharedString::from(format!("{}-dec", id_prefix)), "-")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this: &mut RekaptrWorkspace, _, _, cx| {
                    on_dec(this, (value - step).max(min));
                    cx.notify();
                }))
        )
        .child(
            div()
                .min_w(px(72.0))
                .text_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.tokens.foreground)
                .child(if suffix.is_empty() { format!("{}", value) } else { format!("{} {}", value, suffix) }),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", id_prefix)), "+")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this: &mut RekaptrWorkspace, _, _, cx| {
                    on_inc(this, (value + step).min(max));
                    cx.notify();
                }))
        )
        .into_any_element()
}
