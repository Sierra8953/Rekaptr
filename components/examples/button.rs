//! Example component: a simple button.
//!
//! Run with: `cargo run --example button`

use adabraka_ui::prelude::*;
use components::preview;
use gpui::{div, px, Application, Context, Window};

struct ButtonDemo;

impl Render for ButtonDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(theme.tokens.background)
            .child(
                div()
                    .px(px(20.0))
                    .py(px(10.0))
                    .bg(theme.tokens.primary)
                    .text_color(theme.tokens.primary_foreground)
                    .rounded(px(6.0))
                    .child("Click me"),
            )
    }
}

fn main() {
    Application::new().run(|cx| {
        preview::init(cx);
        let options = preview::window_options(cx);
        cx.open_window(options, |_, cx| cx.new(|_| ButtonDemo))
            .unwrap();
    });
}
