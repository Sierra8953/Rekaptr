use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct SelectItem {
    pub id: SharedString,
    pub label: SharedString,
}

pub struct AppSelect {
    focus_handle: FocusHandle,
    items: Vec<SelectItem>,
    selected: Option<usize>,
    open: bool,
    bounds: Bounds<Pixels>,
    on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync>>,
}

impl AppSelect {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
            selected: None,
            open: false,
            bounds: Bounds::default(),
            on_change: None,
        }
    }

    pub fn items(mut self, items: Vec<(&str, &str)>) -> Self {
        self.items = items
            .into_iter()
            .map(|(id, label)| SelectItem {
                id: SharedString::from(id.to_string()),
                label: SharedString::from(label.to_string()),
            })
            .collect();
        self
    }

    pub fn selected_index(mut self, idx: usize) -> Self {
        self.selected = Some(idx);
        self
    }

    pub fn on_change(
        mut self,
        f: impl Fn(&str, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    pub fn selected_label(&self) -> &str {
        self.selected
            .and_then(|i| self.items.get(i))
            .map(|item| item.label.as_ref())
            .unwrap_or("Select...")
    }

    #[allow(dead_code)]
    pub fn set_selected_by_id(&mut self, id: &str) {
        self.selected = self.items.iter().position(|item| item.id.as_ref() == id);
    }
}

impl Render for AppSelect {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
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
            .bg(theme.tokens.card)
            .border_1()
            .border_color(if open {
                theme.tokens.primary
            } else {
                theme.tokens.border
            })
            .rounded(px(3.0))
            .text_color(if has_selection {
                theme.tokens.foreground
            } else {
                theme.tokens.muted_foreground
            })
            .text_size(px(13.0))
            .cursor_pointer()
            .when(!open, |d| {
                d.hover(|s| {
                    s.bg(theme.tokens.accent)
                        .border_color(theme.tokens.muted_foreground)
                })
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.open = !this.open;
                    if this.open {
                        window.focus(&this.focus_handle);
                    }
                    cx.notify();
                }),
            )
            .child(
                div()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(selected_label),
            )
            .child(
                Icon::new(IconSource::Named(
                    if open { "chevron-up" } else { "chevron-down" }.into(),
                ))
                .size(px(14.0))
                .color(theme.tokens.muted_foreground.into()),
            )
            .child({
                let entity = cx.entity().clone();
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            this.bounds = bounds;
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            });

        let items_for_panel: Vec<(usize, SelectItem)> =
            self.items.iter().cloned().enumerate().collect();
        let current_selected = self.selected;

        div()
            .relative()
            .w_full()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.open {
                    this.open = false;
                    cx.notify();
                }
            }))
            .child(trigger)
            .when(open, |d| {
                d.child(
                    deferred(
                        anchored()
                            .snap_to_window_with_margin(Edges::all(px(4.0)))
                            .child(
                                div().occlude().w(bounds.size.width).child(
                                    div()
                                        .occlude()
                                        .mt(px(2.0))
                                        .bg(theme.tokens.card)
                                        .border_1()
                                        .border_color(theme.tokens.border)
                                        .rounded(px(3.0))
                                        .shadow_md()
                                        .overflow_hidden()
                                        .py(px(2.0))
                                        .id("select-panel")
                                        .max_h(px(200.0))
                                        .overflow_y_scroll()
                                        .children(items_for_panel.into_iter().map(
                                            |(idx, item)| {
                                                let is_selected =
                                                    current_selected == Some(idx);
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "item-{idx}"
                                                    )))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(6.0))
                                                    .h(px(26.0))
                                                    .px(px(8.0))
                                                    .text_size(px(13.0))
                                                    .cursor_pointer()
                                                    .when(is_selected, |d| {
                                                        d.bg(theme.tokens.primary)
                                                            .text_color(gpui::white())
                                                    })
                                                    .when(!is_selected, |d| {
                                                        d.text_color(theme.tokens.foreground)
                                                            .hover(|s| {
                                                                s.bg(theme.tokens.accent)
                                                            })
                                                    })
                                                    .when(is_selected, |d| {
                                                        d.child(
                                                            Icon::new(IconSource::Named(
                                                                "check".into(),
                                                            ))
                                                            .size(px(12.0))
                                                            .color(gpui::white()),
                                                        )
                                                    })
                                                    .when(!is_selected, |d| {
                                                        d.child(div().w(px(12.0)))
                                                    })
                                                    .child(item.label.clone())
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |this,
                                                                  _,
                                                                  window,
                                                                  cx| {
                                                                this.selected = Some(idx);
                                                                this.open = false;
                                                                cx.notify();
                                                                if let Some(cb) =
                                                                    &this.on_change
                                                                {
                                                                    if let Some(item) =
                                                                        this.items.get(idx)
                                                                    {
                                                                        (cb)(
                                                                            item.id.as_ref(),
                                                                            window,
                                                                            cx,
                                                                        );
                                                                    }
                                                                }
                                                            },
                                                        ),
                                                    )
                                            },
                                        )),
                                ),
                            ),
                    )
                    .with_priority(1),
                )
            })
    }
}
