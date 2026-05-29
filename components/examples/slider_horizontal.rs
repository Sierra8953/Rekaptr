//! Horizontal slider component.
//!
//! Run with: `cargo run --example slider_horizontal`

use adabraka_ui::prelude::*;
use components::preview;
use gpui::{
    div, px, Application, Context, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window,
};

const TRACK_WIDTH: f32 = 320.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB_SIZE: f32 = 18.0;

struct HorizontalSlider {
    value: f32,
    dragging: bool,
    drag_start_x: Pixels,
    drag_start_value: f32,
}

impl HorizontalSlider {
    fn new() -> Self {
        Self {
            value: 0.5,
            dragging: false,
            drag_start_x: px(0.0),
            drag_start_value: 0.5,
        }
    }
}

impl Render for HorizontalSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let value = self.value.clamp(0.0, 1.0);
        let thumb_offset = px(value * TRACK_WIDTH - THUMB_SIZE / 2.0);
        let fill_width = px(value * TRACK_WIDTH);

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(24.0))
            .size_full()
            .bg(theme.tokens.background)
            // Drag tracking lives on the root so movement/release work
            // anywhere in the window, not just over the thumb.
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if !this.dragging {
                    return;
                }
                // If the button was released outside the window, the up event
                // never reached us — detect it on re-entry and reset.
                if event.pressed_button != Some(MouseButton::Left) {
                    this.dragging = false;
                    cx.notify();
                    return;
                }
                let delta = event.position.x - this.drag_start_x;
                let new_value = this.drag_start_value + f32::from(delta) / TRACK_WIDTH;
                this.value = new_value.clamp(0.0, 1.0);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.dragging {
                        this.dragging = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .text_color(theme.tokens.foreground)
                    .child(format!("{:.2}", value)),
            )
            .child(
                // Track
                div()
                    .relative()
                    .w(px(TRACK_WIDTH))
                    .h(px(TRACK_HEIGHT))
                    .bg(theme.tokens.muted)
                    .rounded(px(TRACK_HEIGHT / 2.0))
                    .child(
                        // Fill
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .w(fill_width)
                            .h(px(TRACK_HEIGHT))
                            .bg(theme.tokens.primary)
                            .rounded(px(TRACK_HEIGHT / 2.0)),
                    )
                    .child(
                        // Thumb
                        div()
                            .absolute()
                            .left(thumb_offset)
                            .top(px(-(THUMB_SIZE - TRACK_HEIGHT) / 2.0))
                            .w(px(THUMB_SIZE))
                            .h(px(THUMB_SIZE))
                            .bg(theme.tokens.foreground)
                            .rounded(px(THUMB_SIZE / 2.0))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.dragging = true;
                                    this.drag_start_x = event.position.x;
                                    this.drag_start_value = this.value;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx| {
        preview::init(cx);
        let options = preview::window_options(cx);
        cx.open_window(options, |_, cx| cx.new(|_| HorizontalSlider::new()))
            .unwrap();
    });
}
