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
                    .pb_3()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.muted_foreground)
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

    /// Sign-out control: revokes tokens and returns to the sign-in gate.
    fn render_sign_out_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let busy = self.teams.busy;

        div()
            .id("teams-sign-out")
            .flex()
            .items_center()
            .gap_3()
            .h(px(40.0))
            .px_3()
            .rounded_md()
            .cursor_pointer()
            .text_color(theme.tokens.muted_foreground)
            .hover(|s| s.bg(theme.tokens.muted))
            .when(!busy, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.sign_out_cloud(cx)),
                )
            })
            .child(
                Icon::new(IconSource::Named("log-in".into()))
                    .size(px(15.0))
                    .color(theme.tokens.muted_foreground),
            )
            .child(
                div()
                    .text_sm()
                    .child(if busy { "Signing out…" } else { "Sign out" }),
            )
    }

    fn team_rail_item(&self, i: usize, team: &Team, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();
        let active = self.teams.active.unwrap_or(0) == i;
        let online = team.members.iter().filter(|m| m.online).count();

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
                            .child(section_header(&heading, count))
                            .child(self.render_clip_grid(team, clips, cx)),
                    ),
            )
            .into_any_element()
    }

    fn feed_clips<'a>(&self, team: &'a Team) -> Vec<&'a TeamClip> {
        team.clips
            .iter()
            .filter(|c| self.teams.member_filter.map_or(true, |m| c.author == m))
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
            .gap_4()
            .children(
                clips
                    .into_iter()
                    .map(|c| self.render_team_clip_card(team, c, cx).into_any_element())
                    .collect::<Vec<_>>(),
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
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!("{} clips shared", team.clips.len())),
                            ),
                    ),
            )
            .child(
                HStack::new()
                    .gap_3()
                    .items_center()
                    .child(render_member_stack(&team.members))
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
                        Button::new("teams-invite", "Invite")
                            .icon(IconSource::Named("user-plus".into()))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.teams.panel = TeamsPanel::Create;
                                cx.notify();
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

    fn render_team_clip_card(
        &self,
        team: &Team,
        clip: &TeamClip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = use_theme();
        let fallback = Member {
            user_id: String::new(),
            name: "Unknown".to_string(),
            initial: "?".to_string(),
            tint: 0x6b7280,
            online: false,
        };
        let author = team.members.get(clip.author).unwrap_or(&fallback);
        let card_group = SharedString::from(format!("teams-tcard-{}", clip.id));

        div()
            .group(card_group.clone())
            .id(SharedString::from(format!("teams-tclip-{}", clip.id)))
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
                div()
                    .id(SharedString::from(format!("teams-tthumb-{}", clip.id)))
                    .relative()
                    .w_full()
                    .h(px(160.0))
                    .bg(gpui::rgb(clip.thumb_tint))
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
                    .child(
                        div().absolute().inset_0().bg(gpui::linear_gradient(
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
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.0)
                            .group_hover(card_group.clone(), |s| {
                                s.opacity(1.0).bg(gpui::rgba(0x00000066))
                            })
                            .child(
                                div()
                                    .size(px(52.0))
                                    .rounded_full()
                                    .bg(theme.tokens.primary)
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
            // ── Body
            .child(
                VStack::new()
                    .px_3()
                    .py_3()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(clip.title.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .child(clip.game.clone()),
                    )
                    .child(
                        HStack::new()
                            .pt_1()
                            .justify_between()
                            .items_center()
                            .child(
                                HStack::new()
                                    .gap_2()
                                    .items_center()
                                    .child(avatar(&author.initial, author.tint, 22.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.tokens.muted_foreground)
                                            .child(author.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.tokens.muted_foreground.opacity(0.7))
                                            .child(format!("· {}", clip.when)),
                                    ),
                            )
                            .child({
                                let reacted = clip.reacted_by_me;
                                let team_id = team.id.clone();
                                let clip_id = clip.id.clone();
                                let heart_color = if reacted {
                                    gpui::rgb(REACT).into()
                                } else {
                                    theme.tokens.muted_foreground
                                };
                                let count_color = if reacted {
                                    gpui::rgb(REACT).into()
                                } else {
                                    theme.tokens.muted_foreground
                                };
                                div()
                                    .id(SharedString::from(format!("teams-react-{}", clip.id)))
                                    .flex()
                                    .gap_1()
                                    .items_center()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.muted))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.toggle_clip_reaction(
                                                team_id.clone(),
                                                clip_id.clone(),
                                                cx,
                                            );
                                        }),
                                    )
                                    .child(
                                        Icon::new(IconSource::Named("heart".into()))
                                            .size(px(13.0))
                                            .color(heart_color),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(count_color)
                                            .child(format!("{}", clip.reactions)),
                                    )
                            }),
                    ),
            )
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
fn team_initials(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if initials.is_empty() { "T".to_string() } else { initials }
}

fn team_badge(initials: &str, tint: u32, size: f32) -> impl IntoElement {
    let theme = use_theme();
    div()
        .size(px(size))
        .flex_none()
        .rounded_lg()
        .bg(gpui::rgb(tint))
        .border_1()
        .border_color(gpui::rgba(0xFFFFFF14))
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.tokens.foreground)
        .font_weight(FontWeight::BOLD)
        .text_size(px((size * 0.36).round()))
        .child(initials.to_string())
}

fn avatar(initial: &str, tint: u32, size: f32) -> impl IntoElement {
    let theme = use_theme();
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .bg(gpui::rgb(tint))
        .border_1()
        .border_color(gpui::rgba(0xFFFFFF14))
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.tokens.foreground)
        .font_weight(FontWeight::BOLD)
        .text_size(px((size * 0.46).round()))
        .child(initial.to_string())
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
                .child(avatar(&m.initial, m.tint, 30.0))
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
