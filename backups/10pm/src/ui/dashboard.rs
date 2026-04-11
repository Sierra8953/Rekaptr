use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use adabraka_ui::overlays::popover::PopoverContent;
use adabraka_ui::navigation::tabs::TabPanel;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_dashboard(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(v) = &self.video_source {
            if !v.paused() {
                cx.notify();
            }
        }

        let theme = use_theme();
        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64())
        } else {
            (0.0, 1.0)
        };

        let video_element = match &self.video_source {
            Some(v) => {
                let v_clone = v.clone();
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .child(
                        canvas(
                            {
                                let v_inner = v_clone.clone();
                                move |bounds, _window, _cx| {
                                    let w = f32::from(bounds.size.width).round() as u32;
                                    let h = f32::from(bounds.size.height).round() as u32;
                                    let (current_w, current_h) = v_inner.display_size();
                                    
                                    if w > 0 && h > 0 && (w != current_w || h != current_h) {
                                        v_inner.set_display_size(Some(w), Some(h));
                                    }
                                }
                            },
                            |_bounds, _state, _window, _cx| {}
                        ).w_full().h_full()
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .child(video(v.clone()).id("main-video").buffer_capacity(30))
                    )
                    .into_any_element()
            }
            None => div().w_full().h_full().bg(rgb(0x000000)).flex().items_center().justify_center().child(div().text_color(theme.tokens.muted_foreground).child("No video source loaded")).into_any_element(),
        };

        let is_recording = self.app_state.lock().is_recording.load(std::sync::atomic::Ordering::SeqCst);

        VStack::new()
            .flex_1()
            .p_8()
            .gap_6()
            .min_h_0()
            .child(
                HStack::new()
                    .justify_between()
                    .items_center()
                    .child(
                        HStack::new()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child("Dashboard")
                            )
                            .when(is_recording, |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2()
                                        .py_1()
                                        .bg(theme.tokens.destructive.opacity(0.1))
                                        .rounded_md()
                                        .child(div().w_2().h_2().rounded_full().bg(theme.tokens.destructive))
                                        .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(theme.tokens.destructive).child("REC"))
                                )
                            })
                    )
                    .child(
                        div()
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("Source: {}", self.selected_source.as_deref().unwrap_or("Monitor")))
                    )
            )
            .child(
                Card::new()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
                    .content(video_element)
            )
            .child(
                Card::new()
                    .p_4()
                    .content(
                        VStack::new()
                            .gap_4()
                            // Timeline Row
                            .child(self.render_timeline(window, cx))
                            // Controls Row
                            .child(
                                HStack::new()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        HStack::new()
                                            .gap_2()
                                            .child(
                                                Button::new("btn-record", "")
                                                    .icon(if is_recording { IconSource::Named("square".to_string()) } else { IconSource::Named("circle-dot".to_string()) })
                                                    .variant(if is_recording { ButtonVariant::Destructive } else { ButtonVariant::Default })
                                                    .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                        this.toggle_recording(window, cx);
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-back", "")
                                                    .icon(IconSource::Named("rotate-ccw".to_string()))
                                                    .variant(ButtonVariant::Outline)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, _cx| {
                                                        if let Some(v) = &this.video_source {
                                                            let new_pos = (v.position().as_secs_f64() - 10.0).max(0.0);
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                        }
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-play", "")
                                                    .icon(if let Some(v) = &self.video_source { if v.paused() { IconSource::Named("play".to_string()) } else { IconSource::Named("pause".to_string()) } } else { IconSource::Named("play".to_string()) })
                                                    .variant(ButtonVariant::Outline)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.toggle_play_pause(cx);
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-fwd", "")
                                                    .icon(IconSource::Named("rotate-cw".to_string()))
                                                    .variant(ButtonVariant::Outline)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, _cx| {
                                                        if let Some(v) = &this.video_source {
                                                            let new_pos = (v.position().as_secs_f64() + 30.0).min(v.duration().as_secs_f64());
                                                            let _ = v.seek(std::time::Duration::from_secs_f64(new_pos), true);
                                                        }
                                                    }))
                                            )
                                            .child(div().w(px(10.0)))
                                            .child(
                                                Button::new("btn-in", "IN")
                                                    .variant(ButtonVariant::Secondary)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.set_clip_in(cx);
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-out", "OUT")
                                                    .variant(ButtonVariant::Secondary)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.set_clip_out(cx);
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-save", "SAVE")
                                                    .variant(ButtonVariant::Default)
                                                    .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                                                        this.save_clip(window, cx);
                                                    }))
                                            )
                                    )
                                    .child(
                                        div()
                                            .text_color(theme.tokens.muted_foreground)
                                            .text_sm()
                                            .font_family(".system-monospaced")
                                            .child(format!("{:.1}s / {:.1}s", position, duration))
                                    )
                            )
                    )
            )
            .child(self.render_game_gallery(window, cx))
    }

    pub fn render_game_gallery(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.lock();
        let sessions = &state.manual_sessions;
        let theme = use_theme();
        
        let mut gallery = HStack::new()
            .h(px(140.0)) // Reduced from 180 to give more vertical space to the video
            .gap_4()
            .overflow_x_scroll();

        gallery = gallery.child(
            div()
                .id("add-source-wrap")
                .child(
                    Card::new()
                        .w(px(200.0))
                        .h_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .bg(theme.tokens.card)
                        .content(
                            VStack::new()
                                .items_center()
                                .child(
                                    div()
                                        .id("add-plus-icon")
                                        .child(Icon::new("plus").size(px(32.0)).color(theme.tokens.muted_foreground))
                                )
                                .child(div().text_color(theme.tokens.muted_foreground).font_weight(FontWeight::MEDIUM).mt_2().child("Add Source"))
                        )
                )
                .cursor_pointer()
                .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_add_source_modal = true;
                    let mut s = this.app_state.lock();
                    let mut detector = crate::game_detector::GameDetector::new();
                    s.available_windows = detector.enumerate_windows();
                    cx.notify();
                }))
        );

        let monitor_selected = self.selected_source.as_deref() == Some("Monitor");
        gallery = gallery.child(
            div()
                .id("monitor-source-wrap")
                .relative()
                .child(
                    Card::new()
                        .w(px(256.0))
                        .h_full()
                        .border_color(if monitor_selected { theme.tokens.primary } else { theme.tokens.border })
                        .content(
                            VStack::new()
                                .h_full()
                                .justify_between()
                                .p_4()
                                .child(
                                    HStack::new()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child("Monitor"))
                                        .child(div().w_2().h_2().rounded_full().bg(theme.tokens.primary))
                                )
                                .child(
                                    div().text_sm().text_color(theme.tokens.muted_foreground).child("Record entire desktop")
                                )
                        )
                )
                .child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .child(
                            div()
                                .id("monitor-settings-btn-hitbox")
                                .child(
                                    Button::new("monitor-settings-btn", "")
                                        .icon(IconSource::Named("settings".to_string()))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                )
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.advanced_settings_source = Some("Monitor".to_string());
                                    cx.notify();
                                }))
                        )
                )
                .cursor_pointer()
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    this.selected_source = Some("Monitor".to_string());
                    cx.notify();
                }))
        );

        for session in sessions.iter() {
            let title = session.value().title.to_string();
            let is_selected = self.selected_source.as_deref() == Some(title.as_str());
            gallery = gallery.child(
                div()
                    .id(("session-wrap", *session.key() as usize))
                    .relative()
                    .child(
                        Card::new()
                            .w(px(256.0))
                            .h_full()
                            .border_color(if is_selected { theme.tokens.primary } else { theme.tokens.border })
                            .content(
                                VStack::new()
                                    .h_full()
                                    .justify_between()
                                    .p_4()
                                    .child(
                                        HStack::new()
                                            .justify_between()
                                            .items_center()
                                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(title.clone()))
                                            .child(div().w_2().h_2().rounded_full().bg(gpui::hsla(142.0/360.0, 0.71, 0.45, 1.0))) // Green
                                    )
                                    .child(
                                        div().text_sm().text_color(theme.tokens.muted_foreground).child("Click to select / view buffer")
                                    )
                            )
                    )
                    .child(
                        div()
                            .absolute()
                            .top_2()
                            .right_2()
                            .child({
                                let title = title.clone();
                                let key = *session.key() as usize;
                                div()
                                    .id(("session-settings-btn-hitbox", key))
                                    .child(
                                        Button::new(("session-settings-btn", key), "")
                                            .icon(IconSource::Named("settings".to_string()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.advanced_settings_source = Some(title.clone());
                                        cx.notify();
                                    }))
                            })
                    )
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let title = title.clone();
                        move |this: &mut Self, _, _, cx| {
                            this.selected_source = Some(title.clone());
                            if let Some(playlist_path) = crate::utils::generate_session_playlist(&title) {
                                this.load_video(&playlist_path, cx);
                            }
                            cx.notify();
                        }
                    }))
            );
        }

        gallery
    }
}
