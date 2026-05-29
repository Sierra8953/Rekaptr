//! Vertical slider component.
//!
//! Run with: `cargo run --example slider_vertical`

use adabraka_ui::prelude::*;
use components::preview;
use gpui::{
    div, px, Application, Context, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window,
};

const TRACK_HEIGHT: f32 = 240.0;
const TRACK_WIDTH: f32 = 6.0;
const THUMB_SIZE: f32 = 18.0;

struct VerticalSlider {
    value: f32,
    dragging: bool,
    drag_start_y: Pixels,
    drag_start_value: f32,
}

impl VerticalSlider {
    fn new() -> Self {
        Self {
            value: 0.5,
            dragging: false,
            drag_start_y: px(0.0),
            drag_start_value: 0.5,
        }
    }
}

impl Render for VerticalSlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let value = self.value.clamp(0.0, 1.0);
        // value 0 = bottom, 1 = top; thumb offset measured from top of track
        let thumb_offset = px((1.0 - value) * TRACK_HEIGHT - THUMB_SIZE / 2.0);
        let fill_height = px(value * TRACK_HEIGHT);
        let fill_top = px((1.0 - value) * TRACK_HEIGHT);

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
                // Dragging down decreases value
                let delta = event.position.y - this.drag_start_y;
                let new_value = this.drag_start_value - f32::from(delta) / TRACK_HEIGHT;
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
                    .rounded(px(TRACK_WIDTH / 2.0))
                    .child(
                        // Fill (grows upward from bottom)
                        div()
                            .absolute()
                            .left_0()
                            .top(fill_top)
                            .w(px(TRACK_WIDTH))
                            .h(fill_height)
                            .bg(theme.tokens.primary)
                            .rounded(px(TRACK_WIDTH / 2.0)),
                    )
                    .child(
                        // Thumb
                        div()
                            .absolute()
                            .top(thumb_offset)
                            .left(px(-(THUMB_SIZE - TRACK_WIDTH) / 2.0))
                            .w(px(THUMB_SIZE))
                            .h(px(THUMB_SIZE))
                            .bg(theme.tokens.foreground)
                            .rounded(px(THUMB_SIZE / 2.0))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.dragging = true;
                                    this.drag_start_y = event.position.y;
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
        cx.open_window(options, |_, cx| cx.new(|_| VerticalSlider::new()))
            .unwrap();
    });
}
