use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::LumaWorkspace;

pub struct TimelineState {
    pub progress: f32,
    pub clip_start: f32, // 0.0 to 1.0, -1.0 for disabled
    pub clip_end: f32,   // 0.0 to 1.0, -1.0 for disabled
    pub is_dragging_in: bool,
    pub is_dragging_out: bool,
    pub is_scrubbing: bool,
}

impl TimelineState {
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            clip_start: 0.2,
            clip_end: 0.8,
            is_dragging_in: false,
            is_dragging_out: false,
            is_scrubbing: false,
        }
    }
}

impl LumaWorkspace {
    pub fn render_timeline(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        
        // In a real app, these would come from the video player state
        let (position, duration) = if let Some(v) = &self.video_source {
            (v.position().as_secs_f64(), v.duration().as_secs_f64())
        } else {
            (0.0, 1.0)
        };
        let progress = if duration > 0.0 { (position / duration) as f32 } else { 0.0 };
        
        // For now, let's use some dummy clip values or track them in LumaWorkspace
        let clip_start = 0.1f32;
        let clip_end = 0.9f32;

        div()
            .w_full()
            .h(px(80.0))
            .bg(theme.tokens.card)
            .border_t_1()
            .border_color(theme.tokens.border)
            .px_8()
            .py_4()
            .child(
                VStack::new()
                    .gap_2()
                    .child(
                        HStack::new()
                            .justify_between()
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(format!("{:.1}s", position)))
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(format!("{:.1}s", duration)))
                    )
                    .child(
                        div()
                            .id("timeline-track")
                            .relative()
                            .w_full()
                            .h(px(40.0))
                            .bg(theme.tokens.background)
                            .rounded_md()
                            .border_1()
                            .border_color(theme.tokens.border)
                            .overflow_hidden()
                            // 1. Ruler Ticks
                            .child(self.render_ticks(theme.tokens.border))
                            // 2. Clip Range Highlight
                            .child(self.render_clip_range(clip_start, clip_end, theme.tokens.primary))
                            // 3. Progress Fill
                            .child(self.render_progress_fill(progress, theme.tokens.primary))
                            // 4. Playhead
                            .child(self.render_playhead(progress, theme.tokens.foreground))
                            // 5. Clip Markers
                            .child(self.render_clip_marker(clip_start, true, theme.tokens.primary, theme.tokens.background))
                            .child(self.render_clip_marker(clip_end, false, theme.tokens.primary, theme.tokens.background))
                    )
            )
    }

    fn render_ticks(&self, color: Hsla) -> impl IntoElement {
        let mut ticks = HStack::new().absolute().inset_0().items_end().px(px(10.0));
        for i in 0..21 {
            let height = if i % 5 == 0 { "40%" } else { "20%" };
            ticks = ticks.child(
                div()
                    .w(px(1.0))
                    .h(Relative(if i % 5 == 0 { 0.4 } else { 0.2 }))
                    .bg(color)
                    .map(|this| {
                        if i < 20 {
                            this.mr_auto()
                        } else {
                            this
                        }
                    })
            );
        }
        ticks
    }

    fn render_clip_range(&self, start: f32, end: f32, color: Hsla) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(Relative(start))
            .w(Relative(end - start))
            .bg(color.opacity(0.15))
    }

    fn render_progress_fill(&self, progress: f32, color: Hsla) -> impl IntoElement {
        // Linear gradient placeholder (GPUI doesn't have a direct linear-gradient method on Div yet, 
        // but we can use a solid color with low opacity for now or a custom shader if needed)
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(Relative(progress))
            .bg(color.opacity(0.1))
    }

    fn render_playhead(&self, progress: f32, color: Hsla) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(Relative(progress))
            .ml(px(-1.0))
            .w(px(2.0))
            .bg(color)
            .child(
                div()
                    .absolute()
                    .top_half()
                    .left_half()
                    .ml(px(-2.0))
                    .mt(px(-2.0))
                    .w(px(4.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(color)
            )
    }

    fn render_clip_marker(&self, position: f32, is_in: bool, color: Hsla, text_color: Hsla) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(Relative(position))
            .ml(px(if is_in { 0.0 } else { -2.0 }))
            .w(px(2.0))
            .bg(color)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(-6.0))
                    .w(px(14.0))
                    .h(px(18.0))
                    .bg(color)
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child(if is_in { "[" } else { "]" })
                    )
            )
    }
}
