use adabraka_ui::{
    components::icon::{Icon},
    components::icon_source::IconSource,
    layout::{HStack, VStack},
    prelude::*,
};
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

struct Assets { base: PathBuf }

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(Into::into)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        std::fs::read_dir(self.base.join(path))
            .map(|entries| entries.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()).map(SharedString::from)).collect())
            .map_err(Into::into)
    }
}

// ── OBS-style Select ────────────────────────────────────────────────

// Zinc/Violet palette — matches the app's theme from main.rs
const BG_INPUT: u32     = 0x18181BFF; // Zinc 900 (card)
const BG_HOVER: u32     = 0x27272AFF; // Zinc 800
const BG_PANEL: u32     = 0x18181BFF; // Zinc 900
const BG_ITEM_HOVER: u32 = 0x27272AFF; // Zinc 800
const BG_ITEM_ACTIVE: u32 = 0x7C3AEDFF; // Violet 600
const BORDER_COLOR: u32 = 0x3F3F46FF; // Zinc 700
const BORDER_FOCUS: u32 = 0x8B5CF6FF; // Violet 500 (primary)
const TEXT_PRIMARY: u32 = 0xFAFAFAFF; // Zinc 50
const TEXT_DIM: u32     = 0xA1A1AAFF; // Zinc 400

#[derive(Clone)]
struct SelectItem {
    id: SharedString,
    label: SharedString,
}

struct AppSelect {
    focus_handle: FocusHandle,
    items: Vec<SelectItem>,
    selected: Option<usize>,
    open: bool,
    bounds: Bounds<Pixels>,
    on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync>>,
}

impl AppSelect {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
            selected: None,
            open: false,
            bounds: Bounds::default(),
            on_change: None,
        }
    }

    fn items(mut self, items: Vec<(&str, &str)>) -> Self {
        self.items = items.into_iter().map(|(id, label)| SelectItem {
            id: SharedString::from(id.to_string()),
            label: SharedString::from(label.to_string()),
        }).collect();
        self
    }

    fn selected_index(mut self, idx: usize) -> Self {
        self.selected = Some(idx);
        self
    }

    fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    fn selected_label(&self) -> &str {
        self.selected
            .and_then(|i| self.items.get(i))
            .map(|item| item.label.as_ref())
            .unwrap_or("Select...")
    }

    fn selected_id(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.items.get(i))
            .map(|item| item.id.as_ref())
    }

    pub fn set_selected_by_id(&mut self, id: &str) {
        self.selected = self.items.iter().position(|item| item.id.as_ref() == id);
    }
}

impl Render for AppSelect {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open;
        let selected_label: SharedString = self.selected_label().to_string().into();
        let has_selection = self.selected.is_some();
        let bounds = self.bounds;

        let trigger = div()
            .id("select-trigger")
            .flex()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .px(px(8.0))
            .bg(rgba(BG_INPUT))
            .border_1()
            .border_color(if open { rgba(BORDER_FOCUS) } else { rgba(BORDER_COLOR) })
            .rounded(px(3.0))
            .text_color(if has_selection { rgba(TEXT_PRIMARY) } else { rgba(TEXT_DIM) })
            .text_size(px(13.0))
            .cursor_pointer()
            .when(!open, |d| {
                d.hover(|s| s.bg(rgba(BG_HOVER)).border_color(rgba(0x52525BFF)))
            })
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.open = !this.open;
                if this.open {
                    window.focus(&this.focus_handle);
                }
                cx.notify();
            }))
            .child(
                div().overflow_hidden().text_ellipsis().child(selected_label)
            )
            .child(
                Icon::new(IconSource::Named(
                    if open { "chevron-up".into() } else { "chevron-down".into() }
                ))
                .size(px(14.0))
                .color(rgba(TEXT_DIM).into())
            )
            .child({
                let entity = cx.entity().clone();
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| { this.bounds = bounds; })
                    },
                    |_, _, _, _| {},
                ).absolute().size_full()
            });

        let items_for_panel: Vec<(usize, SelectItem)> = self.items.iter().cloned().enumerate().collect();
        let current_selected = self.selected;

        div()
            .relative()
            .w_full()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.open { this.open = false; cx.notify(); }
            }))
            .child(trigger)
            .when(open, |d| {
                d.child(
                    deferred(
                        anchored()
                            .snap_to_window_with_margin(Edges::all(px(4.0)))
                            .child(
                                div()
                                    .occlude()
                                    .w(bounds.size.width)
                                    .child(
                                        div()
                                            .occlude()
                                            .mt(px(2.0))
                                            .bg(rgba(BG_PANEL))
                                            .border_1()
                                            .border_color(rgba(BORDER_COLOR))
                                            .rounded(px(3.0))
                                            .shadow_md()
                                            .overflow_hidden()
                                            .py(px(2.0))
                                            .id("select-panel")
                                            .max_h(px(200.0))
                                            .overflow_y_scroll()
                                            .children(
                                                items_for_panel.into_iter().map(|(idx, item)| {
                                                    let is_selected = current_selected == Some(idx);
                                                    div()
                                                        .id(SharedString::from(format!("item-{idx}")))
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(6.0))
                                                        .h(px(26.0))
                                                        .px(px(8.0))
                                                        .text_size(px(13.0))
                                                        .cursor_pointer()
                                                        .when(is_selected, |d| {
                                                            d.bg(rgba(BG_ITEM_ACTIVE))
                                                             .text_color(gpui::white())
                                                        })
                                                        .when(!is_selected, |d| {
                                                            d.text_color(rgba(TEXT_PRIMARY))
                                                             .hover(|s| s.bg(rgba(BG_ITEM_HOVER)))
                                                        })
                                                        .when(is_selected, |d| {
                                                            d.child(
                                                                Icon::new(IconSource::Named("check".into()))
                                                                    .size(px(12.0))
                                                                    .color(gpui::white())
                                                            )
                                                        })
                                                        .when(!is_selected, |d| {
                                                            d.child(div().w(px(12.0)))
                                                        })
                                                        .child(item.label.clone())
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                            this.selected = Some(idx);
                                                            this.open = false;
                                                            cx.notify();
                                                            if let Some(cb) = &this.on_change {
                                                                if let Some(item) = this.items.get(idx) {
                                                                    (cb)(item.id.as_ref(), window, cx);
                                                                }
                                                            }
                                                        }))
                                                })
                                            )
                                    )
                            )
                    ).with_priority(1)
                )
            })
    }
}

// ── Demo App ────────────────────────────────────────────────────────

struct SettingsDemo {
    encoder: Entity<AppSelect>,
    resolution: Entity<AppSelect>,
    fps: Entity<AppSelect>,
    preset: Entity<AppSelect>,
    rate_control: Entity<AppSelect>,
    status: String,
}

impl SettingsDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let e1 = cx.entity().clone();
        let e2 = cx.entity().clone();
        let e3 = cx.entity().clone();
        let e4 = cx.entity().clone();
        let e5 = cx.entity().clone();

        Self {
            encoder: cx.new(|cx| {
                AppSelect::new(cx)
                    .items(vec![
                        ("h264_nvenc", "H.264 (NVENC)"),
                        ("hevc_nvenc", "HEVC (NVENC)"),
                        ("av1_nvenc", "AV1 (NVENC)"),
                        ("x264", "H.264 (x264)"),
                    ])
                    .selected_index(0)
                    .on_change(move |val, _, cx| {
                        e1.update(cx, |this, cx| {
                            this.status = format!("Encoder → {val}");
                            cx.notify();
                        });
                    })
            }),
            resolution: cx.new(|cx| {
                AppSelect::new(cx)
                    .items(vec![
                        ("original", "Original"),
                        ("3840x2160", "3840×2160"),
                        ("2560x1440", "2560×1440"),
                        ("1920x1080", "1920×1080"),
                        ("1280x720", "1280×720"),
                    ])
                    .selected_index(0)
                    .on_change(move |val, _, cx| {
                        e2.update(cx, |this, cx| {
                            this.status = format!("Resolution → {val}");
                            cx.notify();
                        });
                    })
            }),
            fps: cx.new(|cx| {
                AppSelect::new(cx)
                    .items(vec![
                        ("30", "30 FPS"),
                        ("60", "60 FPS"),
                        ("120", "120 FPS"),
                        ("144", "144 FPS"),
                        ("165", "165 FPS"),
                        ("240", "240 FPS"),
                    ])
                    .selected_index(1)
                    .on_change(move |val, _, cx| {
                        e3.update(cx, |this, cx| {
                            this.status = format!("FPS → {val}");
                            cx.notify();
                        });
                    })
            }),
            preset: cx.new(|cx| {
                AppSelect::new(cx)
                    .items(vec![
                        ("p1", "P1 (Fastest)"),
                        ("p2", "P2"),
                        ("p3", "P3"),
                        ("p4", "P4 (Balanced)"),
                        ("p5", "P5"),
                        ("p6", "P6"),
                        ("p7", "P7 (Best Quality)"),
                    ])
                    .selected_index(3)
                    .on_change(move |val, _, cx| {
                        e4.update(cx, |this, cx| {
                            this.status = format!("Preset → {val}");
                            cx.notify();
                        });
                    })
            }),
            rate_control: cx.new(|cx| {
                AppSelect::new(cx)
                    .items(vec![
                        ("cbr", "CBR (Constant)"),
                        ("vbr", "VBR (Variable)"),
                        ("cqp", "CQP (Constant QP)"),
                    ])
                    .selected_index(0)
                    .on_change(move |val, _, cx| {
                        e5.update(cx, |this, cx| {
                            this.status = format!("Rate Control → {val}");
                            cx.notify();
                        });
                    })
            }),
            status: String::new(),
        }
    }
}

fn settings_row(label: &str, control: impl IntoElement) -> impl IntoElement {
    HStack::new()
        .justify_between()
        .items_center()
        .h(px(36.0))
        .px(px(12.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(TEXT_PRIMARY))
                .child(label.to_string()),
        )
        .child(div().w(px(200.0)).child(control))
}

fn section_header(title: &str) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgba(TEXT_PRIMARY))
        .pb(px(4.0))
        .mb(px(4.0))
        .border_b_1()
        .border_color(rgba(BORDER_COLOR))
        .child(title.to_string())
}

impl Render for SettingsDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgba(0x09090BFF))
            .text_color(rgba(TEXT_PRIMARY))
            .child(
                VStack::new()
                    .p(px(20.0))
                    .gap(px(16.0))
                    .max_w(px(500.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Video Settings"),
                    )
                    .child(
                        VStack::new()
                            .bg(rgba(0x18181BFF))
                            .border_1()
                            .border_color(rgba(BORDER_COLOR))
                            .rounded(px(4.0))
                            .p(px(12.0))
                            .gap(px(2.0))
                            .child(section_header("Encoder"))
                            .child(settings_row("Encoder", self.encoder.clone()))
                            .child(settings_row("Rate Control", self.rate_control.clone()))
                            .child(settings_row("Preset", self.preset.clone()))
                    )
                    .child(
                        VStack::new()
                            .bg(rgba(0x18181BFF))
                            .border_1()
                            .border_color(rgba(BORDER_COLOR))
                            .rounded(px(4.0))
                            .p(px(12.0))
                            .gap(px(2.0))
                            .child(section_header("Output"))
                            .child(settings_row("Resolution", self.resolution.clone()))
                            .child(settings_row("Frame Rate", self.fps.clone()))
                    )
                    .when(!self.status.is_empty(), |d| {
                        d.child(
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .bg(rgba(0x18181BFF))
                                .rounded(px(4.0))
                                .text_size(px(12.0))
                                .text_color(rgba(TEXT_DIM))
                                .child(self.status.clone()),
                        )
                    })
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

            let bounds = Bounds::centered(None, size(px(550.0), px(450.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Settings Select Test".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| SettingsDemo::new(cx)),
            )
            .unwrap();
        });
}
