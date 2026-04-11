use gpui::*;
use crate::video_player::video;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;

impl LumaWorkspace {
    pub fn render_dashboard(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let progress = if duration > 0.0 { (position / duration) as f32 } else { 0.0 };

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
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.foreground)
                            .child("Dashboard")
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
                    .p_6()
                    .content(
                        VStack::new()
                            .gap_6()
                            .child(
                                // Simple progress bar
                                div()
                                    .id("scrubber-track")
                                    .h_2()
                                    .w_full()
                                    .bg(theme.tokens.secondary)
                                    .rounded_full()
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(progress))
                                            .bg(theme.tokens.primary)
                                            .rounded_full()
                                    )
                            )
                            .child(
                                HStack::new()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        HStack::new()
                                            .gap_4()
                                            .child(
                                                Button::new("btn-record", if is_recording { "STOP" } else { "REC" })
                                                    .variant(if is_recording { ButtonVariant::Destructive } else { ButtonVariant::Default })
                                                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.toggle_recording(cx);
                                                    }))
                                            )
                                            .child(
                                                Button::new("btn-play", if let Some(v) = &self.video_source { if v.paused() { "Play" } else { "Pause" } } else { "Play" })
                                                    .variant(ButtonVariant::Outline)
                                                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                        this.toggle_play_pause(cx);
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
            .child(self.render_game_gallery(_window, cx))
    }

    pub fn render_game_gallery(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.lock();
        let sessions = &state.manual_sessions;
        let theme = use_theme();
        
        let mut gallery = HStack::new()
            .h(px(180.0))
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
                                .child(div().text_3xl().text_color(theme.tokens.muted_foreground).child("+"))
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
                                            .child(div().w_2().h_2().rounded_full().bg(rgb(0x22c55e)))
                                    )
                                    .child(
                                        div().text_sm().text_color(theme.tokens.muted_foreground).child("Click to select / view buffer")
                                    )
                            )
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
