//! Timeline scrubber: draggable playhead with in/out clip markers and a
//! faux playback loop. A simplified standalone version of `src/ui/timeline.rs`
//! that lets you iterate on the marker/scrub UX without a real video pipeline.
//!
//! Run with: `cargo run --example timeline_scrubber`
//!
//! File layout: each visual piece is its own `render_*` method on
//! `TimelineScrubber`. Constants for that piece live at the top of its
//! method, so width/height/color/etc. are always next to where they're used.

use adabraka_ui::prelude::*;
use components::preview;
use gpui::{
    div, px, App, Context, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, Styled, Window,
};
use std::time::Instant;

// =================== Clip / track sizing ===================
// The track is the whole strip. Everything else positions itself relative
// to TRACK_HEIGHT, so changing the track size cascades correctly.
const DURATION_SECS: f64 = 120.0;
const TRACK_WIDTH: f32 = 640.0;
const TRACK_HEIGHT: f32 = 56.0;
const TRACK_BORDER: f32 = 1.0;
/// Vertical span available *inside* the track border for absolutely-positioned
/// children. Use this for anything that should fit flush inside the border.
const TRACK_INNER_H: f32 = TRACK_HEIGHT - TRACK_BORDER * 2.0;

// =================== State ===================

#[derive(Copy, Clone, PartialEq, Eq)]
enum DragTarget {
    Playhead,
    InMarker,
    OutMarker,
}

struct TimelineScrubber {
    /// Current playback position in seconds (0..DURATION_SECS).
    position: f64,
    /// Clip in/out points in seconds. None = unset.
    clip_in: Option<f64>,
    clip_out: Option<f64>,

    playing: bool,
    /// Wall-clock anchor for the faux play tick.
    last_tick: Instant,

    drag: Option<DragTarget>,
    drag_start_x: Pixels,
    drag_start_value: f64,
}

impl TimelineScrubber {
    fn new() -> Self {
        Self {
            position: 0.0,
            clip_in: Some(20.0),
            clip_out: Some(95.0),
            playing: false,
            last_tick: Instant::now(),
            drag: None,
            drag_start_x: px(0.0),
            drag_start_value: 0.0,
        }
    }

    // ---- Playback clock ----

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        if self.playing {
            self.position = (self.position + dt).min(DURATION_SECS);
            if self.position >= DURATION_SECS {
                self.playing = false;
            }
        }
    }

    fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.last_tick = Instant::now();
    }

    // ---- Drag math ----

    fn px_to_secs(delta: Pixels) -> f64 {
        f32::from(delta) as f64 / TRACK_WIDTH as f64 * DURATION_SECS
    }

    fn start_drag(&mut self, target: DragTarget, event_x: Pixels) {
        self.drag = Some(target);
        self.drag_start_x = event_x;
        self.drag_start_value = match target {
            DragTarget::Playhead => self.position,
            DragTarget::InMarker => self.clip_in.unwrap_or(0.0),
            DragTarget::OutMarker => self.clip_out.unwrap_or(DURATION_SECS),
        };
    }

    fn apply_drag(&mut self, event_x: Pixels) {
        let Some(target) = self.drag else { return };
        let delta = event_x - self.drag_start_x;
        let new_value =
            (self.drag_start_value + Self::px_to_secs(delta)).clamp(0.0, DURATION_SECS);
        match target {
            DragTarget::Playhead => self.position = new_value,
            DragTarget::InMarker => {
                let cap = self.clip_out.unwrap_or(DURATION_SECS);
                self.clip_in = Some(new_value.min(cap - 0.1));
            }
            DragTarget::OutMarker => {
                let floor = self.clip_in.unwrap_or(0.0);
                self.clip_out = Some(new_value.max(floor + 0.1));
            }
        }
    }
}

fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// secs (0..DURATION_SECS) → pixel x along the track (0..TRACK_WIDTH)
fn secs_to_px(secs: f64) -> f32 {
    (secs / DURATION_SECS).clamp(0.0, 1.0) as f32 * TRACK_WIDTH
}

// =================== Render: top-level frame ===================

impl Render for TimelineScrubber {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        // Drive the faux playback clock: advance time, then ask gpui to
        // repaint next frame so we keep ticking while `playing`.
        self.tick();
        if self.playing {
            window.request_animation_frame();
        }

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(20.0))
            .size_full()
            .bg(theme.tokens.background)
            // Window-level drag tracking: works anywhere in the window, and
            // resets if the button was released outside (re-entry has
            // pressed_button == None).
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.drag.is_none() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    this.drag = None;
                    cx.notify();
                    return;
                }
                this.apply_drag(event.position.x);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.is_some() {
                        this.drag = None;
                        cx.notify();
                    }
                }),
            )
            .child(self.render_header())
            .child(self.render_track(cx))
            .child(self.render_controls(cx))
    }
}

// =================== Render: per-piece methods ===================
// Each render_* method owns its own constants. Edit a piece in one place.

impl TimelineScrubber {
    /// Time readout + clip-length display above the track.
    fn render_header(&self) -> impl IntoElement {
        const GAP: f32 = 24.0;

        let theme = use_theme();
        let clip_len = match (self.clip_in, self.clip_out) {
            (Some(a), Some(b)) => fmt_time(b - a),
            _ => "—".into(),
        };

        div()
            .flex()
            .gap(px(GAP))
            .text_color(theme.tokens.foreground)
            .child(div().child(format!(
                "{} / {}",
                fmt_time(self.position),
                fmt_time(DURATION_SECS)
            )))
            .child(
                div()
                    .text_color(theme.tokens.muted_foreground)
                    .child(format!("clip: {}", clip_len)),
            )
    }

    /// The track strip and everything inside it (shading, ticks, handles, playhead).
    fn render_track(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const RADIUS: f32 = 6.0;

        let theme = use_theme();
        let in_x = self.clip_in.map(secs_to_px);
        let out_x = self.clip_out.map(secs_to_px);

        div()
            .relative()
            .w(px(TRACK_WIDTH))
            .h(px(TRACK_HEIGHT))
            .bg(theme.tokens.card)
            .border(px(TRACK_BORDER))
            .border_color(theme.tokens.border)
            .rounded(px(RADIUS))
            .child(self.render_clip_shading(in_x, out_x))
            .child(self.render_tick_marks())
            .when_some(in_x, |this, x| {
                this.child(self.render_handle(DragTarget::InMarker, x, theme.tokens.primary, cx))
            })
            .when_some(out_x, |this, x| {
                this.child(self.render_handle(DragTarget::OutMarker, x, gpui::rgb(0xef4444).into(), cx))
            })
            // Playhead drawn last so it stays on top.
            .child(self.render_playhead(cx))
    }

    /// Tinted band between the in and out markers.
    fn render_clip_shading(&self, in_x: Option<f32>, out_x: Option<f32>) -> impl IntoElement {
        const OPACITY: f32 = 0.18;

        let theme = use_theme();
        let (left, width) = match (in_x, out_x) {
            (Some(a), Some(b)) if b > a => (px(a), px(b - a)),
            _ => (px(0.0), px(0.0)),
        };
        div()
            .absolute()
            .top_0()
            .left(left)
            .w(width)
            .h(px(TRACK_INNER_H))
            .bg(theme.tokens.primary)
            .opacity(OPACITY)
    }

    /// Evenly-spaced minor tick marks along the bottom of the track.
    fn render_tick_marks(&self) -> impl IntoElement {
        const COUNT: u32 = 12;
        const W: f32 = 1.0;
        const H: f32 = 6.0;

        let theme = use_theme();
        div().absolute().top_0().left_0().w_full().h_full().children(
            (0..=COUNT).map(move |i| {
                let x = px(i as f32 / COUNT as f32 * TRACK_WIDTH - W / 2.0);
                div()
                    .absolute()
                    .top(px(TRACK_INNER_H - H))
                    .left(x)
                    .w(px(W))
                    .h(px(H))
                    .bg(theme.tokens.border)
            }),
        )
    }

    /// In/out marker handle. Same visual for both; color + drag target differ.
    fn render_handle(
        &self,
        target: DragTarget,
        x: f32,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const W: f32 = 2.5;
        const H: f32 = TRACK_INNER_H;
        const Y_OFFSET: f32 = -1.0; // nudge up 1px so it sits flush
        const RADIUS: f32 = 2.0;
        const OPACITY: f32 = 0.9;

        div()
            .absolute()
            .top(px((TRACK_INNER_H - H) / 2.0 + Y_OFFSET))
            .left(px(x - W / 2.0))
            .w(px(W))
            .h(px(H))
            .bg(color)
            .rounded(px(RADIUS))
            .opacity(OPACITY)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.start_drag(target, event.position.x);
                    cx.notify();
                }),
            )
    }

    /// Vertical bar showing the current playback position.
    fn render_playhead(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const W: f32 = 3.0;
        const H: f32 = TRACK_INNER_H;
        const Y_OFFSET: f32 = -1.0;
        const RADIUS: f32 = 1.5;

        let theme = use_theme();
        let x = secs_to_px(self.position);

        div()
            .absolute()
            .top(px((TRACK_INNER_H - H) / 2.0 + Y_OFFSET))
            .left(px(x - W / 2.0))
            .w(px(W))
            .h(px(H))
            .bg(theme.tokens.foreground)
            .rounded(px(RADIUS))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.start_drag(DragTarget::Playhead, event.position.x);
                    cx.notify();
                }),
            )
    }

    /// Play/pause + "set in/out to playhead" buttons.
    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const GAP: f32 = 8.0;
        let play_label = if self.playing { "Pause" } else { "Play" };

        div()
            .flex()
            .gap(px(GAP))
            .child(self.render_primary_button(play_label, cx.listener(|this, _, _, cx| {
                this.toggle_play();
                cx.notify();
            })))
            .child(self.render_secondary_button(
                "Set In = playhead",
                cx.listener(|this, _, _, cx| {
                    this.clip_in = Some(this.position);
                    if let Some(out) = this.clip_out {
                        if out <= this.position {
                            this.clip_out = Some(this.position + 0.1);
                        }
                    }
                    cx.notify();
                }),
            ))
            .child(self.render_secondary_button(
                "Set Out = playhead",
                cx.listener(|this, _, _, cx| {
                    this.clip_out = Some(this.position);
                    if let Some(inp) = this.clip_in {
                        if inp >= this.position {
                            this.clip_in = Some((this.position - 0.1).max(0.0));
                        }
                    }
                    cx.notify();
                }),
            ))
    }

    fn render_primary_button(
        &self,
        label: &'static str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        const PX_X: f32 = 16.0;
        const PX_Y: f32 = 8.0;
        const RADIUS: f32 = 6.0;

        let theme = use_theme();
        div()
            .px(px(PX_X))
            .py(px(PX_Y))
            .bg(theme.tokens.primary)
            .text_color(theme.tokens.primary_foreground)
            .rounded(px(RADIUS))
            .child(label)
            .on_mouse_down(MouseButton::Left, on_click)
    }

    fn render_secondary_button(
        &self,
        label: &'static str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        const PX_X: f32 = 16.0;
        const PX_Y: f32 = 8.0;
        const RADIUS: f32 = 6.0;

        let theme = use_theme();
        div()
            .px(px(PX_X))
            .py(px(PX_Y))
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .text_color(theme.tokens.foreground)
            .rounded(px(RADIUS))
            .child(label)
            .on_mouse_down(MouseButton::Left, on_click)
    }
}

fn main() {
    preview::run(|| TimelineScrubber::new());
}
