//! Teams view layer: all render_* methods for the Teams tab and the small
//! view helpers (badges, avatars, member stack, labels).

use super::*;
use crate::ui::RekaptrWorkspace;
use adabraka_ui::components::input::Input;

impl RekaptrWorkspace {
    pub fn render_teams(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.teams.signed_in {
            return self.render_teams_signin(cx).into_any_element();
        }
        let show_empty = self.teams.list.is_empty();

        div()
            .size_full()
            .relative()
            .flex()
            .child(self.render_team_rail(cx))
            .child(
                VStack::new()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .when_some(self.teams.error.clone(), |this, msg| {
                        this.child(self.render_teams_error_banner(msg, cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(if show_empty {
                                self.render_teams_empty(cx).into_any_element()
                            } else {
                                self.render_team_feed(cx).into_any_element()
                            }),
                    ),
            )
            .when(self.teams.panel != TeamsPanel::None, |this| {
                this.child(self.render_teams_overlay(cx))
            })
            .when(self.teams.player.is_some(), |this| {
                this.child(self.render_team_player_overlay(window, cx))
            })
            .when(self.teams.clip_menu.is_some(), |this| {
                this.child(self.render_clip_actions_overlay(cx))
            })
            .when(self.teams.rename_target.is_some(), |this| {
                this.child(self.render_rename_overlay(cx))
            })
            .when(self.teams.members_open, |this| {
                this.child(self.render_members_overlay(cx))
            })
            .when(self.teams.account_menu_open, |this| {
                this.child(self.render_account_menu(cx))
            })
            .when(self.teams.comments_open.is_some(), |this| {
                this.child(self.render_comments_overlay(cx))
            })
            .when(self.teams.reaction_picker.is_some(), |this| {
                this.child(self.render_reaction_picker(cx))
            })
            .into_any_element()
    }

    // ── Left rail: the user's teams ─────────────────────────────────
    fn render_team_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        VStack::new()
            .w(px(248.0))
            .h_full()
            .bg(theme.tokens.card.opacity(0.5))
            .border_r_1()
            .border_color(theme.tokens.border)
            .py_5()
            .px_3()
            .gap_1()
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .tracking_wider()
                    .text_color(theme.tokens.muted_foreground.opacity(0.8))
                    .child("YOUR TEAMS"),
            )
            .children(
                self.teams.list
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.team_rail_item(i, t, cx).into_any_element())
                    .collect::<Vec<_>>(),
            )
            .child(div().h(px(8.0)))
            .child(
                div()
                    .id("teams-rail-create")
                    .flex()
                    .items_center()
                    .gap_3()
                    .h(px(44.0))
                    .px_3()
                    .rounded_md()
                    .cursor_pointer()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .hover(|s| s.bg(theme.tokens.muted))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.teams.panel = TeamsPanel::Create;
                            cx.notify();
                        }),
                    )
                    .child(
                        Icon::new(IconSource::Named("plus".into()))
                            .size(px(16.0))
                            .color(theme.tokens.muted_foreground),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.tokens.muted_foreground)
                            .child("Create or join"),
                    ),
            )
            // Push the sign-out control to the bottom of the rail.
            .child(div().flex_1())
            .child(self.render_sign_out_button(cx))
    }

    /// Bottom-rail account control: shows the signed-in user's avatar + name and
    /// opens the account menu (profile / settings / sign out) on click. Falls
    /// back to a generic person icon until the user's profile has loaded.
    fn render_sign_out_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        // Disable the control while *any* cloud op is in flight, but only show
        // "Signing out…" during an actual sign-out — not during the first-load
        // team fetch, which also sets `busy`.
        let busy = self.teams.busy;
        let signing_out = self.teams.signing_out;
        let profile = self.my_profile();
        let open = self.teams.account_menu_open;

        let label: SharedString = if signing_out {
            "Signing out…".into()
        } else {
            profile
                .as_ref()
                .map(|(_, name, _, _)| SharedString::from(name.clone()))
                .unwrap_or_else(|| "Account".into())
        };

        div()
            .id("teams-account")
            .flex()
            .items_center()
            .gap_3()
            .h(px(48.0))
            .px_2()
            .rounded_lg()
            .cursor_pointer()
            .bg(if open { theme.tokens.muted } else { gpui::transparent_black() })
            .hover(|s| s.bg(theme.tokens.muted.opacity(0.6)))
            .when(!busy, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.teams.account_menu_open = true;
                        cx.notify();
                    }),
                )
            })
            .child(match &profile {
                Some((seed, _, initial, tint)) => {
                    avatar(seed, initial, *tint, 34.0).into_any_element()
                }
                None => div()
                    .size(px(34.0))
                    .flex_none()
                    .rounded_full()
                    .bg(theme.tokens.muted)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named("user".into()))
                            .size(px(16.0))
                            .color(theme.tokens.muted_foreground),
                    )
                    .into_any_element(),
            })
            .child(
                VStack::new()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child("Account"),
                    ),
            )
            .child(
                Icon::new(IconSource::Named("chevron-right".into()))
                    .size(px(15.0))
                    .color(theme.tokens.muted_foreground),
            )
    }

    /// Account menu, anchored near the bottom-left account control: view/edit
    /// profile and account settings (deep-link to rekaptr.dev), plus sign out.
    fn render_account_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let profile = self.my_profile();
        let signing_out = self.teams.signing_out;

        div()
            .absolute()
            .inset_0()
            // Click the scrim to dismiss.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.account_menu_open = false;
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .absolute()
                    .left(px(12.0))
                    .bottom(px(72.0))
                    .w(px(244.0))
                    .gap_1()
                    .p_2()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header: who's signed in.
                    .child(
                        HStack::new()
                            .gap_3()
                            .items_center()
                            .px_2()
                            .py_2()
                            .child(match &profile {
                                Some((seed, _, initial, tint)) => {
                                    avatar(seed, initial, *tint, 38.0).into_any_element()
                                }
                                None => avatar("", "?", 0x6b7280, 38.0).into_any_element(),
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child(
                                        profile
                                            .as_ref()
                                            .map(|(_, name, _, _)| name.clone())
                                            .unwrap_or_else(|| "Your account".to_string()),
                                    ),
                            ),
                    )
                    .child(div().h(px(1.0)).bg(theme.tokens.border).mx_1())
                    .child(
                        menu_row("teams-account-profile", "user", "View profile", false)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.teams.account_menu_open = false;
                                    crate::cloud::auth::open_url("https://rekaptr.dev/account");
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        menu_row("teams-account-settings", "settings", "Account settings", false)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.teams.account_menu_open = false;
                                    crate::cloud::auth::open_url("https://rekaptr.dev/account/settings");
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(div().h(px(1.0)).bg(theme.tokens.border).mx_1())
                    .child(
                        menu_row(
                            "teams-account-signout",
                            "log-out",
                            if signing_out { "Signing out…" } else { "Sign out" },
                            true,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.teams.account_menu_open = false;
                                this.sign_out_cloud(cx);
                            }),
                        ),
                    ),
            )
    }

    fn team_rail_item(&self, i: usize, team: &Team, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let active = self.teams.active.unwrap_or(0) == i;
        let online = team.members.iter().filter(|m| m.online).count();
        let unread = self.team_is_unread(team);

        div()
            .id(SharedString::from(format!("team-{}", team.id)))
            .relative()
            .flex()
            .items_center()
            .gap_3()
            .h(px(56.0))
            .px_2()
            .rounded_lg()
            .cursor_pointer()
            .bg(if active { theme.tokens.muted } else { gpui::transparent_black() })
            .hover(|s| s.bg(theme.tokens.muted.opacity(0.5)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.teams.active = Some(i);
                    this.teams.member_filter = None;
                    this.teams.game_filter = None;
                    if let Some(id) = this.teams.list.get(i).map(|t| t.id.clone()) {
                        this.mark_team_seen(&id);
                    }
                    this.load_active_team(cx);
                    cx.notify();
                }),
            )
            .when(active, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(-12.0))
                        .top(px(16.0))
                        .w(px(3.0))
                        .h(px(24.0))
                        .rounded_r_sm()
                        .bg(theme.tokens.primary),
                )
            })
            .child(team_badge(&team.initials, team.tint, 38.0))
            .child(
                VStack::new()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                            .text_color(if active {
                                theme.tokens.foreground
                            } else {
                                theme.tokens.muted_foreground
                            })
                            .child(team.name.clone()),
                    )
                    .child(
                        HStack::new()
                            .gap_1p5()
                            .items_center()
                            .child(div().size(px(6.0)).rounded_full().bg(gpui::rgb(ONLINE)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("{} online · {} members", online, team.members.len())),
                            ),
                    ),
            )
            // Unread dot: activity newer than the last time this team was viewed.
            // Hidden on the active team (viewing it counts as seeing it).
            .when(unread && !active, |this| {
                this.child(
                    div()
                        .size(px(8.0))
                        .flex_none()
                        .rounded_full()
                        .bg(theme.tokens.primary),
                )
            })
    }

    // ── Main: a team's shared-clip feed ─────────────────────────────
    fn render_team_feed(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(team) = self.active_team() else {
            return div().into_any_element();
        };

        let clips = self.feed_clips(team);
        let heading = match self.teams.member_filter {
            Some(m) => format!("{}'s clips", team.members[m].name),
            None => "Shared clips".to_string(),
        };
        let count = clips.len();
        let games = Self::team_games(team);

        VStack::new()
            .h_full()
            .child(self.render_feed_header(team, cx))
            .child(
                div()
                    .id("teams-feed-scroll")
                    .flex_1()
                    .min_w_0()
                    .overflow_y_scroll()
                    .child(
                        VStack::new()
                            .px_8()
                            .py_6()
                            .gap_5()
                            .child(self.render_member_chips(team, cx))
                            // Game filter row, only when there's more than one game
                            // to choose between.
                            .when(games.len() > 1, |this| {
                                this.child(self.render_game_chips(&games, cx))
                            })
                            .child(
                                HStack::new()
                                    .justify_between()
                                    .items_center()
                                    .gap_4()
                                    .child(section_header(&heading, count))
                                    .child(self.render_sort_pills(cx)),
                            )
                            .child(self.render_clip_grid(team, clips, cx)),
                    ),
            )
            .into_any_element()
    }

    fn feed_clips<'a>(&self, team: &'a Team) -> Vec<&'a TeamClip> {
        const WEEK_SECS: i64 = 7 * 24 * 60 * 60;
        let now = chrono::Utc::now().timestamp();
        let mut clips: Vec<&TeamClip> = team
            .clips
            .iter()
            .filter(|c| self.teams.member_filter.map_or(true, |m| c.author == m))
            .filter(|c| self.teams.game_filter.as_deref().map_or(true, |g| c.game == g))
            .filter(|c| match self.teams.sort {
                ClipSort::Week => now - c.created_unix < WEEK_SECS,
                _ => true,
            })
            .collect();
        match self.teams.sort {
            // Most-reacted first (by total reactions), breaking ties by recency.
            ClipSort::Top => {
                let total = |c: &&TeamClip| c.reactions.iter().map(|t| t.count).sum::<u32>();
                clips.sort_by(|a, b| {
                    total(b)
                        .cmp(&total(a))
                        .then(b.created_unix.cmp(&a.created_unix))
                })
            }
            ClipSort::Newest | ClipSort::Week => {
                clips.sort_by(|a, b| b.created_unix.cmp(&a.created_unix))
            }
        }
        clips
    }

    /// Distinct game tags present in a team's clips, in first-seen (feed) order.
    /// Used to build the game-filter chip row.
    fn team_games(team: &Team) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        team.clips
            .iter()
            .filter(|c| !c.game.is_empty())
            .filter(|c| seen.insert(c.game.as_str()))
            .map(|c| c.game.clone())
            .collect()
    }

    fn render_clip_grid(
        &self,
        team: &Team,
        clips: Vec<&TeamClip>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        if clips.is_empty() {
            return div()
                .w_full()
                .py_20()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    VStack::new()
                        .items_center()
                        .gap_3()
                        .child(
                            Icon::new(IconSource::Named("video".into()))
                                .size(px(48.0))
                                .color(theme.tokens.muted_foreground.opacity(0.5)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground)
                                .child("No clips shared here yet"),
                        ),
                )
                .into_any_element();
        }

        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap_5()
            .children(
                clips
                    .into_iter()
                    .map(|c| self.render_team_clip_card(team, c, cx).into_any_element()),
            )
            .into_any_element()
    }

    fn render_feed_header(&self, team: &Team, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        HStack::new()
            .px_8()
            .py_5()
            .border_b_1()
            .border_color(theme.tokens.border)
            .justify_between()
            .items_center()
            .child(
                HStack::new()
                    .gap_4()
                    .items_center()
                    .child(team_badge(&team.initials, team.tint, 48.0))
                    .child(
                        VStack::new()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(team.name.clone()),
                            )
                            // Subtitle: live presence + clip count, with the same
                            // green online dot used in the team rail for continuity.
                            .child(
                                HStack::new()
                                    .gap_1p5()
                                    .items_center()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(div().size(px(6.0)).rounded_full().bg(gpui::rgb(ONLINE)))
                                    .child(format!(
                                        "{} online",
                                        team.members.iter().filter(|m| m.online).count()
                                    ))
                                    .child(
                                        div()
                                            .text_color(theme.tokens.muted_foreground.opacity(0.5))
                                            .child("·"),
                                    )
                                    .child(format!("{} clips shared", team.clips.len())),
                            ),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .id("teams-open-members")
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.open_members_panel(cx)),
                            )
                            .child(render_member_stack(&team.members)),
                    )
                    .child({
                        let label = if self.teams.sharing {
                            format!("Uploading… {}%", (self.teams.share_progress * 100.0) as u32)
                        } else {
                            "Share a clip".to_string()
                        };
                        Button::new("teams-share-clip", label)
                            .icon(IconSource::Named("upload".into()))
                            .variant(ButtonVariant::Default)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.share_clip_to_active_team(cx);
                            }))
                    })
                    .child(
                        Button::new("teams-members", "Members")
                            .icon(IconSource::Named("users".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_members_panel(cx);
                            })),
                    ),
            )
    }

    fn render_member_chips(&self, team: &Team, cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .gap_2()
            .flex_wrap()
            .items_center()
            .child(self.member_chip("All", None, self.teams.member_filter.is_none(), cx))
            .children(team.members.iter().enumerate().map(|(i, m)| {
                self.member_chip(&m.name, Some(i), self.teams.member_filter == Some(i), cx)
                    .into_any_element()
            }))
    }

    fn member_chip(
        &self,
        label: &str,
        member: Option<usize>,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let label_owned = label.to_string();

        div()
            .id(SharedString::from(format!("teams-mchip-{:?}", member)))
            .flex()
            .items_center()
            .px_4()
            .h(px(34.0))
            .rounded_full()
            .text_sm()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
            .cursor_pointer()
            .bg(if active { theme.tokens.primary } else { theme.tokens.card })
            .text_color(if active {
                theme.tokens.primary_foreground
            } else {
                theme.tokens.muted_foreground
            })
            .border_1()
            .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
            .hover(|s| s.bg(if active { theme.tokens.primary } else { theme.tokens.muted }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.teams.member_filter = member;
                    cx.notify();
                }),
            )
            .child(label_owned)
    }

    /// Segmented sort control (Newest / Top / This week) for the feed.
    fn render_sort_pills(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        HStack::new()
            .gap_0p5()
            .p_0p5()
            .rounded_lg()
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .children(ClipSort::ALL.into_iter().map(|sort| {
                let active = self.teams.sort == sort;
                div()
                    .id(SharedString::from(format!("teams-sort-{}", sort.label())))
                    .flex()
                    .items_center()
                    .px_3()
                    .h(px(28.0))
                    .rounded_md()
                    .text_xs()
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                    .cursor_pointer()
                    .bg(if active { theme.tokens.primary } else { gpui::transparent_black() })
                    .text_color(if active {
                        theme.tokens.primary_foreground
                    } else {
                        theme.tokens.muted_foreground
                    })
                    .hover(|s| s.bg(if active { theme.tokens.primary } else { theme.tokens.muted }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.teams.sort = sort;
                            cx.notify();
                        }),
                    )
                    .child(sort.label())
            }))
    }

    fn render_game_chips(&self, games: &[String], cx: &mut Context<Self>) -> impl IntoElement {
        HStack::new()
            .gap_2()
            .flex_wrap()
            .items_center()
            .child(self.game_chip("All games", None, self.teams.game_filter.is_none(), cx))
            .children(games.iter().map(|g| {
                let active = self.teams.game_filter.as_deref() == Some(g.as_str());
                self.game_chip(g, Some(g.clone()), active, cx).into_any_element()
            }))
    }

    fn game_chip(
        &self,
        label: &str,
        value: Option<String>,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let label_owned = label.to_string();

        div()
            .id(SharedString::from(format!("teams-gchip-{}", label)))
            .flex()
            .items_center()
            .px_3()
            .h(px(30.0))
            .rounded_full()
            .text_xs()
            .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
            .cursor_pointer()
            .bg(if active { theme.tokens.primary.opacity(0.15) } else { theme.tokens.card })
            .text_color(if active { theme.tokens.primary } else { theme.tokens.muted_foreground })
            .border_1()
            .border_color(if active { theme.tokens.primary } else { theme.tokens.border })
            .hover(|s| s.border_color(theme.tokens.primary))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.teams.game_filter = value.clone();
                    cx.notify();
                }),
            )
            .child(label_owned)
    }

    fn render_team_clip_card(
        &self,
        team: &Team,
        clip: &TeamClip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        // Borrow the author's display fields; only fall back to literals when
        // missing — no per-card-per-frame allocation in the common (found) path.
        let (author_seed, author_initial, author_name, author_tint) = team
            .members
            .get(clip.author)
            .map(|m| (m.user_id.as_str(), m.initial.as_str(), m.name.as_str(), m.tint))
            .unwrap_or(("", "?", "Unknown", 0x6b7280));
        // All ids/groups derive from the precomputed `el_id` via cheap Arc
        // clones + integer disambiguators — no per-frame `format!` allocation.
        let card_group = clip.el_id.clone();

        div()
            .group(card_group.clone())
            .id((clip.el_id.clone(), 1usize))
            .w(px(284.0))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.tokens.card)
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(theme.tokens.border)
            .cursor_pointer()
            .hover(|s| s.border_color(theme.tokens.primary).shadow_lg())
            // ── Thumbnail (click to play in the mini player)
            .child(
                crate::ui::thumbnail(
                    clip.thumb_url.clone().map(SharedString::from),
                    // Card header: round the top to match the card, leave the
                    // bottom square where it meets the body.
                    gpui::Corners {
                        top_left: px(12.0),
                        top_right: px(12.0),
                        bottom_left: px(0.0),
                        bottom_right: px(0.0),
                    },
                    // Per-clip tint as the placeholder while the clip transcodes.
                    gpui::rgb(clip.thumb_tint).into(),
                )
                    .id((clip.el_id.clone(), 2usize))
                    .h(px(160.0))
                    .when_some(clip.video_url.clone(), |this, url| {
                        let title = clip.title.clone();
                        this.cursor_pointer().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.open_team_clip(url.clone(), title.clone(), window, cx);
                            }),
                        )
                    })
                    // Subtle top-down sheen so the flat tint reads with depth.
                    // Full-bleed overlays must carry the tile's top rounding —
                    // GPUI's content mask is a rectangle and won't round-clip
                    // children, so a square overlay would fill the corners.
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded_tl(px(12.0))
                            .rounded_tr(px(12.0))
                            .bg(gpui::linear_gradient(
                                160.0,
                                gpui::linear_color_stop(gpui::rgba(0xFFFFFF14), 0.0),
                                gpui::linear_color_stop(gpui::rgba(0x00000033), 1.0),
                            )),
                    )
                    .when(clip.new, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_2()
                                .left_2()
                                .px(px(6.0))
                                .py_0p5()
                                .rounded_md()
                                .bg(theme.tokens.primary)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .tracking_wider()
                                .text_color(theme.tokens.primary_foreground)
                                .child("NEW"),
                        )
                    })
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
                            .text_color(gpui::white())
                            .child(clip.duration.clone()),
                    )
                    // "···" actions button (top-right, on hover).
                    .child({
                        let id = clip.id.clone();
                        div()
                            .id((clip.el_id.clone(), 4usize))
                            .absolute()
                            .top_2()
                            .right_2()
                            .size(px(26.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(gpui::rgba(0x000000B3))
                            .cursor_pointer()
                            .opacity(0.0)
                            .group_hover(card_group.clone(), |s| s.opacity(1.0))
                            .hover(|s| s.bg(gpui::rgba(0x000000E6)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.teams.clip_menu = Some(id.clone());
                                    cx.notify();
                                }),
                            )
                            .child(
                                Icon::new(IconSource::Named("ellipsis".into()))
                                    .size(px(16.0))
                                    .color(gpui::white()),
                            )
                    })
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded_tl(px(12.0))
                            .rounded_tr(px(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.0)
                            .group_hover(card_group.clone(), |s| {
                                s.opacity(1.0).bg(gpui::rgba(0x00000066))
                            })
                            .child(
                                div()
                                    .size(px(54.0))
                                    .rounded_full()
                                    .bg(theme.tokens.primary)
                                    .shadow_lg()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named("play".into()))
                                            .size(px(22.0))
                                            .color(theme.tokens.primary_foreground),
                                    ),
                            ),
                    ),
            )
            // ── Body: clear hierarchy — title primary, identity/meta secondary,
            // and the reactions/comments action row fenced off below a hairline
            // divider so it doesn't blur into the author line.
            .child(
                VStack::new()
                    .px_3()
                    .py_3()
                    .gap_2p5()
                    // Title (primary): a touch larger + bolder so it clearly
                    // leads the card over the secondary identity/meta line.
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.tokens.foreground)
                            .child(clip.title.clone()),
                    )
                    // Identity + meta (secondary), grouped: author name above a
                    // tertiary "game · when" line beside the author avatar.
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(avatar(author_seed, author_initial, author_tint, 24.0))
                            .child(
                                VStack::new()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.tokens.foreground.opacity(0.9))
                                            .child(author_name.to_string()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(11.0))
                                            .text_color(theme.tokens.muted_foreground)
                                            .child(if clip.game.is_empty() {
                                                clip.when.clone()
                                            } else {
                                                format!("{} · {}", clip.game, clip.when)
                                            }),
                                    ),
                            ),
                    )
                    // Divider fences identity from the action row below.
                    .child(div().h(px(1.0)).bg(theme.tokens.border.opacity(0.7)))
                    // Reactions (left) + comments (right).
                    .child(self.render_clip_actions_row(team, clip, cx)),
            )
    }

    /// The reactions + comments row beneath a clip card: emoji tally chips, an
    /// "add reaction" button, and the comment count.
    fn render_clip_actions_row(
        &self,
        team: &Team,
        clip: &TeamClip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();

        HStack::new()
            .gap_2()
            .items_center()
            .justify_between()
            // Left group: emoji tallies + the add-reaction button, wrapping as a
            // unit so reactions stay visually distinct from the comment action.
            .child(
                HStack::new()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .flex_wrap()
                    .items_center()
                    // Existing emoji tallies (each toggles the caller's reaction).
                    .children(clip.reactions.iter().map(|t| {
                        let team_id = team.id.clone();
                        let clip_id = clip.id.clone();
                        let emoji = t.emoji.clone();
                        let mine = t.mine;
                        div()
                            .id(SharedString::from(format!("react-{}-{}", clip.id, t.emoji)))
                            .flex()
                            .gap_1()
                            .items_center()
                            .px_1p5()
                            .h(px(24.0))
                            .rounded_full()
                            .cursor_pointer()
                            .bg(if mine { theme.tokens.primary.opacity(0.18) } else { theme.tokens.muted })
                            .border_1()
                            .border_color(if mine { theme.tokens.primary } else { gpui::transparent_black() })
                            .hover(|s| s.bg(theme.tokens.primary.opacity(0.12)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_clip_reaction(
                                        team_id.clone(),
                                        clip_id.clone(),
                                        emoji.clone(),
                                        cx,
                                    );
                                }),
                            )
                            .child(div().text_xs().child(t.emoji.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if mine {
                                        theme.tokens.primary
                                    } else {
                                        theme.tokens.muted_foreground
                                    })
                                    .child(format!("{}", t.count)),
                            )
                            .into_any_element()
                    }))
                    // Add-reaction button → opens the emoji picker. A smiley-plus
                    // glyph reads as "react with an emoji" rather than a generic
                    // "add"; the dashed-feel muted ring marks it as the affordance.
                    .child({
                        let id = clip.id.clone();
                        div()
                            .id((clip.el_id.clone(), 6usize))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.0))
                            .rounded_full()
                            .cursor_pointer()
                            .bg(theme.tokens.muted.opacity(0.5))
                            .border_1()
                            .border_color(theme.tokens.border)
                            .hover(|s| {
                                s.bg(theme.tokens.primary.opacity(0.12))
                                    .border_color(theme.tokens.primary)
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.teams.reaction_picker = Some(id.clone());
                                    cx.notify();
                                }),
                            )
                            .child(
                                Icon::new(IconSource::Named("smile-plus".into()))
                                    .size(px(14.0))
                                    .color(theme.tokens.muted_foreground),
                            )
                    }),
            )
            // Right: comment count → opens the thread.
            .child({
                let cid = clip.id.clone();
                div()
                    .id((clip.el_id.clone(), 5usize))
                    .flex_none()
                    .flex()
                    .gap_1()
                    .items_center()
                    .px_2()
                    .h(px(24.0))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.muted))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.open_comments(cid.clone(), cx);
                        }),
                    )
                    .child(
                        Icon::new(IconSource::Named("message-square".into()))
                            .size(px(14.0))
                            .color(theme.tokens.muted_foreground),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.muted_foreground)
                            .child(format!("{}", clip.comment_count)),
                    )
            })
    }

    /// Inline error banner shown above the signed-in Teams content so cloud
    /// failures (list/load/create/join/sign-out) aren't silent. Dismissible.
    fn render_teams_error_banner(
        &self,
        msg: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        const ERR: u32 = 0xef4444;

        HStack::new()
            .mx_8()
            .mt_4()
            .px_4()
            .py_3()
            .gap_3()
            .items_center()
            .rounded_lg()
            .bg(gpui::rgba(0xef444414))
            .border_1()
            .border_color(gpui::rgba(0xef444455))
            .child(
                Icon::new(IconSource::Named("info".into()))
                    .size(px(16.0))
                    .color(gpui::rgb(ERR).into()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .text_color(theme.tokens.foreground)
                    .child(msg),
            )
            .child(
                div()
                    .id("teams-error-dismiss")
                    .size(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.muted))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.teams.error = None;
                            cx.notify();
                        }),
                    )
                    .child(
                        Icon::new(IconSource::Named("x".into()))
                            .size(px(14.0))
                            .color(theme.tokens.muted_foreground),
                    ),
            )
    }

    // ── Empty state: create or join ─────────────────────────────────
    fn render_teams_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                VStack::new()
                    .w(px(660.0))
                    .gap_8()
                    .items_center()
                    .child(
                        VStack::new()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .size(px(72.0))
                                    .rounded_xl()
                                    .bg(theme.tokens.primary.opacity(0.12))
                                    .border_1()
                                    .border_color(theme.tokens.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconSource::Named("users".into()))
                                            .size(px(30.0))
                                            .color(theme.tokens.primary),
                                    ),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Play together, clip together"),
                            )
                            .child(
                                div()
                                    .max_w(px(440.0))
                                    .text_sm()
                                    .text_center()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(
                                        "Create a team for your squad, or join one with an invite \
                                         code. Everyone's best clips land in one shared feed.",
                                    ),
                            ),
                    )
                    .child(
                        HStack::new()
                            .gap_4()
                            .w_full()
                            .child(self.render_choice_card(
                                "teams-choice-create",
                                "user-plus",
                                "Create a team",
                                "Start a team and invite your friends with a code or link.",
                                TeamsPanel::Create,
                                cx,
                            ))
                            .child(self.render_choice_card(
                                "teams-choice-join",
                                "log-in",
                                "Join a team",
                                "Got an invite code? Drop it in to join an existing team.",
                                TeamsPanel::Join,
                                cx,
                            )),
                    ),
            )
    }

    fn render_choice_card(
        &self,
        id: &'static str,
        icon: &'static str,
        title: &'static str,
        body: &'static str,
        target: TeamsPanel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();

        div()
            .id(id)
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .rounded_xl()
            .bg(theme.tokens.card)
            .border_1()
            .border_color(theme.tokens.border)
            .cursor_pointer()
            .hover(|s| s.border_color(theme.tokens.primary).shadow_lg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.teams.panel = target;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .size(px(44.0))
                    .rounded_lg()
                    .bg(theme.tokens.primary.opacity(0.12))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconSource::Named(icon.into()))
                            .size(px(20.0))
                            .color(theme.tokens.primary),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.tokens.muted_foreground)
                    .child(body),
            )
    }

    // ── Modal overlay: create / join forms ──────────────────────────
    fn render_teams_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let is_create = self.teams.panel == TeamsPanel::Create;

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000B3))
            .on_mouse_down(MouseButton::Left, |_, _, _| {}) // block click-through
            .child(
                VStack::new()
                    .w(px(440.0))
                    .gap_5()
                    .p_6()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(if is_create { "Create a team" } else { "Join a team" }),
                            )
                            .child(
                                div()
                                    .id("teams-close-overlay")
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.muted))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.teams.panel = TeamsPanel::None;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconSource::Named("x".into()))
                                            .size(px(16.0))
                                            .color(theme.tokens.muted_foreground),
                                    ),
                            ),
                    )
                    .child(if is_create {
                        VStack::new()
                            .gap_2()
                            .child(field_label("Team name"))
                            .child(Input::new(&self.teams.name_input).placeholder("e.g. Night Squad"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("You'll get a shareable invite code once it's created."),
                            )
                            .into_any_element()
                    } else {
                        VStack::new()
                            .gap_2()
                            .child(field_label("Invite code"))
                            .child(Input::new(&self.teams.join_code_input).placeholder("XXXX-XXXX"))
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(IconSource::Named("info".into()))
                                            .size(px(13.0))
                                            .color(theme.tokens.muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.tokens.muted_foreground)
                                            .child("Ask a teammate for the code on rekaptr.dev."),
                                    ),
                            )
                            .into_any_element()
                    })
                    .child(
                        HStack::new()
                            .gap_2()
                            .justify_end()
                            .pt_2()
                            .child(
                                Button::new("teams-cancel", "Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Md)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.teams.panel = TeamsPanel::None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(
                                    "teams-confirm",
                                    if is_create { "Create team" } else { "Join team" },
                                )
                                .variant(ButtonVariant::Default)
                                .size(ButtonSize::Md)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_teams_panel(is_create, window, cx);
                                })),
                            ),
                    ),
            )
    }
    /// Action sheet for a single clip (the "···" menu), shown as a centered
    /// modal. Actions are gated on whether the clip is the caller's and on their
    /// team role.
    fn render_clip_actions_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let Some(clip_id) = self.teams.clip_menu.clone() else {
            return div().into_any_element();
        };
        let Some(team) = self.active_team() else {
            return div().into_any_element();
        };
        let Some(clip) = team.clips.iter().find(|c| c.id == clip_id) else {
            return div().into_any_element();
        };

        let team_id = team.id.clone();
        let title = clip.title.clone();
        let is_mine = team.clip_is_mine(clip);
        let can_unshare = is_mine || team.i_am_admin();
        let video_url = clip.video_url.clone();
        let download_name = sanitize_filename(&title);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000B3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.clip_menu = None;
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .w(px(320.0))
                    .gap_1()
                    .p_2()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .truncate()
                            .child(title.clone()),
                    )
                    .child(div().h(px(1.0)).bg(theme.tokens.border).mx_1())
                    // Download + copy link: only once the clip is READY (has URLs).
                    .when_some(video_url.clone(), |this, url| {
                        this.child(
                            menu_row("teams-clipmenu-download", "download", "Download", false)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.download_clip(url.clone(), download_name.clone(), cx);
                                    }),
                                ),
                        )
                    })
                    .when(video_url.is_some(), |this| {
                        let id = clip_id.clone();
                        this.child(
                            menu_row("teams-clipmenu-copy", "external-link", "Copy link", false)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.copy_clip_link(id.clone(), window, cx);
                                    }),
                                ),
                        )
                    })
                    .when(is_mine, |this| {
                        let id = clip_id.clone();
                        let t = title.clone();
                        this.child(
                            menu_row("teams-clipmenu-rename", "pencil", "Rename", false)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.begin_rename_clip(id.clone(), t.clone(), window, cx);
                                    }),
                                ),
                        )
                    })
                    .when(can_unshare, |this| {
                        let tid = team_id.clone();
                        let id = clip_id.clone();
                        this.child(
                            menu_row("teams-clipmenu-unshare", "scissors", "Remove from team", false)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.unshare_clip_here(tid.clone(), id.clone(), cx);
                                    }),
                                ),
                        )
                    })
                    .when(is_mine, |this| {
                        let id = clip_id.clone();
                        this.child(
                            menu_row("teams-clipmenu-delete", "trash", "Delete clip", true)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.delete_clip_everywhere(id.clone(), cx);
                                    }),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Rename modal: a single text field prefilled with the clip's current name.
    fn render_rename_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000B3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.rename_target = None;
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .w(px(440.0))
                    .gap_5()
                    .p_6()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(div().text_lg().font_weight(FontWeight::BOLD).child("Rename clip"))
                    .child(
                        VStack::new()
                            .gap_2()
                            .child(field_label("Clip name"))
                            .child(Input::new(&self.teams.rename_input).placeholder("e.g. Insane 1v3 clutch")),
                    )
                    .child(
                        HStack::new()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("teams-rename-cancel", "Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Md)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.teams.rename_target = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("teams-rename-save", "Save")
                                    .variant(ButtonVariant::Default)
                                    .size(ButtonSize::Md)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_rename_clip(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    /// Emoji reaction picker for a clip, shown as a centered palette.
    fn render_reaction_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let Some(clip_id) = self.teams.reaction_picker.clone() else {
            return div().into_any_element();
        };
        let Some(team) = self.active_team() else {
            return div().into_any_element();
        };
        let team_id = team.id.clone();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000080))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.reaction_picker = None;
                    cx.notify();
                }),
            )
            .child(
                HStack::new()
                    .gap_1()
                    .p_2()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(REACTION_EMOJI.iter().map(|emoji| {
                        let team_id = team_id.clone();
                        let clip_id = clip_id.clone();
                        let e = emoji.to_string();
                        div()
                            .id(SharedString::from(format!("pick-{emoji}")))
                            .size(px(40.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .cursor_pointer()
                            .text_size(px(22.0))
                            .hover(|s| s.bg(theme.tokens.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_clip_reaction(
                                        team_id.clone(),
                                        clip_id.clone(),
                                        e.clone(),
                                        cx,
                                    );
                                }),
                            )
                            .child(emoji.to_string())
                    })),
            )
            .into_any_element()
    }

    /// Comment thread for a clip, shown as a centered modal: header, scrollable
    /// thread, and a compose row.
    fn render_comments_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let loading = self.teams.comments_loading;
        let count = self.teams.comments.len();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000B3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.comments_open = None;
                    this.teams.comments = Vec::new();
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .w(px(460.0))
                    .h(px(560.0))
                    .gap_3()
                    .p_5()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(section_header("Comments", count))
                            .child(
                                div()
                                    .id("teams-comments-close")
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.muted))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.teams.comments_open = None;
                                            this.teams.comments = Vec::new();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconSource::Named("x".into()))
                                            .size(px(16.0))
                                            .color(theme.tokens.muted_foreground),
                                    ),
                            ),
                    )
                    // Thread
                    .child(
                        div()
                            .id("teams-comments-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(if loading {
                                div()
                                    .py_10()
                                    .flex()
                                    .justify_center()
                                    .text_sm()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("Loading…")
                                    .into_any_element()
                            } else if count == 0 {
                                div()
                                    .py_10()
                                    .flex()
                                    .justify_center()
                                    .text_sm()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("No comments yet. Say something!")
                                    .into_any_element()
                            } else {
                                VStack::new()
                                    .gap_3()
                                    .children(
                                        self.teams
                                            .comments
                                            .iter()
                                            .map(|c| self.render_comment_row(c, cx).into_any_element()),
                                    )
                                    .into_any_element()
                            }),
                    )
                    // Compose
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.teams.comment_input).placeholder("Add a comment…")),
                            )
                            .child(
                                Button::new("teams-comment-send", "Send")
                                    .variant(ButtonVariant::Default)
                                    .size(ButtonSize::Md)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_comment(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_comment_row(&self, c: &CommentItem, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        HStack::new()
            .gap_3()
            .items_start()
            .group(SharedString::from(format!("cmt-{}", c.id)))
            .child(avatar(&c.author_user_id, &c.author_initial, c.author_tint, 30.0))
            .child(
                VStack::new()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        HStack::new()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.tokens.foreground)
                                    .child(c.author_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground.opacity(0.7))
                                    .child(c.when.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.tokens.foreground.opacity(0.9))
                            .child(c.body.clone()),
                    ),
            )
            .when(c.can_delete, |this| {
                let id = c.id.clone();
                this.child(
                    div()
                        .id(SharedString::from(format!("cmt-del-{}", c.id)))
                        .size(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .opacity(0.0)
                        .group_hover(SharedString::from(format!("cmt-{}", c.id)), |s| s.opacity(1.0))
                        .hover(|s| s.bg(theme.tokens.muted))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.delete_comment(id.clone(), cx);
                            }),
                        )
                        .child(
                            Icon::new(IconSource::Named("trash".into()))
                                .size(px(14.0))
                                .color(theme.tokens.muted_foreground),
                        ),
                )
            })
    }

    /// Member-management panel: invite code (admins), roster with role controls,
    /// and leave/delete-team actions.
    fn render_members_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let Some(team) = self.active_team() else {
            return div().into_any_element();
        };
        let is_owner = team.my_role == "OWNER";
        let is_admin = team.i_am_admin();
        let invite_code = team.invite_code.clone();
        // Owner may leave only when sole member (that path deletes the team);
        // otherwise they must transfer first.
        let show_leave = !is_owner || team.members.len() <= 1;

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000B3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.teams.members_open = false;
                    cx.notify();
                }),
            )
            .child(
                VStack::new()
                    .w(px(520.0))
                    .max_h(px(620.0))
                    .gap_4()
                    .p_6()
                    .rounded_xl()
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .shadow_xl()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{} · members", team.name)),
                            )
                            .child(
                                div()
                                    .id("teams-members-close")
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.muted))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.teams.members_open = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconSource::Named("x".into()))
                                            .size(px(16.0))
                                            .color(theme.tokens.muted_foreground),
                                    ),
                            ),
                    )
                    // Invite code (admins+)
                    .when(is_admin, |this| {
                        this.child(
                            HStack::new()
                                .gap_3()
                                .items_center()
                                .p_3()
                                .rounded_lg()
                                .bg(theme.tokens.muted.opacity(0.5))
                                .border_1()
                                .border_color(theme.tokens.border)
                                .child(
                                    VStack::new()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(field_label("Invite code"))
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.tokens.foreground)
                                                .child(
                                                    invite_code.clone().unwrap_or_else(|| "…".into()),
                                                ),
                                        ),
                                )
                                .child(
                                    Button::new("teams-invite-copy", "Copy")
                                        .icon(IconSource::Named("external-link".into()))
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.copy_invite_code(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("teams-invite-regen", "Regenerate")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.regenerate_invite(cx);
                                        })),
                                ),
                        )
                    })
                    // Roster
                    .child(
                        div()
                            .id("teams-members-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                VStack::new().gap_1().children(
                                    team.members
                                        .iter()
                                        .map(|m| self.render_member_row(team, m, cx).into_any_element()),
                                ),
                            ),
                    )
                    // Footer: leave / delete
                    .child(
                        HStack::new()
                            .justify_between()
                            .items_center()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme.tokens.border)
                            .child(
                                div().when(show_leave, |this| {
                                    this.child(
                                        Button::new("teams-leave", "Leave team")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.leave_active_team(cx);
                                            })),
                                    )
                                }),
                            )
                            .child(
                                div().when(is_owner, |this| {
                                    this.child(
                                        Button::new("teams-delete", "Delete team")
                                            .icon(IconSource::Named("trash".into()))
                                            .variant(ButtonVariant::Destructive)
                                            .size(ButtonSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.delete_active_team(cx);
                                            })),
                                    )
                                }),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_member_row(
        &self,
        team: &Team,
        m: &Member,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let i_am_owner = team.my_role == "OWNER";
        let i_am_admin = team.i_am_admin();
        let is_me = m.user_id == team.me_user_id;
        let target_is_owner = m.role == "OWNER";
        let target_is_admin = m.role == "ADMIN";

        // Role chip color: owner gold, admin primary, member muted.
        let (role_label, role_color) = match m.role.as_str() {
            "OWNER" => ("Owner", gpui::rgb(0xf0b429).into()),
            "ADMIN" => ("Admin", theme.tokens.primary),
            _ => ("Member", theme.tokens.muted_foreground),
        };

        HStack::new()
            .items_center()
            .gap_3()
            .h(px(48.0))
            .px_2()
            .rounded_lg()
            .hover(|s| s.bg(theme.tokens.muted.opacity(0.4)))
            .child(avatar(&m.user_id, &m.initial, m.tint, 32.0))
            .child(
                HStack::new()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.tokens.foreground)
                            .child(m.name.clone()),
                    )
                    .when(is_me, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .child("(you)"),
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(role_color)
                    .child(role_label),
            )
            // Owner controls: promote/demote, transfer to an admin, remove.
            .when(i_am_owner && !is_me && !target_is_owner, |this| {
                let uid_role = m.user_id.clone();
                let uid_remove = m.user_id.clone();
                let to_role = if target_is_admin { "MEMBER" } else { "ADMIN" };
                let role_btn_label = if target_is_admin { "Make member" } else { "Make admin" };
                this.child(
                    Button::new(
                        SharedString::from(format!("teams-role-{}", m.user_id)),
                        role_btn_label,
                    )
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_member_role(uid_role.clone(), to_role, cx);
                    })),
                )
                .when(target_is_admin, |this| {
                    let uid_owner = m.user_id.clone();
                    this.child(
                        Button::new(
                            SharedString::from(format!("teams-owner-{}", m.user_id)),
                            "Make owner",
                        )
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_member_role(uid_owner.clone(), "OWNER", cx);
                        })),
                    )
                })
                .child(
                    Button::new(
                        SharedString::from(format!("teams-remove-{}", m.user_id)),
                        "Remove",
                    )
                    .variant(ButtonVariant::Destructive)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_team_member(uid_remove.clone(), cx);
                    })),
                )
            })
            // Admin (non-owner) controls: remove regular members only.
            .when(i_am_admin && !i_am_owner && !is_me && !target_is_owner && !target_is_admin, |this| {
                let uid = m.user_id.clone();
                this.child(
                    Button::new(
                        SharedString::from(format!("teams-remove-{}", m.user_id)),
                        "Remove",
                    )
                    .variant(ButtonVariant::Destructive)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_team_member(uid.clone(), cx);
                    })),
                )
            })
    }

    /// Signed-out gate for the Teams tab: a single "Sign in" call to action.
    fn render_teams_signin(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let busy = self.teams.busy;

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                VStack::new()
                    .w(px(420.0))
                    .gap_5()
                    .items_center()
                    .child(
                        div()
                            .size(px(72.0))
                            .rounded_xl()
                            .bg(theme.tokens.primary.opacity(0.12))
                            .border_1()
                            .border_color(theme.tokens.border)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconSource::Named("users".into()))
                                    .size(px(30.0))
                                    .color(theme.tokens.primary),
                            ),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Play together, clip together"),
                    )
                    .child(
                        div()
                            .max_w(px(360.0))
                            .text_sm()
                            .text_center()
                            .text_color(theme.tokens.muted_foreground)
                            .child(
                                "Sign in to your Rekaptr account to create a team, share clips, \
                                 and see what your squad is hitting.",
                            ),
                    )
                    .child(
                        div()
                            .id("teams-signin-btn")
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .h(px(44.0))
                            .px_6()
                            .rounded_lg()
                            .cursor_pointer()
                            .bg(theme.tokens.primary)
                            .text_color(theme.tokens.primary_foreground)
                            .font_weight(FontWeight::SEMIBOLD)
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.start_cloud_sign_in(cx)),
                            )
                            .child(
                                Icon::new(IconSource::Named("log-in".into()))
                                    .size(px(18.0))
                                    .color(theme.tokens.primary_foreground),
                            )
                            .child(if busy { "Signing in…" } else { "Sign in" }),
                    )
                    .when(self.teams.error.is_some(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0xef4444))
                                .child(self.teams.error.clone().unwrap_or_default()),
                        )
                    }),
            )
    }
    fn render_team_player_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let Some(video) = self.teams.player.as_ref() else {
            return div().into_any_element();
        };

        let pos = video.position().as_secs_f64();
        let dur = video.duration().as_secs_f64().max(1.0);
        let progress = (pos / dur).clamp(0.0, 1.0) as f32;
        let paused = video.paused();

        let fmt = |s: f64| {
            let t = s.max(0.0) as u64;
            format!("{:01}:{:02}", t / 60, t % 60)
        };
        let time_label = format!("{} / {}", fmt(pos), fmt(dur));

        let window_width = window.viewport_size().width.0;
        let window_height = window.viewport_size().height.0;
        let player_width = (window_width - 80.0).clamp(480.0, 1120.0);
        let player_height = (player_width * 630.0 / 1120.0).min(window_height - 80.0);
        let left_offset = (window_width - player_width) / 2.0;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000_cc))
            .flex()
            .items_center()
            .justify_center()
            // Click the dimmed backdrop to close.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_team_player(window, cx)),
            )
            .child(
                div()
                    .id("teams-player-box")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        if this.teams.player_scrubbing {
                            if let Some(v) = &this.teams.player {
                                let p = ((event.position.x.0 - left_offset) / player_width)
                                    .clamp(0.0, 1.0);
                                let _ = v.seek(
                                    std::time::Duration::from_secs_f64(p as f64 * dur),
                                    false,
                                );
                                cx.notify();
                            }
                        }
                    }))
                    .w(px(player_width))
                    .h(px(player_height))
                    .bg(gpui::black())
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.tokens.border)
                    .overflow_hidden()
                    .relative()
                    .child(div().size_full().bg(gpui::black()).child(crate::video_player::video(video.clone())))
                    // Top HUD: title + close.
                    .child(
                        HStack::new()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .px_5()
                            .py_4()
                            .bg(gpui::rgba(0x000000_88))
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(gpui::white())
                                    .truncate()
                                    .child(self.teams.player_title.clone().unwrap_or_default()),
                            )
                            .child(
                                div()
                                    .id("teams-player-close")
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.close_team_player(window, cx)
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconSource::Named("x".into()))
                                            .size(px(24.0))
                                            .color(gpui::white()),
                                    ),
                            ),
                    )
                    // Bottom HUD: play/pause + scrub bar + time.
                    .child(
                        VStack::new()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .bg(gpui::rgba(0x000000_88))
                            .child(
                                div()
                                    .id("teams-player-seek")
                                    .h(px(12.0))
                                    .w_full()
                                    .relative()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(
                                        move |this, event: &MouseDownEvent, _, cx| {
                                            this.teams.player_scrubbing = true;
                                            if let Some(v) = &this.teams.player {
                                                let p = ((event.position.x.0 - left_offset)
                                                    / player_width)
                                                    .clamp(0.0, 1.0);
                                                let _ = v.seek(
                                                    std::time::Duration::from_secs_f64(
                                                        p as f64 * dur,
                                                    ),
                                                    false,
                                                );
                                            }
                                            cx.notify();
                                        },
                                    ))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.teams.player_scrubbing = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        adabraka_ui::components::progress::ProgressBar::new(progress)
                                            .h(px(6.0))
                                            .absolute()
                                            .bottom_0(),
                                    ),
                            )
                            .child(
                                HStack::new()
                                    .px_5()
                                    .py_3()
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        div()
                                            .id("teams-player-playpause")
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    if let Some(v) = &this.teams.player {
                                                        v.set_paused(!v.paused());
                                                    }
                                                    cx.notify();
                                                }),
                                            )
                                            .child(
                                                Icon::new(IconSource::Named(
                                                    if paused { "play".into() } else { "pause".into() },
                                                ))
                                                .size(px(20.0))
                                                .color(gpui::white()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(gpui::white())
                                            .child(time_label),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

// ── Small shared pieces ─────────────────────────────────────────────

/// Lighten (`amt > 0`) or darken (`amt < 0`) an `0xRRGGBB` color by a fraction
/// of the way to white/black. Used to derive the top→bottom gradient that gives
/// flat tint avatars/badges a bit of depth.
fn shade(rgb: u32, amt: f32) -> u32 {
    let chan = |shift: u32| {
        let c = ((rgb >> shift) & 0xff) as f32;
        let v = if amt >= 0.0 { c + (255.0 - c) * amt } else { c * (1.0 + amt) };
        v.clamp(0.0, 255.0) as u32
    };
    (chan(16) << 16) | (chan(8) << 8) | chan(0)
}

/// Diagonal top-left→bottom-right gradient from a lightened to a darkened tint,
/// shared by avatars and team badges so a flat per-user/team color reads with
/// depth instead of as a solid blob.
fn tint_gradient(tint: u32) -> gpui::Background {
    gpui::linear_gradient(
        145.0,
        gpui::linear_color_stop(gpui::rgb(shade(tint, 0.26)), 0.0),
        gpui::linear_color_stop(gpui::rgb(shade(tint, -0.20)), 1.0),
    )
}

fn team_badge(initials: &str, tint: u32, size: f32) -> impl IntoElement {
    let theme = use_theme();
    div()
        .size(px(size))
        .flex_none()
        .rounded_lg()
        .bg(tint_gradient(tint))
        // Soft shadow (no white ring) gives separation without a chrome edge.
        .shadow_sm()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.tokens.foreground)
        .font_weight(FontWeight::BOLD)
        .text_size(px((size * 0.38).round()))
        .child(initials.to_string())
}

/// The parsed DiceBear `identicon` style definition, built once.
fn identicon_style() -> Option<&'static dicebear_core::Style> {
    static STYLE: std::sync::OnceLock<Option<dicebear_core::Style>> = std::sync::OnceLock::new();
    STYLE
        .get_or_init(|| match dicebear_core::Style::from_str(dicebear_styles::IDENTICON) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("[Teams] dicebear identicon style failed to parse: {e}");
                None
            }
        })
        .as_ref()
}

/// Generate (and memoize) a DiceBear identicon for `seed` as a renderable GPUI
/// SVG image. The seed is the user's stable id, so each user gets a distinct
/// but reproducible avatar everywhere they appear. Returns `None` (→ initial
/// fallback) if the styles crate or generation ever fails. Memoized per seed so
/// the SVG is produced once, not per frame.
fn dicebear_avatar(seed: &str) -> Option<std::sync::Arc<gpui::Image>> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<dashmap::DashMap<String, Option<std::sync::Arc<gpui::Image>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(dashmap::DashMap::new);
    if let Some(hit) = cache.get(seed) {
        return hit.clone();
    }

    let image = identicon_style().and_then(|style| {
        // Options mirror the requested identicon look (violet→near-black linear
        // fill); only the seed varies per user.
        let opts = serde_json::json!({
            "rowColorFill": ["linear"],
            "rowColorAngle": 170,
            "rowColorFillStops": 3,
            "rowColor": ["8b5cf6", "09090b"],
            "seed": seed,
            // Without an explicit size the SVG only carries a `0 0 5 5` viewBox,
            // so GPUI rasterizes it ~5px wide and it renders badly blurred.
            // Emit width/height = 256 so it rasterizes crisply for any avatar
            // size we display.
            "size": 256,
        });
        match dicebear_core::Avatar::new(style, opts) {
            Ok(a) => Some(std::sync::Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Svg,
                a.to_svg().as_bytes().to_vec(),
            ))),
            Err(e) => {
                log::warn!("[Teams] dicebear avatar generation failed for {seed:?}: {e}");
                None
            }
        }
    });
    cache.insert(seed.to_string(), image.clone());
    image
}

/// A user profile picture: a DiceBear identicon seeded by `seed` (the user's
/// id), clipped to a circle with a ring + shadow. Falls back to the tinted
/// initial monogram if avatar generation is unavailable.
fn avatar(seed: &str, initial: &str, tint: u32, size: f32) -> impl IntoElement {
    let theme = use_theme();
    let frame = div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .overflow_hidden()
        // Soft shadow lifts the avatar off the surface; no white ring (it read as
        // a chrome edge).
        .shadow_sm();

    match dicebear_avatar(seed) {
        Some(image) => frame
            // Round the image itself too: GPUI's content mask is a rectangle and
            // won't round-clip a child to the frame's circle, so a square image
            // would spill past the round border. Matching radius keeps it inside.
            .child(
                img(image)
                    .size_full()
                    .rounded_full()
                    .object_fit(ObjectFit::Fill),
            )
            .into_any_element(),
        None => frame
            .bg(tint_gradient(tint))
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.tokens.foreground)
            .font_weight(FontWeight::EXTRA_BOLD)
            .text_size(px((size * 0.44).round()))
            .child(initial.to_string())
            .into_any_element(),
    }
}

// Overlapping avatar stack for the team header.
fn render_member_stack(members: &[Member]) -> impl IntoElement {
    let theme = use_theme();
    let shown: Vec<Member> = members.iter().take(4).cloned().collect();
    let extra = members.len().saturating_sub(shown.len());

    HStack::new()
        .items_center()
        .children(shown.into_iter().enumerate().map(|(i, m)| {
            div()
                .when(i > 0, |s| s.ml(px(-8.0)))
                .rounded_full()
                .border_2()
                .border_color(theme.tokens.background)
                .child(avatar(&m.user_id, &m.initial, m.tint, 30.0))
        }))
        .when(extra > 0, |s| {
            s.child(
                div()
                    .ml(px(-8.0))
                    .size(px(30.0))
                    .rounded_full()
                    .bg(theme.tokens.muted)
                    .border_2()
                    .border_color(theme.tokens.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.muted_foreground)
                    .child(format!("+{}", extra)),
            )
        })
}

/// A single interactive row in the clip "···" action sheet. Returns the styled
/// row; the caller attaches `.on_mouse_down(..)`. `danger` tints it red.
fn menu_row(
    id: impl Into<ElementId>,
    icon: &str,
    label: impl Into<String>,
    danger: bool,
) -> gpui::Stateful<Div> {
    let theme = use_theme();
    const DANGER: u32 = 0xef4444;
    let text_color = if danger { gpui::rgb(DANGER).into() } else { theme.tokens.foreground };
    let icon_color = if danger {
        gpui::rgb(DANGER).into()
    } else {
        theme.tokens.muted_foreground
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_3()
        .h(px(40.0))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme.tokens.muted))
        .child(
            Icon::new(IconSource::Named(icon.to_string().into()))
                .size(px(16.0))
                .color(icon_color),
        )
        .child(div().text_sm().text_color(text_color).child(label.into()))
}

/// Make a clip title safe to use as a download filename (no path separators or
/// reserved characters), appending `.mp4`.
fn sanitize_filename(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_().".contains(c) { c } else { '_' })
        .collect();
    let stem = stem.trim();
    format!("{}.mp4", if stem.is_empty() { "clip" } else { stem })
}

fn field_label(text: &'static str) -> impl IntoElement {
    let theme = use_theme();
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.tokens.muted_foreground)
        .child(text)
}

// Section heading with a trailing count, matching the app's Clips sections.
fn section_header(text: &str, count: usize) -> impl IntoElement {
    let theme = use_theme();
    HStack::new()
        .gap_2()
        .items_baseline()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.tokens.foreground)
                .child(text.to_string()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.tokens.muted_foreground)
                .child(format!("{}", count)),
        )
}
