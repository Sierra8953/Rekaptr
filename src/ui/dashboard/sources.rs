use super::*;
use crate::ui::RekaptrWorkspace;

impl RekaptrWorkspace {
    // ── sources list (dense table) ─────────────────────────────────────────────
    pub(super) fn render_sources_list(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let global_recording = self.app_state.recording.phase.lock().is_recording();
        let total = self.app_state.manual_sessions.len() + 1; // + monitor

        // Build row data (monitor, then sessions), filtered by the search query
        // and resolving icon + stats.
        let query = self.sources.search_input.read(cx).content().trim().to_lowercase();
        let matches = |title: &str| query.is_empty() || title.to_lowercase().contains(&query);

        let mut data: Vec<SrcRow> = Vec::with_capacity(total);
        let monitor_selected = self.selected_source.as_deref() == Some("monitor");
        if matches("Monitor") {
            data.push(self.build_src_row("monitor", "Monitor", "Display", "Record entire desktop", false, monitor_selected, monitor_selected && global_recording, cx));
        }

        let sessions: Vec<(String, bool)> = self
            .app_state
            .manual_sessions
            .iter()
            .map(|s| (s.value().title.clone(), s.value().auto_record))
            .collect();
        for (title, auto) in sessions {
            if !matches(&title) {
                continue;
            }
            let selected = self.selected_source.as_deref() == Some(title.as_str());
            let subtitle = if auto { "Auto-record on launch" } else { "Manual capture" };
            data.push(self.build_src_row(&title, &title, "Game", subtitle, auto, selected, selected && global_recording, cx));
        }

        // Sort by most recently recorded first; sources that have never been
        // recorded (no segment mtime) fall to the bottom.
        data.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

        // Rows live in a scroll area sized to fit up to 3 rows (each a fixed
        // 64px). A 4th row makes the area scroll (with a visible scrollbar)
        // rather than growing the table; fewer rows shrink it to fit. The scroll
        // handle + scrollbar state are persisted on the workspace so the offset
        // survives re-renders.
        const ROW_H: f32 = 64.0;
        let rows_h = (data.len().clamp(1, 3) as f32) * ROW_H;
        let mut content = div().w_full().flex().flex_col();
        let last_idx = data.len().saturating_sub(1);
        for (i, row) in data.iter().enumerate() {
            content = content.child(self.source_row(row, i == last_idx, cx));
        }
        // ── custom scrollbar geometry (dim-purple thumb, slightly inset track,
        // visible only while the box is hovered or the thumb is being dragged) ──
        const TRACK_INSET: f32 = 16.0; // shortens the track at top & bottom
        const THUMB_W: f32 = 6.0;
        const MIN_THUMB: f32 = 28.0;
        const MAX_THUMB: f32 = 40.0; // caps the thumb length so the bar stays short
        let content_h = data.len() as f32 * ROW_H;
        let needs_scroll = content_h > rows_h + 0.5;
        let max_off = (content_h - rows_h).max(0.0);
        let track_len = (rows_h - TRACK_INSET * 2.0).max(0.0);
        let thumb_len = (rows_h / content_h * track_len)
            .clamp(MIN_THUMB.min(track_len), MAX_THUMB.min(track_len));
        let scroll = (-self.sources.scroll_handle.offset().y.0).clamp(0.0, max_off);
        let frac = if max_off > 0.0 { scroll / max_off } else { 0.0 };
        let thumb_top = TRACK_INSET + frac * (track_len - thumb_len);
        let show_thumb = self.sources.box_hovered || self.sources.scrollbar_dragging;
        // dim purple; brighter while dragging, hidden when the box isn't hovered
        let thumb_alpha = if self.sources.scrollbar_dragging { 0.85 } else { 0.55 };
        let thumb_color = hsla(258.0 / 360.0, 0.5, 0.62, thumb_alpha);

        let view = cx.entity().downgrade();
        let scroll_area = div()
            .w_full()
            .h(px(rows_h))
            .relative()
            .child(
                div()
                    .id("sources-scroll")
                    .track_scroll(&self.sources.scroll_handle)
                    .overflow_y_scroll()
                    .relative()
                    .size_full()
                    .child(content),
            )
            .when(needs_scroll, |area| {
                area
                    // capture the scroll area's window-space rect for drag mapping
                    .child(
                        canvas(
                            move |_, _, _| {},
                            move |bounds, _, _, cx| {
                                let _ = view.update(cx, |this, _| this.sources.track_bounds = bounds);
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .when(show_thumb, |area| {
                        area.child(
                            div()
                                .id("sources-scrollthumb")
                                .absolute()
                                .top(px(thumb_top))
                                .right(px(3.0))
                                .w(px(THUMB_W))
                                .h(px(thumb_len))
                                .rounded_full()
                                .bg(thumb_color)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                                    this.sources.scrollbar_dragging = true;
                                    cx.stop_propagation();
                                    cx.notify();
                                })),
                        )
                    })
            });

        // Search box + Add-source button form a bar that lives at the top of the
        // table card, sitting directly on top of the rows. (No "Sources" title or
        // sort dropdown — removed to free vertical room for the preview pane.)
        let controls_bar = div()
            .w_full()
            .px_4()
            .py_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(theme.tokens.border)
            // search box (takes the remaining width)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(Input::new(&self.sources.search_input).placeholder("Search sources")),
            )
            // add source (functional)
            .child(
                div()
                    .id("add-source-btn")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(32.0))
                    .flex_shrink_0()
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(theme.tokens.background)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .hover(|s| s.border_color(theme.tokens.primary).bg(theme.tokens.accent))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                        this.add_source.modal_open = true;
                        this.refresh_available_windows(cx);
                        cx.notify();
                    }))
                    .child(Icon::new("plus").size(px(15.0)).color(theme.tokens.muted_foreground))
                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.tokens.foreground).child("Add source")),
            );

        let table = div()
            .w_full()
            .bg(theme.tokens.card)
            .rounded_xl()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .child(controls_bar)
            .child(list_header())
            .child(scroll_area);

        // Drag geometry snapshot for mapping cursor-Y → scroll offset.
        let drag_handle = self.sources.scroll_handle.clone();
        let (drag_track_len, drag_thumb_len, drag_max_off) = (track_len, thumb_len, max_off);

        div()
            .id("sources-box")
            .w_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            // Pop the thumb up whenever the cursor is anywhere over the box.
            .on_hover(cx.listener(|this: &mut Self, hovered: &bool, _, cx| {
                if this.sources.box_hovered != *hovered {
                    this.sources.box_hovered = *hovered;
                    cx.notify();
                }
            }))
            // Continue/finish a thumb drag from anywhere within the box.
            .on_mouse_move(cx.listener(move |this: &mut Self, ev: &MouseMoveEvent, _, cx| {
                if !this.sources.scrollbar_dragging || drag_max_off <= 0.0 {
                    return;
                }
                let track_top = this.sources.track_bounds.origin.y.0 + TRACK_INSET;
                let usable = (drag_track_len - drag_thumb_len).max(1.0);
                let frac = ((ev.position.y.0 - track_top - drag_thumb_len / 2.0) / usable).clamp(0.0, 1.0);
                drag_handle.set_offset(gpui::point(px(0.0), px(-(frac * drag_max_off))));
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this: &mut Self, _, _, cx| {
                if this.sources.scrollbar_dragging {
                    this.sources.scrollbar_dragging = false;
                    cx.notify();
                }
            }))
            .child(table)
    }

    /// Resolve a source's icon + on-disk stats + clip count and pack them into a
    /// renderable row.
    #[allow(clippy::too_many_arguments)]
    fn build_src_row(&self, key: &str, title: &str, kind: &'static str, subtitle: &'static str, auto: bool, selected: bool, recording: bool, cx: &mut Context<Self>) -> SrcRow {
        let icon = self.ensure_source_icon(key, title, cx);
        let stats = self.ensure_source_stats(key, cx);
        let clips = stats.clip_count;
        SrcRow {
            key: key.to_string(),
            title: title.to_string(),
            kind,
            subtitle,
            auto,
            selected,
            recording,
            icon,
            captured: fmt_dur(stats.total_secs),
            on_disk: fmt_size(stats.disk_bytes),
            clips: clips.to_string(),
            last: fmt_ago(stats.last_modified),
            last_modified: stats.last_modified,
        }
    }

    /// One row of the sources table — a 1:1 port of the mockup's `list_row`.
    fn source_row(&self, r: &SrcRow, last: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let dir = crate::utils::get_storage_root().join(crate::utils::clean_title(&r.title));
        let key_for_load = r.key.clone();
        let key_for_settings = r.key.clone();
        let key_for_play = r.key.clone();

        div()
            .id(SharedString::from(format!("src-row-{}", r.key)))
            .relative()
            .w_full()
            .h(px(64.0))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .cursor_pointer()
            .when(r.selected, |el| el.bg(theme.tokens.accent))
            .when(!last, |el| el.border_b_1().border_color(theme.tokens.border))
            .hover(|s| s.bg(theme.tokens.accent))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this: &mut Self, _, window, cx| {
                this.selected_source = Some(key_for_load.clone());
                this.load_video(&key_for_load, window, cx);
                cx.notify();
            }))
            // selection accent bar
            .when(r.selected, |el| {
                el.child(div().absolute().top_0().bottom_0().left_0().w(px(3.0)).bg(theme.tokens.primary))
            })
            // SOURCE (flex)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .min_w(px(0.0))
                    .child(source_avatar(r.icon.clone(), &r.title, r.recording))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.title.clone()))
                                    .child(kind_tag(r.kind))
                                    .when(r.auto, |el| el.child(auto_chip())),
                            )
                            .child(div().text_xs().text_color(theme.tokens.muted_foreground).child(r.subtitle)),
                    ),
            )
            // STATUS
            .child(div().w(px(104.0)).flex().items_center().child(status_chip(r.recording)))
            // ACTIVITY
            .child(div().w(px(110.0)).flex().items_center().child(sparkline(&spark_pattern(&r.title), r.recording)))
            // CAPTURED
            .child(div().w(px(74.0)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.captured.clone()))
            // ON DISK
            .child(div().w(px(78.0)).text_sm().text_color(theme.tokens.muted_foreground).child(r.on_disk.clone()))
            // CLIPS
            .child(div().w(px(50.0)).text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.foreground).child(r.clips.clone()))
            // LAST
            .child(div().w(px(74.0)).text_sm().text_color(theme.tokens.muted_foreground).child(r.last.clone()))
            // quick actions
            .child(
                div()
                    .w(px(100.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(ghost_action("circle-play", SharedString::from(format!("src-play-{}", r.key)), cx.listener(move |this: &mut Self, _, window, cx| {
                        cx.stop_propagation();
                        this.selected_source = Some(key_for_play.clone());
                        this.load_video(&key_for_play, window, cx);
                        cx.notify();
                    })))
                    .child(ghost_action("folder", SharedString::from(format!("src-folder-{}", r.key)), cx.listener(move |_this: &mut Self, _, _, cx| {
                        cx.stop_propagation();
                        if dir.exists() {
                            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                        }
                    })))
                    .child(ghost_action("settings", SharedString::from(format!("src-settings-{}", r.key)), cx.listener(move |this: &mut Self, _, _, cx| {
                        cx.stop_propagation();
                        this.open_source_settings(&key_for_settings, cx);
                    }))),
            )
    }

    /// Square Steam game-icon path for a source, resolved from the local Steam
    /// librarycache and cached. `None` while resolving / when unavailable (caller
    /// falls back to a letter tile). Mirrors the mockup's clienticon avatar.
    fn ensure_source_icon(&self, key: &str, title: &str, cx: &mut Context<Self>) -> Option<std::path::PathBuf> {
        if key == "monitor" {
            return None; // not a Steam game — letter tile
        }
        let title = title.to_string();
        if let Some(p) = self.app_state.icon_cache.get(&title).and_then(|v| v.value().clone()) {
            return Some(std::path::PathBuf::from(p));
        }
        if self.app_state.icon_cache.contains_key(&title) {
            return None; // resolving (or none)
        }
        self.app_state.icon_cache.insert(title.clone(), None);
        let app_state = self.app_state.clone();
        let handle = cx.weak_entity();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let resolved = cx
                    .background_executor()
                    .spawn({
                        let title = title.clone();
                        async move { crate::utils::find_steam_icon(&title) }
                    })
                    .await;
                if let Some(path) = resolved {
                    app_state.icon_cache.insert(title, Some(path.to_string_lossy().replace('\\', "/")));
                    let _ = handle.update(&mut cx, |_, cx| cx.notify());
                }
            }
        })
        .detach();
        None
    }

    /// On-disk stats for a source, cached in `AppState::source_stats`. Kicks off
    /// a one-time background scan on first sight (returns zeros until it lands).
    fn ensure_source_stats(&self, key: &str, cx: &mut Context<Self>) -> crate::utils::SourceStats {
        if let Some(s) = self.app_state.source_stats.get(key) {
            return *s.value();
        }
        self.app_state.source_stats.insert(key.to_string(), crate::utils::SourceStats::default());
        let app_state = self.app_state.clone();
        let handle = cx.weak_entity();
        let key = key.to_string();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let scan_key = key.clone();
                let stats = cx
                    .background_executor()
                    .spawn(async move { crate::utils::source_stats(&scan_key) })
                    .await;
                app_state.source_stats.insert(key, stats);
                let _ = handle.update(&mut cx, |_, cx| cx.notify());
            }
        })
        .detach();
        crate::utils::SourceStats::default()
    }

    /// Load the advanced-settings form state for a source and open the dialog.
    /// Shared by the mixer header/footer and the source-row settings button.
    pub fn open_source_settings(&mut self, source: &str, cx: &mut Context<Self>) {
        self.add_source.advanced_source = Some(source.to_string());
        self.refresh_available_windows(cx);
        self.add_source.overlay_enabled = None;

        let config = crate::config::AppConfig::load();
        if source == "monitor" {
            let v = &config.global_video;
            self.add_source.encoder = v.encoder.clone();
            self.add_source.rate_control = v.rate_control_index;
            self.add_source.bitrate = v.bitrate_kbps;
            self.add_source.cq = v.cq_level;
            self.add_source.retention = v.retention_minutes;
            self.add_source.resolution = v.resolution.clone();
            self.add_source.fps = v.fps;
            self.add_source.gop = v.gop_size;
            self.add_source.bframes = v.bframes;
            self.add_source.preset = v.preset.clone();
            self.add_source.zero_latency = v.zero_latency;
            self.add_source.lookahead = v.lookahead;
            self.add_source.lookahead_frames = v.lookahead_frames;
            self.add_source.spatial_aq = v.spatial_aq;
            self.add_source.temporal_aq = v.temporal_aq;
            self.add_source.audio_tracks = config.global_audio_tracks.clone();
        } else if let Some(settings) = config.game_registry.get(source) {
            if let Some(video) = &settings.video_overrides {
                self.add_source.encoder = video.encoder.clone();
                self.add_source.rate_control = video.rate_control_index;
                self.add_source.bitrate = video.bitrate_kbps;
                self.add_source.cq = video.cq_level;
                self.add_source.resolution = video.resolution.clone();
                self.add_source.fps = video.fps;
                self.add_source.retention = video.retention_minutes;
                self.add_source.gop = video.gop_size;
                self.add_source.bframes = video.bframes;
                self.add_source.preset = video.preset.clone();
                self.add_source.zero_latency = video.zero_latency;
                self.add_source.lookahead = video.lookahead;
                self.add_source.lookahead_frames = video.lookahead_frames;
                self.add_source.spatial_aq = video.spatial_aq;
                self.add_source.temporal_aq = video.temporal_aq;
            }
            if let Some(audio) = &settings.audio_routing {
                self.add_source.audio_tracks = audio.clone();
            } else {
                self.add_source.audio_tracks = config.global_audio_tracks.clone();
            }
            self.add_source.auto_record = settings.auto_record;
            self.add_source.overlay_enabled = settings.overlay_enabled;
        }
        self.add_source.active_tab = 0;
        self.add_source.editing_track_index = None;
        cx.notify();
    }
}

/// Packed, render-ready data for one sources-table row.
struct SrcRow {
    key: String,
    title: String,
    kind: &'static str,
    subtitle: &'static str,
    auto: bool,
    selected: bool,
    recording: bool,
    icon: Option<std::path::PathBuf>,
    captured: String,
    on_disk: String,
    clips: String,
    last: String,
    /// Most-recent recording mtime, used to sort rows by recency.
    last_modified: Option<std::time::SystemTime>,
}

fn avatar_tint(title: &str) -> u32 {
    const PALETTE: &[u32] = &[
        0x8b5cf6, 0x22d3ee, 0x4ade80, 0xf472b6, 0x60a5fa, 0xfbbf24,
    ];
    let idx = title.as_bytes().first().copied().unwrap_or(0) as usize % PALETTE.len();
    PALETTE[idx]
}

/// 42px square avatar — the Steam game icon if resolved, else a gradient letter
/// tile. A red ring is drawn while recording. 1:1 with the mockup's `avatar()`.
fn source_avatar(icon: Option<std::path::PathBuf>, title: &str, recording: bool) -> AnyElement {
    let ring = |d: Div| {
        d.child(
            div()
                .absolute()
                .inset(px(-3.0))
                .rounded_lg()
                .border_2()
                .border_color(gpui::rgba(0xef4444_cc)),
        )
    };
    if let Some(path) = icon {
        let mut d = div().relative().size(px(42.0)).flex_shrink_0().child(
            img(path)
                .size_full()
                .rounded_lg()
                .border_1()
                .border_color(gpui::rgba(0xffffff_12))
                .shadow_md()
                .object_fit(ObjectFit::Cover),
        );
        if recording {
            d = ring(d);
        }
        d.into_any_element()
    } else {
        let tint = avatar_tint(title);
        let letter = title.chars().next().unwrap_or('?').to_uppercase().to_string();
        let grad = gpui::linear_gradient(
            155.0,
            gpui::linear_color_stop(gpui::rgba((tint << 8) | 0xff), 0.0),
            gpui::linear_color_stop(gpui::rgba((tint << 8) | 0x80), 1.0),
        );
        let mut d = div()
            .relative()
            .size(px(42.0))
            .rounded_lg()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(grad)
            .border_1()
            .border_color(gpui::rgba(0xffffff_12))
            .shadow_md()
            .text_base()
            .font_weight(FontWeight::BLACK)
            .text_color(gpui::white())
            .child(letter);
        if recording {
            d = ring(d);
        }
        d.into_any_element()
    }
}

/// Deterministic decorative activity sparkline pattern per source title. There
/// is no real per-source activity time-series, so this is stable visual filler
/// (varied but reproducible), not measured data.
fn spark_pattern(title: &str) -> [f32; 12] {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset
    for b in title.as_bytes() {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    let mut out = [0.0f32; 12];
    for (i, v) in out.iter_mut().enumerate() {
        h ^= h >> 33;
        h = h
            .wrapping_mul(0xff51afd7ed558ccd)
            .wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        *v = 0.15 + ((h >> 24) % 1000) as f32 / 1000.0 * 0.85;
    }
    out
}

fn sparkline(vals: &[f32; 12], recording: bool) -> impl IntoElement {
    let theme = use_theme();
    let color = if recording { theme.tokens.primary } else { gpui::rgb(0x52525b).into() };
    let mut row = div().flex().items_end().gap(px(2.0)).h(px(26.0));
    for &v in vals {
        row = row.child(div().w(px(5.0)).h(px(4.0 + v * 22.0)).rounded_sm().bg(color.opacity(0.8)));
    }
    row
}

fn kind_tag(kind: &'static str) -> impl IntoElement {
    let theme = use_theme();
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded_md()
        .bg(theme.tokens.background)
        .border_1()
        .border_color(theme.tokens.border)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.tokens.muted_foreground)
        .child(kind)
}

fn auto_chip() -> impl IntoElement {
    let theme = use_theme();
    div()
        .flex()
        .items_center()
        .gap_1()
        .px(px(6.0))
        .py(px(1.0))
        .rounded_md()
        .bg(theme.tokens.primary.opacity(0.14))
        .border_1()
        .border_color(theme.tokens.primary.opacity(0.33))
        .child(Icon::new("zap").size(px(10.0)).color(theme.tokens.primary))
        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.primary).child("Auto"))
}

fn ghost_action(
    name: &'static str,
    id: SharedString,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = use_theme();
    div()
        .id(id)
        .size(px(28.0))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(gpui::rgba(0xffffff_06))
        .border_1()
        .border_color(theme.tokens.border)
        .hover(|s| s.bg(theme.tokens.muted).border_color(theme.tokens.border))
        .child(Icon::new(name).size(px(14.0)).color(theme.tokens.muted_foreground))
        .on_mouse_down(MouseButton::Left, on_down)
}

fn list_header() -> Div {
    let theme = use_theme();
    let col = |label: &'static str, w: f32| {
        div()
            .w(px(w))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.tokens.muted_foreground)
            .child(label)
    };
    div()
        .w_full()
        .h(px(34.0))
        .px_4()
        .flex()
        .items_center()
        .bg(theme.tokens.background)
        .border_b_1()
        .border_color(theme.tokens.border)
        .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.tokens.muted_foreground).child("SOURCE"))
        .child(col("STATUS", 104.0))
        .child(col("ACTIVITY", 110.0))
        .child(col("CAPTURED", 74.0))
        .child(col("ON DISK", 78.0))
        .child(col("CLIPS", 50.0))
        .child(col("LAST", 74.0))
        .child(div().w(px(100.0)))
}

fn status_chip(recording: bool) -> Div {
    let (label, dot, fg, bg, br) = if recording {
        ("Recording", 0xef4444u32, 0xfecaca, 0xef4444_2e_u32, 0xef4444_66u32)
    } else {
        ("Idle", 0x4ade80u32, 0xbbf7d0, 0x4ade80_24u32, 0x4ade80_55u32)
    };
    div()
        .h(px(22.0))
        .px_2()
        .rounded_full()
        .flex()
        .items_center()
        .gap_2()
        .bg(rgba(bg))
        .border_1()
        .border_color(rgba(br))
        .child(div().size(px(6.0)).rounded_full().bg(rgb(dot)))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(fg))
                .child(label),
        )
}

fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else if bytes == 0 {
        "—".to_string()
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_dur(secs: f64) -> String {
    if secs <= 0.0 {
        return "—".to_string();
    }
    let h = secs / 3600.0;
    if h >= 1.0 {
        format!("{:.1}h", h)
    } else if secs >= 60.0 {
        format!("{:.0}m", secs / 60.0)
    } else {
        format!("{:.0}s", secs)
    }
}

fn fmt_ago(t: Option<std::time::SystemTime>) -> String {
    let Some(t) = t else { return "—".to_string() };
    let s = t.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    if s < 60 {
        "now".to_string()
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else if s < 172800 {
        "yesterday".to_string()
    } else {
        format!("{}d ago", s / 86400)
    }
}
