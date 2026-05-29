use adabraka_ui::prelude::*;
use gpui::{
    div, hsla, px, rgb, size, App, Application, Bounds, Context, IntoElement, ParentElement,
    Render, Styled, Window, WindowBounds, WindowKind, WindowOptions,
};

/// Initialize adabraka-ui and install the same Slint-consistent Violet
/// theme the main rekaptr app uses (see src/main.rs).
/// Call this once at the start of an example before opening a window.
pub fn init(cx: &mut App) {
    adabraka_ui::init(cx);

    let mut theme = adabraka_ui::theme::Theme::dark();
    theme.tokens.primary = hsla(258.0 / 360.0, 0.90, 0.66, 1.0); // Violet 500 (#8b5cf6)
    theme.tokens.background = rgb(0x09090b).into(); // Zinc 950
    theme.tokens.card = rgb(0x18181b).into(); // Zinc 900
    theme.tokens.border = rgb(0x3f3f46).into(); // Zinc 700

    adabraka_ui::theme::install_theme(cx, theme);
}

/// Standard window options for previewing a single component.
pub fn window_options(cx: &mut App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::Normal,
        ..Default::default()
    }
}

/// One-call entry point for an example: init + open a window rendering `view`.
/// `factory` builds the view; called inside the window's entity context.
pub fn run<V, F>(factory: F)
where
    V: Render + 'static,
    F: FnOnce() -> V + Send + 'static,
{
    Application::new().run(|cx| {
        init(cx);
        let options = window_options(cx);
        cx.open_window(options, |_, cx| cx.new(|_| factory()))
            .unwrap();
    });
}

/// Placeholder view used by stubbed-out example files.
/// Renders the component name + a TODO note in the themed window.
pub struct Placeholder {
    name: &'static str,
}

impl Placeholder {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .size_full()
            .bg(theme.tokens.background)
            .child(
                div()
                    .text_color(theme.tokens.foreground)
                    .text_size(px(20.0))
                    .child(self.name),
            )
            .child(
                div()
                    .text_color(theme.tokens.muted_foreground)
                    .child("not implemented yet"),
            )
    }
}
