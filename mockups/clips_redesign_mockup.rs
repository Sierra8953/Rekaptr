// Clips page redesign mockup.
//
// Direction: replace the current flat masonry grid with a "Library" layout —
// a slim left rail for game/filter navigation, a featured hero for the latest
// clip, and Netflix-style horizontal carousels grouped by game. Cards are
// denser (thumbnail-dominant, duration badge, inline title).
//
// Self-contained: no real file I/O, no real video playback. All data is mocked.

use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::icon_source::IconSource;
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::InputState;
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

// ── Theme ───────────────────────────────────────────────────────────
const BG: u32 = 0x09090BFF;
const SURFACE: u32 = 0x121215FF;
const CARD: u32 = 0x18181BFF;
const CARD_HOVER: u32 = 0x22222AFF;
const BORDER: u32 = 0x2A2A30FF;
const BORDER_STRONG: u32 = 0x3F3F46FF;
const PRIMARY: u32 = 0x8B5CF6FF;
const FG: u32 = 0xFAFAFAFF;
const FG_MUTED: u32 = 0xA1A1AAFF;
const FG_SUBTLE: u32 = 0x71717AFF;
const ACCENT: u32 = 0xFBBF24FF; // favorite star

// ── Mock data ───────────────────────────────────────────────────────
#[derive(Clone)]
struct MockClip {
    id: u64,
    title: String,
    game: String,
    date: String,
    duration: String,
    size: String,
    favorited: bool,
    // Colour used as a placeholder "thumbnail" so the mockup doesn't need
    // real image files.
    thumb_tint: u32,
}

#[derive(Clone, PartialEq, Eq)]
enum Filter {
    All,
    Favorites,
    Recent,
    Game(String),
}

fn mock_clips() -> Vec<MockClip> {
    let base = [
        ("Counter-Strike 2", 0x6E7F66FF),
        ("Elden Ring", 0x8B6F3FFF),
        ("Helldivers 2", 0x3F5B8BFF),
        ("Baldur's Gate 3", 0x6B3A5BFF),
    ];
    let mut out = Vec::new();
    let mut id = 0u64;
    for (game, tint) in base {
        let count = if game == "Counter-Strike 2" { 7 } else { 4 };
        for i in 0..count {
            id += 1;
            let fav = (id % 5) == 0;
            out.push(MockClip {
                id,
                title: match game {
                    "Counter-Strike 2" => [
                        "Clutch 1v4 on Mirage",
                        "Ace at B site",
                        "Deagle headshot streak",
                        "Ninja defuse",
                        "AWP triple-kill",
                        "Wallbang through smoke",
                        "Pistol round win",
                    ][i % 7].to_string(),
                    "Elden Ring" => [
                        "Malenia phase 2 clear",
                        "Parry boss no-hit",
                        "Invasion counter",
                        "Mohg solo",
                    ][i % 4].to_string(),
                    "Helldivers 2" => [
                        "Extraction under fire",
                        "Stratagem chain kill",
                        "Bug breach hold",
                        "Automaton base assault",
                    ][i % 4].to_string(),
                    "Baldur's Gate 3" => [
                        "Surprise round win",
                        "Honour mode boss",
                        "Perfect critical stacking",
                        "Dialogue skill check",
                    ][i % 4].to_string(),
                    _ => format!("Clip {}", i + 1),
                },
                game: game.to_string(),
                date: format!("Apr {}, 2026", 5 + (id as u32 % 15)),
                duration: match id % 4 {
                    0 => "0:42".into(),
                    1 => "1:18".into(),
                    2 => "0:24".into(),
                    _ => "2:06".into(),
                },
                size: format!("{:.1} MB", 42.0 + (id as f32 * 13.7) % 220.0),
                favorited: fav,
                thumb_tint: tint,
            });
        }
    }
    out
}

// ── Workspace ───────────────────────────────────────────────────────
struct ClipsMockup {
    clips: Vec<MockClip>,
    search: Entity<InputState>,
    search_expanded: bool,
    filter: Filter,
    selected_clip: Option<u64>,
    hero_index: usize,
}

impl ClipsMockup {
    fn new(cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(cx));
        Self {
            clips: mock_clips(),
            search,
            search_expanded: false,
            filter: Filter::All,
            selected_clip: None,
            hero_index: 0,
        }
    }

    fn filtered<'a>(&'a self, cx: &App) -> Vec<&'a MockClip> {
        let q = self.search.read(cx).content().to_lowercase();
        self.clips
            .iter()
            .filter(|c| match &self.filter {
                Filter::All => true,
                Filter::Favorites => c.favorited,
                Filter::Recent => c.id > 10, // pretend the higher-id ones are newer
                Filter::Game(g) => &c.game == g,
            })
            .filter(|c| {
                q.is_empty()
                    || c.title.to_lowercase().contains(&q)
                    || c.game.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn games_with_counts(&self) -> Vec<(String, usize)> {
        let mut seen: Vec<(String, usize)> = Vec::new();
        for c in &self.clips {
            if let Some(entry) = seen.iter_mut().find(|(g, _)| g == &c.game) {
                entry.1 += 1;
            } else {
                seen.push((c.game.clone(), 1));
            }
        }
        seen
    }
}

impl Render for ClipsMockup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .size_full()
            .bg(rgba(BG))
            .text_color(rgba(FG))
            .child(self.render_app_sidebar(cx))
            .child(self.render_left_rail(cx))
            .child(
                VStack::new()
                    .flex_1()
                    .h_full()
                    .child(self.render_top_bar(cx))
                    .child(
                        div()
                            .id("clips-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(
                                VStack::new()
                                    .gap_10()
                                    .pb_10()
                                    .child(self.render_hero(cx))
                                    .child(self.render_filter_chips(cx))
                                    .children(self.render_carousels(cx)),
                            ),
                    ),
            )
    }
}

impl ClipsMockup {
    fn render_app_sidebar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .w(px(72.0))
            .h_full()
            .bg(rgba(CARD))
            .border_r_1()
            .border_color(rgba(BORDER))
            .pt(px(12.0))
            .px(px(8.0))
            .gap_2()
            .child(app_nav_item("nav-dash", "layout-dashboard", false))
            .child(app_nav_item("nav-clips", "video", true))
            .child(app_nav_item("nav-settings", "settings", false))
    }
}

fn app_nav_item(id: &'static str, icon_name: &'static str, active: bool) -> impl IntoElement {
    div()
        .id(id)
        .w_full()
        .h(px(56.0))
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            div()
                .size(px(48.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .bg(if active { rgba(CARD_HOVER) } else { rgba(0x00000000) })
                .hover(|s| s.bg(rgba(CARD_HOVER)))
                .child(
                    Icon::new(IconSource::Named(icon_name.into()))
                        .size(px(22.0))
                        .color(if active { rgba(FG).into() } else { rgba(FG_MUTED).into() }),
                ),
        )
        .when(active, |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(16.0))
                    .w(px(3.0))
                    .h(px(24.0))
                    .rounded_r_sm()
                    .bg(rgba(PRIMARY)),
            )
        })
}

impl ClipsMockup {
    fn render_left_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let games = self.games_with_counts();
        let all_count: usize = games.iter().map(|(_, n)| n).sum();
        let fav_count = self.clips.iter().filter(|c| c.favorited).count();

        VStack::new()
            .w(px(240.0))
            .h_full()
            .bg(rgba(SURFACE))
            .border_r_1()
            .border_color(rgba(BORDER))
            .py_5()
            .px_3()
            .gap_1()
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(FG_SUBTLE))
                    .child("LIBRARY"),
            )
            .child(self.rail_item(cx, "layout-dashboard", "All Clips", all_count, Filter::All))
            .child(self.rail_item(cx, "star", "Favorites", fav_count, Filter::Favorites))
            .child(self.rail_item(cx, "rotate-ccw", "Recent", 8, Filter::Recent))
            .child(
                div()
                    .px_3()
                    .pt_6()
                    .pb_3()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba(FG_SUBTLE))
                    .child("GAMES"),
            )
            .children(
                games
                    .into_iter()
                    .map(|(g, n)| self.rail_item(cx, "gamepad-2", &g, n, Filter::Game(g.clone()))),
            )
    }

    fn rail_item(
        &self,
        cx: &mut Context<Self>,
        icon: &str,
        label: &str,
        count: usize,
        filter: Filter,
    ) -> impl IntoElement {
        let active = self.filter == filter;
        let label_owned = label.to_string();
        let icon = icon.to_string();
        let filter_clone = filter.clone();

        div()
            .id(SharedString::from(format!("rail-{}", label)))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .h(px(36.0))
            .px_3()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { rgba(CARD_HOVER) } else { rgba(0x00000000) })
            .hover(|s| s.bg(rgba(CARD)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.filter = filter_clone.clone();
                    cx.notify();
                }),
            )
            .child(
                Icon::new(IconSource::Named(icon))
                    .size(px(16.0))
                    .color(if active {
                        rgba(PRIMARY).into()
                    } else {
                        rgba(FG_MUTED).into()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .child(label_owned),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(FG_SUBTLE))
                    .child(format!("{}", count)),
            )
    }

    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.filter {
            Filter::All => "All Clips".to_string(),
            Filter::Favorites => "Favorites".to_string(),
            Filter::Recent => "Recent".to_string(),
            Filter::Game(g) => g.clone(),
        };
        let subtitle = format!("{} clips", self.filtered(cx).len());

        HStack::new()
            .px_8()
            .py_5()
            .border_b_1()
            .border_color(rgba(BORDER))
            .justify_between()
            .items_center()
            .child(
                VStack::new()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child(subtitle),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(self.render_search(cx))
                    .child(
                        Button::new("sort", "")
                            .icon(IconSource::Named("chevron-down".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    )
                    .child(
                        Button::new("more", "")
                            .icon(IconSource::Named("settings".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm),
                    ),
            )
    }

    fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.search_expanded {
            HStack::new()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .w(px(280.0))
                        .child(
                            Input::new(&self.search)
                                .placeholder("Search clips..."),
                        ),
                )
                .child(
                    Button::new("search-collapse", "")
                        .icon(IconSource::Named("x".into()))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.search_expanded = false;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            div()
                .id("search-icon")
                .size(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgba(CARD)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.search_expanded = true;
                        cx.notify();
                    }),
                )
                .child(
                    Icon::new(IconSource::Named("search".into()))
                        .size(px(16.0))
                        .color(rgba(FG_MUTED).into()),
                )
                .into_any_element()
        }
    }

    fn render_hero(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let clips = self.filtered(cx);
        if clips.is_empty() {
            return div().into_any_element();
        }
        let clip = clips[self.hero_index.min(clips.len() - 1)].clone();

        div()
            .px_8()
            .pt_8()
            .child(
                div()
                    .w_full()
                    .h(px(320.0))
                    .rounded_xl()
                    .overflow_hidden()
                    .relative()
                    .bg(rgba(clip.thumb_tint))
                    .border_1()
                    .border_color(rgba(BORDER_STRONG))
                    // Gradient scrim (approximated as solid dark fill)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(gpui::rgba(0x00000099)),
                    )
                    .child(
                        VStack::new()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .p_8()
                            .gap_3()
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(rgba(PRIMARY))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgba(FG))
                                            .child("LATEST"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_MUTED))
                                            .child(clip.game.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child("•"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_MUTED))
                                            .child(clip.date.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_SUBTLE))
                                            .child("•"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgba(FG_MUTED))
                                            .child(clip.duration.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(FG))
                                    .child(clip.title.clone()),
                            )
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .pt_2()
                                    .child(
                                        Button::new("hero-play", "Play")
                                            .icon(IconSource::Named("play".into()))
                                            .variant(ButtonVariant::Default)
                                            .size(ButtonSize::Lg)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.selected_clip = Some(clip.id);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("hero-fav", "")
                                            .icon(IconSource::Named(
                                                if clip.favorited { "star" } else { "star" }
                                                    .into(),
                                            ))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Lg),
                                    )
                                    .child(
                                        Button::new("hero-export", "Export")
                                            .icon(IconSource::Named("scissors".into()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Lg),
                                    )
                                    .child(
                                        Button::new("hero-more", "")
                                            .icon(IconSource::Named("settings".into()))
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Lg),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_filter_chips(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let chips: Vec<(Filter, &str)> = vec![
            (Filter::All, "All"),
            (Filter::Recent, "Last 7 Days"),
            (Filter::Favorites, "★ Favorites"),
        ];

        HStack::new()
            .px_8()
            .gap_2()
            .children(chips.into_iter().map(|(filter, label)| {
                let active = self.filter == filter;
                let filter_clone = filter.clone();
                div()
                    .id(SharedString::from(format!("chip-{}", label)))
                    .px_4()
                    .py_2()
                    .rounded_full()
                    .text_sm()
                    .cursor_pointer()
                    .bg(if active { rgba(PRIMARY) } else { rgba(CARD) })
                    .text_color(if active { rgba(FG) } else { rgba(FG_MUTED) })
                    .border_1()
                    .border_color(rgba(if active { PRIMARY } else { BORDER }))
                    .hover(|s| s.bg(rgba(CARD_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.filter = filter_clone.clone();
                            cx.notify();
                        }),
                    )
                    .child(label.to_string())
            }))
    }

    fn render_carousels(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        // Group the filtered clips by game, preserving source order.
        let clips = self.filtered(cx);
        let mut groups: Vec<(String, Vec<MockClip>)> = Vec::new();
        for c in clips {
            if let Some(entry) = groups.iter_mut().find(|(g, _)| g == &c.game) {
                entry.1.push(c.clone());
            } else {
                groups.push((c.game.clone(), vec![c.clone()]));
            }
        }

        groups
            .into_iter()
            .map(|(game, clips)| self.render_carousel(&game, clips, cx).into_any_element())
            .collect()
    }

    fn render_carousel(
        &self,
        game: &str,
        clips: Vec<MockClip>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const VISIBLE: usize = 4;
        let game_owned = game.to_string();
        let count = clips.len();
        let game_for_nav = game_owned.clone();
        let game_for_tile = game_owned.clone();
        let overflow = count.saturating_sub(VISIBLE);
        let visible_clips: Vec<MockClip> = clips.into_iter().take(VISIBLE).collect();

        VStack::new()
            .px_8()
            .gap_3()
            .child(
                HStack::new()
                    .justify_between()
                    .items_end()
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(game_owned.clone()),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgba(CARD))
                                    .text_xs()
                                    .text_color(rgba(FG_MUTED))
                                    .child(format!("{} clips", count)),
                            ),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("view-all-{}", game_owned)),
                            "View all",
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.filter = Filter::Game(game_for_nav.clone());
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div().w_full().child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .children(
                            visible_clips
                                .into_iter()
                                .map(|c| self.render_clip_card(c, cx).into_any_element()),
                        )
                        .when(overflow > 0, |s| {
                            s.child(self.render_view_all_tile(
                                &game_for_tile,
                                overflow,
                                cx,
                            ))
                        }),
                ),
            )
    }

    fn render_view_all_tile(
        &self,
        game: &str,
        overflow: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let game_owned = game.to_string();
        div()
            .id(SharedString::from(format!("view-all-tile-{}", game_owned)))
            .w(px(240.0))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgba(CARD))
            .rounded_lg()
            .border_1()
            .border_color(rgba(BORDER))
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(PRIMARY)).bg(rgba(CARD_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.filter = Filter::Game(game_owned.clone());
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .gap_2()
                    .items_center()
                    .child(
                        Icon::new(IconSource::Named("chevron-right".into()))
                            .size(px(28.0))
                            .color(rgba(PRIMARY).into()),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("+{} more", overflow)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(FG_SUBTLE))
                            .child("View all"),
                    ),
            )
    }

    fn render_clip_card(&self, clip: MockClip, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_clip == Some(clip.id);
        let clip_for_click = clip.clone();

        div()
            .group(SharedString::from(format!("card-{}", clip.id)))
            .id(SharedString::from(format!("clip-{}", clip.id)))
            .w(px(240.0))
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgba(CARD))
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .border_color(rgba(if selected { PRIMARY } else { BORDER }))
            .cursor_pointer()
            .hover(|s| s.border_color(rgba(BORDER_STRONG)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.selected_clip = Some(clip_for_click.id);
                    cx.notify();
                }),
            )
            // ── Thumbnail
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(146.0))
                    .rounded_t_lg()
                    .overflow_hidden()
                    .bg(rgba(clip.thumb_tint))
                    // Favorite star (top-left)
                    .when(clip.favorited, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_2()
                                .left_2()
                                .p_1()
                                .rounded_md()
                                .bg(gpui::rgba(0x00000099))
                                .child(
                                    Icon::new(IconSource::Named("star".into()))
                                        .size(px(14.0))
                                        .color(rgba(ACCENT).into()),
                                ),
                        )
                    })
                    // Duration badge (bottom-right)
                    .child(
                        div()
                            .absolute()
                            .bottom_2()
                            .right_2()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(gpui::rgba(0x000000CC))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgba(FG))
                            .child(clip.duration.clone()),
                    )
                    // Play overlay on hover
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(gpui::rgba(0x00000000))
                            .opacity(0.0)
                            .group_hover(SharedString::from(format!("card-{}", clip.id)), |s| {
                                s.opacity(1.0).bg(gpui::rgba(0x00000066))
                            })
                            .child(
                                div()
                                    .w(px(48.0))
                                    .h(px(48.0))
                                    .rounded_full()
                                    .bg(rgba(PRIMARY))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named("play".into()))
                                            .size(px(22.0))
                                            .color(rgba(FG).into()),
                                    ),
                            ),
                    ),
            )
            // ── Body
            .child(
                VStack::new()
                    .px_3()
                    .py_3()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgba(FG))
                            .child(clip.title.clone()),
                    )
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child(clip.date.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgba(FG_SUBTLE))
                                    .child(clip.size.clone()),
                            ),
                    ),
            )
    }
}

fn main() {
    let app = Application::new().with_assets(Assets {
        base: PathBuf::from("assets"),
    });

    app.run(move |cx| {
        adabraka_ui::set_icon_base_path("icons");
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Clips Redesign Mockup".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ClipsMockup::new),
        )
        .unwrap();
    });
}
