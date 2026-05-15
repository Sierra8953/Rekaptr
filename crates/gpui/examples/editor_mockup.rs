use gpui::{
    App, Application, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

struct EditorMockup;

impl Render for EditorMockup {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Main Container
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x18181b)) // Zinc 900
            .text_color(rgb(0xe4e4e7)) // Zinc 200
            
            // Header
            .child(
                div()
                    .h(px(48.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(0x27272a)) // Zinc 800
                    .child(div().child("Quick Cut Editor").text_lg().font_weight(gpui::FontWeight::BOLD))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.button("Cancel", false))
                            .child(self.button("Export Clip", true))
                    )
            )

            // Preview Area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0x09090b)) // Zinc 950
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(640.0))
                            .h(px(360.0))
                            .bg(rgb(0x27272a))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(rgb(0x3f3f46))
                            .child("Video Preview Placeholder")
                    )
            )

            // Timeline Section
            .child(
                div()
                    .h(px(200.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x18181b))
                    .border_t_1()
                    .border_color(rgb(0x27272a))
                    
                    // Ruler
                    .child(
                        div()
                            .h(px(24.0))
                            .w_full()
                            .flex()
                            .items_center()
                            .px_4()
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .child(div().child("00:00").text_xs().text_color(rgb(0x71717a)))
                            .child(div().child("01:00").text_xs().text_color(rgb(0x71717a)).ml_auto())
                    )

                    // Tracks
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_4()
                            
                            // Video Track
                            .child(
                                div()
                                    .h(px(64.0))
                                    .w_full()
                                    .bg(rgb(0x09090b))
                                    .rounded_md()
                                    .relative()
                                    .child(
                                        // The Clip
                                        div()
                                            .absolute()
                                            .left(px(50.0))
                                            .right(px(100.0))
                                            .h_full()
                                            .bg(rgb(0x3b82f6)) // Blue 500
                                            .rounded_md()
                                            .flex()
                                            .items_center()
                                            .px_2()
                                            .child(div().child("elden_ring_boss_fight.mp4").text_sm().text_color(gpui::white()))
                                            // Handle Left
                                            .child(
                                                div()
                                                    .absolute()
                                                    .left_0()
                                                    .w(px(6.0))
                                                    .h_full()
                                                    .bg(rgb(0x60a5fa))
                                                    .rounded_l_md()
                                            )
                                            // Handle Right
                                            .child(
                                                div()
                                                    .absolute()
                                                    .right_0()
                                                    .w(px(6.0))
                                                    .h_full()
                                                    .bg(rgb(0x60a5fa))
                                                    .rounded_r_md()
                                            )
                                    )
                            )

                            // Audio Track
                            .child(
                                div()
                                    .h(px(32.0))
                                    .w_full()
                                    .bg(rgb(0x09090b))
                                    .rounded_md()
                                    .relative()
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(50.0))
                                            .right(px(100.0))
                                            .h_full()
                                            .bg(rgb(0x10b981)) // Emerald 500
                                            .rounded_md()
                                            .opacity(0.6)
                                    )
                            )
                    )

                    // Playhead (visual only)
                    .child(
                        div()
                            .absolute()
                            .left(px(300.0))
                            .top_0()
                            .bottom_0()
                            .w(px(2.0))
                            .bg(rgb(0xef4444)) // Red 500
                    )
            )
    }
}

impl EditorMockup {
    fn button(&self, label: &'static str, primary: bool) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .text_sm()
            .bg(if primary { rgb(0x3b82f6) } else { rgb(0x27272a) })
            .text_color(if primary { rgb(0xffffff) } else { rgb(0xe4e4e7) })
            .border_1()
            .border_color(if primary { rgb(0x2563eb) } else { rgb(0x3f3f46) })
            .child(label)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1024.), px(768.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Rekaptr Editor Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| EditorMockup),
        )
        .unwrap();
        cx.activate(true);
    });
}
