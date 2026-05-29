use adabraka_ui::components::icon::Icon;
use adabraka_ui::layout::{HStack, VStack};
use adabraka_ui::prelude::*;
use gpui::*;
use std::path::PathBuf;

struct Assets {
    base: PathBuf,
}

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(Into::into)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        std::fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|e| {
                        e.ok()
                            .and_then(|e| e.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

// Theme colors
const BG: u32 = 0x09090BFF;
const CARD: u32 = 0x18181BFF;
const BORDER: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const FG: u32 = 0xFAFAFAFF;
const MUTED: u32 = 0xA1A1AAFF;
const DESTRUCTIVE: u32 = 0xEF4444FF;

struct MockDashboard;

impl MockDashboard {
    fn render_sidebar(&self) -> impl IntoElement {
        let nav_items = vec![
            ("layout-dashboard", true),
            ("video", false),
            ("settings", false),
        ];

        VStack::new()
            .w(px(72.0))
            .h_full()
            .bg(rgba(CARD))
            .border_r_1()
            .border_color(rgba(BORDER))
            .pt(px(12.0))
            .px(px(8.0))
            .gap_2()
            .children(nav_items.into_iter().map(|(icon_name, active)| {
                div()
                    .w_full()
                    .h(px(56.0))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(px(48.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .bg(if active {
                                rgba(0x27272AFF)
                            } else {
                                rgba(0x00000000)
                            })
                            .child(
                                Icon::new(icon_name)
                                    .size(px(24.0))
                                    .color(if active { rgba(FG) } else { rgba(MUTED) }.into()),
                            ),
                    )
                    .when(active, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left(px(-8.0))
                                .top(px(16.0))
                                .w(px(3.0))
                                .h(px(24.0))
                                .rounded_r_sm()
                                .bg(rgba(PRIMARY)),
                        )
                    })
            }))
            .child(div().flex_1())
            .child(
                div()
                    .py_4()
                    .border_t_1()
                    .border_color(rgba(BORDER))
                    .flex()
                    .justify_center()
                    .child(div().w_2().h_2().rounded_full().bg(rgba(PRIMARY))),
            )
    }

    fn render_recording_overlay(&self) -> impl IntoElement {
        div()
            .absolute()
            .top_3()
            .right_3()
            .py_2()
            .px_3()
            .rounded(px(8.0))
            .bg(gpui::rgba(0x000000_bb))
            .child(
                VStack::new()
                    .gap_1()
                    .child(
                        HStack::new()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(rgba(DESTRUCTIVE)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(DESTRUCTIVE))
                                    .child("REC"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .child("04:32"),
                            ),
                    )
                    .child(
                        HStack::new()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xaaaaaa))
                                    .child("18.4 Mbps"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xaaaaaa))
                                    .child("2.3 MB/s"),
                            ),
                    )
                    .child(
                        HStack::new()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xaaaaaa))
                                    .child("Dropped: 0"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xaaaaaa))
                                    .child("Segments: 27"),
                            ),
                    ),
            )
    }

    fn render_controls(&self) -> impl IntoElement {
        HStack::new()
            .justify_between()
            .items_center()
            .child(
                HStack::new()
                    .gap_2()
                    .child(self.control_btn("square", true))
                    .child(self.control_btn("rotate-ccw", false))
                    .child(self.control_btn("pause", false))
                    .child(self.control_btn("rotate-cw", false))
                    .child(self.control_btn("rotate-cw", false))
                    .child(div().w(px(10.0)))
                    .child(self.control_btn_secondary("flag"))
                    .child(self.control_btn_secondary("crosshair"))
                    .child(self.control_btn_secondary("skull"))
                    .child(self.control_btn_secondary("star"))
                    .child(div().w(px(6.0)))
                    .child(self.text_btn("IN"))
                    .child(self.text_btn("OUT"))
                    .child(self.text_btn_primary("SAVE")),
            )
            .child(
                div()
                    .text_color(rgba(MUTED))
                    .text_sm()
                    .font_family("Consolas")
                    .child("2:16 / 4:32"),
            )
    }

    fn control_btn(&self, icon: &str, destructive: bool) -> impl IntoElement {
        let (bg, border) = if destructive {
            (rgba(DESTRUCTIVE), rgba(DESTRUCTIVE))
        } else {
            (rgba(0x00000000), rgba(BORDER))
        };
        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .rounded_md()
            .bg(bg)
            .border_1()
            .border_color(border)
            .child(
                Icon::new(icon)
                    .size(px(16.0))
                    .color(rgba(FG).into()),
            )
    }

    fn control_btn_secondary(&self, icon: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .rounded_md()
            .bg(rgba(0x27272AFF))
            .child(
                Icon::new(icon)
                    .size(px(16.0))
                    .color(rgba(MUTED).into()),
            )
    }

    fn text_btn(&self, label: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .px(px(10.0))
            .h(px(32.0))
            .rounded_md()
            .bg(rgba(0x27272AFF))
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgba(FG))
            .child(label.to_string())
    }

    fn text_btn_primary(&self, label: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .px(px(10.0))
            .h(px(32.0))
            .rounded_md()
            .bg(rgba(PRIMARY))
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(gpui::white())
            .child(label.to_string())
    }

    fn render_timeline(&self) -> impl IntoElement {
        let progress = 0.505; // ~halfway through

        div()
            .w_full()
            .bg(rgba(CARD))
            .border_1()
            .border_color(rgba(BORDER))
            .rounded_lg()
            .p_3()
            .child(
                HStack::new()
                    .w_full()
                    .gap_2()
                    .child(
                        VStack::new()
                            .w(px(160.0))
                            .gap_1()
                            .child(self.track_header("Video", true))
                            .child(self.track_header("System Audio", false))
                            .child(self.track_header("Mic", false)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .child(
                                VStack::new()
                                    .gap_1()
                                    .child(self.track_lane(progress, true))
                                    .child(self.track_lane(progress, false))
                                    .child(self.track_lane(progress, false)),
                            )
                            .child(
                                canvas(
                                    move |_, window, cx| {
                                        let layout_id = window.request_layout(
                                            Style {
                                                size: size(
                                                    relative(1.0).into(),
                                                    relative(1.0).into(),
                                                ),
                                                ..Default::default()
                                            },
                                            [],
                                            cx,
                                        );
                                        (layout_id, ())
                                    },
                                    move |bounds, _, window, _cx| {
                                        let width = bounds.size.width;
                                        let left = bounds.left();
                                        let top = bounds.top();
                                        let height = bounds.size.height;

                                        // Playhead
                                        let playhead_x = left + width * progress;

                                        // Glow
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(playhead_x - px(4.0), top),
                                                size(px(8.0), height),
                                            ),
                                            gpui::white().opacity(0.06),
                                        ));

                                        // Line
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(playhead_x - px(1.0), top),
                                                size(px(2.0), height),
                                            ),
                                            gpui::white().opacity(0.9),
                                        ));

                                        // Triangle head
                                        let head_w = px(12.0);
                                        let head_h = px(10.0);
                                        if let Ok(path) = {
                                            let mut pb = PathBuilder::fill();
                                            pb.move_to(point(playhead_x - head_w / 2.0, top));
                                            pb.line_to(point(playhead_x + head_w / 2.0, top));
                                            pb.line_to(point(playhead_x, top + head_h));
                                            pb.close();
                                            pb.build()
                                        } {
                                            window.paint_path(path, gpui::white());
                                        }

                                        // IN marker
                                        let in_x = left + width * 0.25;
                                        let in_color =
                                            gpui::hsla(142.0 / 360.0, 0.71, 0.45, 1.0);
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(in_x - px(1.0), top),
                                                size(px(2.0), height),
                                            ),
                                            in_color.opacity(0.8),
                                        ));
                                        // IN bracket top
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(in_x, top),
                                                size(px(10.0), px(2.0)),
                                            ),
                                            in_color,
                                        ));
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(in_x, top),
                                                size(px(2.0), px(16.0)),
                                            ),
                                            in_color,
                                        ));

                                        // OUT marker
                                        let out_x = left + width * 0.75;
                                        let out_color =
                                            gpui::hsla(346.0 / 360.0, 0.84, 0.61, 1.0);
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(out_x - px(1.0), top),
                                                size(px(2.0), height),
                                            ),
                                            out_color.opacity(0.8),
                                        ));
                                        // OUT bracket top
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(out_x - px(10.0), top),
                                                size(px(10.0), px(2.0)),
                                            ),
                                            out_color,
                                        ));
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(out_x - px(2.0), top),
                                                size(px(2.0), px(16.0)),
                                            ),
                                            out_color,
                                        ));

                                        // Clip range highlight
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(in_x, top),
                                                size(out_x - in_x, height),
                                            ),
                                            gpui::hsla(
                                                258.0 / 360.0,
                                                0.9,
                                                0.67,
                                                0.08,
                                            ),
                                        ));
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(in_x, top),
                                                size(out_x - in_x, px(2.0)),
                                            ),
                                            gpui::hsla(
                                                258.0 / 360.0,
                                                0.9,
                                                0.67,
                                                0.3,
                                            ),
                                        ));
                                    },
                                )
                                .absolute()
                                .inset_0()
                                .size_full(),
                            ),
                    ),
            )
    }

    fn track_header(&self, name: &str, is_video: bool) -> impl IntoElement {
        div()
            .w(px(160.0))
            .h(px(if is_video { 54.0 } else { 38.0 }))
            .px_3()
            .bg(rgba(CARD))
            .rounded_md()
            .border_1()
            .border_color(rgba(BORDER))
            .child(
                HStack::new()
                    .size_full()
                    .justify_between()
                    .items_center()
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .when(is_video, |this| {
                                this.child(Icon::new("video").size(px(16.0)).color(rgba(PRIMARY).into()))
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgba(FG))
                                    .child(name.to_string()),
                            ),
                    )
                    .when(!is_video, |this| {
                        this.child(
                            Icon::new("speaker")
                                .size(px(14.0))
                                .color(rgba(MUTED).into()),
                        )
                    }),
            )
    }

    fn track_lane(&self, progress: f32, is_video: bool) -> impl IntoElement {
        div()
            .flex_1()
            .h(px(if is_video { 54.0 } else { 38.0 }))
            .bg(rgba(BG))
            .rounded_md()
            .border_1()
            .border_color(rgba(BORDER))
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(relative(progress))
                    .bg(gpui::hsla(258.0 / 360.0, 0.9, 0.67, 0.08)),
            )
            .when(is_video, |this| {
                this.child(
                    HStack::new()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(0.12)
                        .gap(px(3.0))
                        .px(px(2.0))
                        .py(px(3.0))
                        .children((0..20).map(|i| {
                            let shade = if i % 2 == 0 { 0.6 } else { 0.4 };
                            div()
                                .w(px(80.0))
                                .h_full()
                                .rounded(px(3.0))
                                .bg(gpui::hsla(0.0, 0.0, 0.63, shade))
                        })),
                )
            })
    }

    fn render_source_card(
        &self,
        title: &str,
        subtitle: &str,
        selected: bool,
        has_artwork: bool,
    ) -> impl IntoElement {
        div()
            .relative()
            .w(px(240.0))
            .h(px(135.0))
            .border_color(if selected {
                rgba(PRIMARY)
            } else {
                rgba(BORDER)
            })
            .border(if selected { px(2.0) } else { px(1.0) })
            .rounded_xl()
            .overflow_hidden()
            .child(
                div()
                    .size_full()
                    .bg(rgba(CARD))
                    .when(has_artwork, |this| {
                        this.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(gpui::hsla(258.0 / 360.0, 0.3, 0.2, 1.0)),
                        )
                        .child(div().absolute().inset_0().bg(gpui::rgba(0x000000_99)))
                    })
                    .child(
                        VStack::new()
                            .relative()
                            .h_full()
                            .justify_between()
                            .p_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(gpui::white())
                                    .child(title.to_string()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::rgba(0xffffff_aa))
                                    .child(subtitle.to_string()),
                            ),
                    ),
            )
    }

    fn render_sessions_gallery(&self) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgba(FG))
                    .child("Recent Sessions"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        div()
                            .w(px(240.0))
                            .h(px(135.0))
                            .bg(rgba(CARD))
                            .border_2()
                            .border_color(rgba(BORDER))
                            .border_dashed()
                            .rounded_xl()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                VStack::new()
                                    .items_center()
                                    .child(
                                        Icon::new("plus")
                                            .size(px(32.0))
                                            .color(rgba(MUTED).into()),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgba(MUTED))
                                            .font_weight(FontWeight::MEDIUM)
                                            .mt_2()
                                            .child("Add Source"),
                                    ),
                            ),
                    )
                    .child(self.render_source_card("Monitor", "Record entire desktop", false, false))
                    .child(self.render_source_card(
                        "VALORANT",
                        "Click to preview recording",
                        true,
                        true,
                    ))
                    .child(self.render_source_card(
                        "ARC Raiders",
                        "Click to preview recording",
                        false,
                        true,
                    )),
            )
    }
}

impl Render for MockDashboard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let screenshot_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Valorant-review-2.jpg");

        div()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .flex()
            .child(self.render_sidebar())
            .child(
                div()
                    .id("main-scroll")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .p_8()
                            .gap_6()
                            .w_full()
                            // Video player area
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(480.0))
                                    .rounded_lg()
                                    .overflow_hidden()
                                    .child(
                                        img(screenshot_path)
                                            .size_full()
                                            .object_fit(ObjectFit::Cover),
                                    )
                                    .child(self.render_recording_overlay()),
                            )
                            // Controls + Timeline card
                            .child(
                                div()
                                    .bg(rgba(CARD))
                                    .border_1()
                                    .border_color(rgba(BORDER))
                                    .rounded_lg()
                                    .p_4()
                                    .child(
                                        VStack::new()
                                            .gap_4()
                                            .child(self.render_controls())
                                            .child(self.render_timeline()),
                                    ),
                            )
                            // Recent Sessions
                            .child(self.render_sessions_gallery()),
                    ),
            )
    }
}

fn main() {
    Application::new()
        .with_assets(Assets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        })
        .run(|cx| {
            adabraka_ui::init(cx);
            adabraka_ui::set_icon_base_path("assets/icons");
            install_theme(cx, Theme::dark());

            let bounds = Bounds::centered(None, size(px(1280.0), px(900.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rekaptr".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|_cx| MockDashboard),
            )
            .unwrap();
        });
}
