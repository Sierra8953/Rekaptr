//! Stepper — numeric +/- control with min/max/step.
//!
//! Replaces `stepper`, `stepper_f32`, `stepper_inline`, and the inline +/-
//! pairs in `src/ui/settings/storage.rs`.
//!
//! Run with: cargo run --example stepper

use std::sync::Arc;

use adabraka_ui::prelude::*;
use components::preview;
use gpui::{
    div, px, App, ClickEvent, Context, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, WeakEntity, Window,
};

/// A +/- numeric stepper rendered as `[-]  value  [+]`.
///
/// The caller owns the value; `on_change` is invoked with the new clamped
/// value when either button is pressed. Buttons are disabled at min/max.
///
/// `id_prefix` must be unique per instance on a page (used to derive the two
/// button element ids).
pub fn stepper(
    id_prefix: impl Into<SharedString>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    format: impl Fn(f64) -> String + 'static,
    on_change: impl Fn(f64, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = use_theme();
    let id_prefix = id_prefix.into();
    let on_change = Arc::new(on_change);

    let at_min = value <= min;
    let at_max = value >= max;

    let on_dec = on_change.clone();
    let on_inc = on_change;

    HStack::new()
        .gap_2()
        .items_center()
        .child(
            Button::new(SharedString::from(format!("{}-dec", id_prefix)), "−")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .disabled(at_min)
                .on_click(move |_: &ClickEvent, window, cx| {
                    let next = (value - step).max(min);
                    if (next - value).abs() > f64::EPSILON {
                        on_dec(next, window, cx);
                    }
                }),
        )
        .child(
            div()
                .min_w(px(56.0))
                .text_center()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.tokens.foreground)
                .child(format(value)),
        )
        .child(
            Button::new(SharedString::from(format!("{}-inc", id_prefix)), "+")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .disabled(at_max)
                .on_click(move |_: &ClickEvent, window, cx| {
                    let next = (value + step).min(max);
                    if (next - value).abs() > f64::EPSILON {
                        on_inc(next, window, cx);
                    }
                }),
        )
}

// ── Demo ─────────────────────────────────────────────────────────────

struct StepperDemo {
    buffer_gb: f64,
    fps: f64,
    retention_days: f64,
}

impl StepperDemo {
    fn new() -> Self {
        Self { buffer_gb: 30.0, fps: 60.0, retention_days: 7.0 }
    }
}

fn row(label: &'static str, hint: String, control: impl IntoElement) -> impl IntoElement {
    let theme = use_theme();
    HStack::new()
        .w_full()
        .items_center()
        .justify_between()
        .py_3()
        .border_b_1()
        .border_color(theme.tokens.border.opacity(0.3))
        .child(
            VStack::new()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.tokens.foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.tokens.muted_foreground)
                        .child(hint),
                ),
        )
        .child(control)
}

/// Build an `on_change` closure that writes `v` into the field selected by
/// `set` on the demo struct, via a weak entity handle.
fn set_field(
    weak: WeakEntity<StepperDemo>,
    set: impl Fn(&mut StepperDemo, f64) + 'static,
) -> impl Fn(f64, &mut Window, &mut App) + 'static {
    move |v, _window, cx| {
        let _ = weak.update(cx, |this, cx| {
            set(this, v);
            cx.notify();
        });
    }
}

impl Render for StepperDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let weak = cx.weak_entity();

        let buffer_gb = self.buffer_gb;
        let fps = self.fps;
        let retention = self.retention_days;

        VStack::new()
            .size_full()
            .bg(theme.tokens.background)
            .items_center()
            .justify_center()
            .p_8()
            .child(
                VStack::new()
                    .w(px(480.0))
                    .p_6()
                    .gap_2()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.card)
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .pb_2()
                            .child("Stepper"),
                    )
                    .child(row(
                        "Buffer size",
                        format!("integer, 10–500, step 5 · current: {} GB", buffer_gb as i32),
                        stepper(
                            "buffer",
                            buffer_gb,
                            10.0,
                            500.0,
                            5.0,
                            |v| format!("{} GB", v as i32),
                            set_field(weak.clone(), |this, v| this.buffer_gb = v),
                        ),
                    ))
                    .child(row(
                        "Frame rate",
                        format!("float, 24.0–240.0, step 0.5 · current: {:.1}", fps),
                        stepper(
                            "fps",
                            fps,
                            24.0,
                            240.0,
                            0.5,
                            |v| format!("{:.1} fps", v),
                            set_field(weak.clone(), |this, v| this.fps = v),
                        ),
                    ))
                    .child(row(
                        "Retention",
                        format!("integer, 1–365, step 1 · current: {} days", retention as i32),
                        stepper(
                            "retention",
                            retention,
                            1.0,
                            365.0,
                            1.0,
                            |v| format!("{} d", v as i32),
                            set_field(weak, |this, v| this.retention_days = v),
                        ),
                    )),
            )
    }
}

fn main() {
    preview::run(StepperDemo::new);
}
