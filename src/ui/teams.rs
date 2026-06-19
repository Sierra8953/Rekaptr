// Teams view — the app's one networked surface: create or join a team and
// browse clips shared by teammates. Local-first everywhere else, this tab is
// intended to be backed by the rekaptr.dev backend (accounts, membership, clip
// hosting). That backend is not wired yet, so the data here is local/mock:
// "Join a team" pulls in a populated demo team, "Create a team" makes a fresh
// empty one. The rendering uses the app's theme tokens and component patterns,
// mirroring the Clips view.

use crate::cloud::api;
use crate::ui::RekaptrWorkspace;
use adabraka_ui::components::input::Input;
use adabraka_ui::prelude::*;
use gpui::*;

// Reaction rose and online-presence green are semantic accents, not brand
// colors, so they stay literal rather than tracking `theme.tokens.primary`.
const REACT: u32 = 0xf2647e;
const ONLINE: u32 = 0x34d399;

// Distinct, vivid avatar colors so members read as individual profiles.
const AVATAR_TINTS: [u32; 6] = [0x8b5cf6, 0x3b82f6, 0xec4899, 0xf59e0b, 0x14b8a6, 0x22c55e];

#[derive(Clone)]
pub struct Member {
    pub user_id: String,
    pub name: String,
    pub initial: String,
    pub tint: u32,
    pub online: bool,
}

#[derive(Clone)]
pub struct TeamClip {
    pub id: String,
    pub title: String,
    pub game: String,
    pub author: usize, // index into the team's members
    pub when: String,
    pub duration: String,
    pub thumb_tint: u32,
    pub reactions: u32,
    pub reacted_by_me: bool,
    pub new: bool,
    /// Direct Bunny MP4 URL, present once the clip is READY (else still
    /// transcoding). Streamed by the mini player.
    pub video_url: Option<String>,
}

#[derive(Clone)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub tint: u32,
    pub initials: String,
    pub invite_code: Option<String>,
    pub members: Vec<Member>,
    pub clips: Vec<TeamClip>,
    /// Whether members/clips have been fetched for this team yet.
    pub loaded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TeamsPanel {
    None,
    Create,
    Join,
}

/// All Teams-tab view state, grouped out of the `RekaptrWorkspace` god-object.
/// Local/mock until the rekaptr.dev backend is fully wired.
pub struct TeamsState {
    /// The user's teams (left rail).
    pub list: Vec<Team>,
    pub active: Option<usize>,
    pub member_filter: Option<usize>,
    pub panel: TeamsPanel,
    pub name_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub join_code_input: Entity<adabraka_ui::components::input_state::InputState>,
    /// Cloud (Clerk) sign-in state for the Teams tab.
    pub signed_in: bool,
    /// A cloud request (sign-in / list / create / join / load) is in flight.
    pub busy: bool,
    /// Whether the team list has been fetched at least once this session.
    pub listed: bool,
    /// Last cloud error, surfaced inline in the Teams tab.
    pub error: Option<String>,
    /// Whether the presence-heartbeat loop is already running (prevents dupes).
    pub presence_running: bool,
    /// A "Share a clip" upload (create → TUS → complete) is in flight.
    pub sharing: bool,
    /// Upload progress (0.0–1.0) for the in-flight share, for the progress UI.
    pub share_progress: f32,
    /// Mini-player for a team clip: the libmpv Video streaming a Bunny MP4 URL,
    /// plus the clip's title for the player HUD. `None` = no player open.
    pub player: Option<crate::video_player::Video>,
    pub player_title: Option<String>,
    /// Whether the user is dragging the team player's scrub bar.
    pub player_scrubbing: bool,
}

impl TeamsState {
    pub fn new(signed_in: bool, cx: &mut Context<RekaptrWorkspace>) -> Self {
        Self {
            list: Vec::new(),
            active: None,
            member_filter: None,
            panel: TeamsPanel::None,
            name_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            join_code_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            signed_in,
            busy: false,
            listed: false,
            error: None,
            presence_running: false,
            sharing: false,
            share_progress: 0.0,
            player: None,
            player_title: None,
            player_scrubbing: false,
        }
    }
}

// ── API → UI mapping ────────────────────────────────────────────────
// Convert the cloud DTOs (`crate::cloud::api`) into the local view structs the
// render code already uses.

fn team_from_summary(s: api::TeamSummary) -> Team {
    Team {
        id: s.id,
        name: s.name,
        tint: s.badge_tint,
        initials: s.initials,
        invite_code: None,
        members: Vec::new(),
        clips: Vec::new(),
        loaded: false,
    }
}

fn members_from_detail(d: &api::TeamDetail) -> Vec<Member> {
    d.members
        .iter()
        .map(|m| Member {
            user_id: m.user_id.clone(),
            name: m.display_name.clone(),
            initial: m.initial.clone(),
            tint: m.avatar_tint,
            online: m.online,
        })
        .collect()
}

fn clips_from_feed(items: &[api::ClipDto], members: &[Member]) -> Vec<TeamClip> {
    items
        .iter()
        .map(|c| {
            // Map the author's user_id back to a member index for attribution.
            let author = c
                .author
                .as_ref()
                .and_then(|a| members.iter().position(|m| m.user_id == a.user_id))
                .unwrap_or(0);
            TeamClip {
                id: c.id.clone(),
                title: c.title.clone(),
                game: c.game.clone(),
                author,
                when: relative_time(&c.created_at),
                duration: c.duration_ms.map(fmt_duration).unwrap_or_default(),
                thumb_tint: thumb_tint_for(&c.id),
                reactions: c.reaction_count,
                reacted_by_me: c.reacted_by_me,
                new: c.is_new,
                video_url: c.video_url.clone(),
            }
        })
        .collect()
}

/// Render an RFC 3339 timestamp as the client-side relative label the feed uses.
fn relative_time(rfc3339: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rfc3339) else {
        return String::new();
    };
    let secs = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0);
    match secs {
        s if s < 60 => "just now".to_string(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s if s < 172_800 => "Yesterday".to_string(),
        s => format!("{}d ago", s / 86_400),
    }
}

fn fmt_duration(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Stable placeholder thumbnail tint, derived from the clip id (used until the
/// real Bunny thumbnail is rendered).
fn thumb_tint_for(id: &str) -> u32 {
    const TINTS: [u32; 6] = [0x2F4858, 0x3A2E4F, 0x4A3340, 0x4A3D2A, 0x274050, 0x4A3550];
    let h = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    TINTS[(h as usize) % TINTS.len()]
}

impl RekaptrWorkspace {
    fn active_team(&self) -> Option<&Team> {
        self.teams.active
            .and_then(|i| self.teams.list.get(i))
            .or_else(|| self.teams.list.first())
    }

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

    /// Resolve the Create / Join modal. With no backend yet, this builds local
    /// state: Create makes a fresh team with just "You"; Join pulls in the
    /// populated demo team. Selects the resulting team and closes the modal.
    /// Resolve the Create / Join modal against the cloud API. Both paths run on
    /// a background thread (the calls are blocking) and then refresh the team
    /// list and select the resulting team.
    fn confirm_teams_panel(&mut self, is_create: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.teams.busy {
            return;
        }
        let auth = self.app_state.cloud_auth.clone();

        if is_create {
            let name = self.teams.name_input.read(cx).content.trim().to_string();
            if name.is_empty() {
                self.teams.error = Some("Enter a team name.".to_string());
                cx.notify();
                return;
            }
            self.teams.name_input.update(cx, |s, cx| s.set_value("", window, cx));
            self.teams.panel = TeamsPanel::None;
            self.teams.busy = true;
            self.teams.error = None;
            cx.notify();

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let created =
                                api::create_team(&auth, &name).map_err(|e| e.to_string())?;
                            let teams = api::list_teams(&auth).map_err(|e| e.to_string())?;
                            Ok::<_, String>((created.team.id, created.invite.code, teams))
                        })
                        .await;
                    let _ = this.update(&mut cx, |this, cx| {
                        this.teams.busy = false;
                        match result {
                            Ok((new_id, code, teams)) => {
                                this.teams.list =
                                    teams.into_iter().map(team_from_summary).collect();
                                this.teams.listed = true;
                                this.teams.member_filter = None;
                                if let Some(pos) = this.teams.list.iter().position(|t| t.id == new_id) {
                                    this.teams.active = Some(pos);
                                    if let Some(t) = this.teams.list.get_mut(pos) {
                                        t.invite_code = Some(code);
                                    }
                                    this.load_active_team(cx);
                                }
                            }
                            Err(e) => this.note_cloud_error(e),
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        } else {
            let code = self.teams.join_code_input.read(cx).content.trim().to_string();
            if code.is_empty() {
                self.teams.error = Some("Enter an invite code.".to_string());
                cx.notify();
                return;
            }
            self.teams.join_code_input.update(cx, |s, cx| s.set_value("", window, cx));
            self.teams.panel = TeamsPanel::None;
            self.teams.busy = true;
            self.teams.error = None;
            cx.notify();

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let accepted =
                                api::accept_invite(&auth, &code).map_err(|e| e.to_string())?;
                            let teams = api::list_teams(&auth).map_err(|e| e.to_string())?;
                            Ok::<_, String>((accepted.team_id, teams))
                        })
                        .await;
                    let _ = this.update(&mut cx, |this, cx| {
                        this.teams.busy = false;
                        match result {
                            Ok((team_id, teams)) => {
                                this.teams.list =
                                    teams.into_iter().map(team_from_summary).collect();
                                this.teams.listed = true;
                                this.teams.member_filter = None;
                                this.teams.active =
                                    this.teams.list.iter().position(|t| t.id == team_id);
                                if this.teams.active.is_some() {
                                    this.load_active_team(cx);
                                }
                            }
                            Err(e) => this.note_cloud_error(e),
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }
}

// ── Cloud (Teams) sign-in + loading ─────────────────────────────────
impl RekaptrWorkspace {
    /// Start the browser OAuth sign-in on a background thread, then load the
    /// user's teams. Blocking cloud work runs on the background executor (never
    /// the GPUI main thread / tokio context).
    /// Record a failed cloud call. If the failure means the session is gone
    /// (the auth layer clears its cache when a token can't be refreshed), drop
    /// back to the signed-out state so the user sees the "Sign in" gate again
    /// instead of a signed-in tab where every action silently errors.
    fn note_cloud_error(&mut self, e: String) {
        self.teams.error = Some(e);
        if !self.app_state.cloud_auth.is_signed_in() {
            self.teams.signed_in = false;
            self.teams.listed = false;
            self.teams.list = Vec::new();
            self.teams.active = None;
            self.teams.member_filter = None;
            self.teams.panel = TeamsPanel::None;
        }
    }

    pub fn start_cloud_sign_in(&mut self, cx: &mut Context<Self>) {
        if self.teams.busy {
            return;
        }
        self.teams.busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        auth.sign_in().map_err(|e| e.to_string())?;
                        api::list_teams(&auth).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.busy = false;
                    match result {
                        Ok(summaries) => {
                            this.teams.list = summaries.into_iter().map(team_from_summary).collect();
                            this.teams.signed_in = true;
                            this.teams.listed = true;
                            this.teams.error = None;
                            this.teams.active = (!this.teams.list.is_empty()).then_some(0);
                            if this.teams.active.is_some() {
                                this.load_active_team(cx);
                            }
                            this.start_presence_heartbeat(cx);
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Refresh the team list from the cloud (used on first opening the tab).
    pub fn reload_teams(&mut self, cx: &mut Context<Self>) {
        if self.teams.busy {
            return;
        }
        self.teams.busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move { api::list_teams(&auth).map_err(|e| e.to_string()) })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.busy = false;
                    match result {
                        Ok(summaries) => {
                            this.teams.list = summaries.into_iter().map(team_from_summary).collect();
                            this.teams.listed = true;
                            this.teams.error = None;
                            if this.teams.active.map_or(true, |i| i >= this.teams.list.len()) {
                                this.teams.active = (!this.teams.list.is_empty()).then_some(0);
                            }
                            if this.teams.active.is_some() {
                                this.load_active_team(cx);
                            }
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Sign out of the cloud account: revoke + clear tokens on a background
    /// thread, then drop all team state so the sign-in gate reappears.
    pub fn sign_out_cloud(&mut self, cx: &mut Context<Self>) {
        if self.teams.busy {
            return;
        }
        self.teams.busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move { auth.sign_out().map_err(|e| e.to_string()) })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.busy = false;
                    this.teams.signed_in = false;
                    this.teams.listed = false;
                    this.teams.presence_running = false;
                    this.teams.player = None;
                    this.teams.player_title = None;
                    this.teams.list.clear();
                    this.teams.active = None;
                    this.teams.member_filter = None;
                    this.teams.panel = TeamsPanel::None;
                    this.teams.error = result.err();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// "Share a clip" from the desktop: pick an exported MP4, then run the
    /// create → TUS-upload → complete pipeline against the active team, all on
    /// background threads. Progress is surfaced via `teams_share_progress`; on
    /// success the team feed is reloaded so the new clip appears.
    pub fn share_clip_to_active_team(&mut self, cx: &mut Context<Self>) {
        if self.teams.busy || self.teams.sharing {
            return;
        }
        let Some(team) = self.active_team() else {
            return;
        };
        let team_id = team.id.clone();
        let auth = self.app_state.cloud_auth.clone();
        let start_dir = self.export_destination.clone();
        self.teams.error = None;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                // Pick the clip file (native dialog), defaulting to the export dir.
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title("Share a clip to your team")
                    .add_filter("Video", &["mp4", "mov", "webm", "mkv"]);
                if start_dir.is_dir() {
                    dialog = dialog.set_directory(&start_dir);
                }
                let Some(handle) = dialog.pick_file().await else {
                    return; // user cancelled
                };
                let path = handle.path().to_path_buf();

                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Clip")
                    .to_string();
                let file_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("clip.mp4")
                    .to_string();
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if size == 0 {
                    let _ = this.update(&mut cx, |this, cx| {
                        this.teams.error = Some("Selected file is empty or unreadable.".into());
                        cx.notify();
                    });
                    return;
                }

                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.sharing = true;
                    this.teams.share_progress = 0.0;
                    cx.notify();
                });

                // Shared progress cell: the upload writes it, a ticker repaints.
                let progress = std::sync::Arc::new(parking_lot::Mutex::new(0.0f32));
                let progress_ui = progress.clone();
                let ticker = this.clone();
                cx.spawn(move |cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        loop {
                            let still = ticker
                                .update(&mut cx, |this, cx| {
                                    if this.teams.sharing {
                                        this.teams.share_progress = *progress_ui.lock();
                                        cx.notify();
                                        true
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !still {
                                break;
                            }
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(150))
                                .await;
                        }
                    }
                })
                .detach();

                let team_id_bg = team_id.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let created = api::create_team_clip(
                            &auth,
                            &team_id_bg,
                            &title,
                            &file_name,
                            size,
                            None,
                        )
                        .map_err(|e| e.to_string())?;
                        crate::cloud::upload::tus_upload(
                            &created.upload,
                            &path,
                            &title,
                            |sent, total| {
                                *progress.lock() = if total > 0 {
                                    sent as f32 / total as f32
                                } else {
                                    0.0
                                };
                            },
                        )?;
                        api::complete_clip(&auth, &created.clip_id).map_err(|e| e.to_string())?;
                        Ok::<(), String>(())
                    })
                    .await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.sharing = false;
                    this.teams.share_progress = 0.0;
                    match result {
                        Ok(()) => {
                            // Refresh the feed if this team is still active.
                            if this.active_team().map(|t| t.id.as_str()) == Some(team_id.as_str()) {
                                this.load_active_team(cx);
                            }
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Start the presence heartbeat loop (idempotent). While the Teams tab is
    /// open and a team is active, POST `/presence` every 30s so the team's
    /// "X online" count stays live. The loop self-terminates on sign-out.
    pub fn start_presence_heartbeat(&mut self, cx: &mut Context<Self>) {
        if self.teams.presence_running || !self.teams.signed_in {
            return;
        }
        self.teams.presence_running = true;
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    // Read current state on the main thread; stop when signed out.
                    let team_id = match this.update(&mut cx, |this, _| {
                        if !this.teams.signed_in {
                            this.teams.presence_running = false;
                            return None;
                        }
                        if this.active_view == crate::ui::ActiveView::Teams {
                            this.active_team().map(|t| t.id.clone())
                        } else {
                            Some(String::new()) // alive but idle (tab not visible)
                        }
                    }) {
                        Ok(Some(id)) => id,
                        // Entity gone, or signed out → stop the loop.
                        _ => break,
                    };

                    if !team_id.is_empty() {
                        let auth = auth.clone();
                        let tid = team_id.clone();
                        let _ = cx
                            .background_executor()
                            .spawn(async move {
                                let _ = api::send_presence(&auth, &tid);
                            })
                            .await;
                    }

                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(30))
                        .await;
                }
            }
        })
        .detach();
    }

    /// Toggle the ❤ reaction on a clip. Updates the UI optimistically, calls
    /// `set_reaction` on a background thread, then reconciles with the server's
    /// authoritative count (reverting the optimistic change on failure).
    pub fn toggle_clip_reaction(
        &mut self,
        team_id: String,
        clip_id: String,
        cx: &mut Context<Self>,
    ) {
        // Optimistic update + capture the desired on/off state.
        let Some(team) = self.teams.list.iter_mut().find(|t| t.id == team_id) else {
            return;
        };
        let Some(clip) = team.clips.iter_mut().find(|c| c.id == clip_id) else {
            return;
        };
        let desired = !clip.reacted_by_me;
        clip.reacted_by_me = desired;
        clip.reactions = if desired {
            clip.reactions + 1
        } else {
            clip.reactions.saturating_sub(1)
        };
        cx.notify();

        let auth = self.app_state.cloud_auth.clone();
        let clip_id_bg = clip_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::set_reaction(&auth, &clip_id_bg, desired).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    let mut pending_err = None;
                    if let Some(team) = this.teams.list.iter_mut().find(|t| t.id == team_id) {
                        if let Some(clip) = team.clips.iter_mut().find(|c| c.id == clip_id) {
                            match result {
                                Ok(state) => {
                                    clip.reactions = state.reaction_count;
                                    clip.reacted_by_me = state.reacted_by_me;
                                }
                                Err(e) => {
                                    // Revert the optimistic change.
                                    clip.reacted_by_me = !desired;
                                    clip.reactions = if desired {
                                        clip.reactions.saturating_sub(1)
                                    } else {
                                        clip.reactions + 1
                                    };
                                    pending_err = Some(e);
                                }
                            }
                        }
                    }
                    // Record the error (and reconcile sign-in state) once the
                    // mutable borrow of `this.teams.list` above has been released.
                    if let Some(e) = pending_err {
                        this.note_cloud_error(e);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Fetch the active team's members + clip feed and populate it.
    pub fn load_active_team(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.teams.active else {
            return;
        };
        let Some(team) = self.teams.list.get(idx) else {
            return;
        };
        let team_id = team.id.clone();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let detail = api::get_team(&auth, &team_id).map_err(|e| e.to_string())?;
                        let feed = api::get_feed(&auth, &team_id).map_err(|e| e.to_string())?;
                        Ok::<_, String>((team_id, detail, feed))
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    match result {
                        Ok((team_id, detail, feed)) => {
                            if let Some(t) = this.teams.list.iter_mut().find(|t| t.id == team_id) {
                                let members = members_from_detail(&detail);
                                t.clips = clips_from_feed(&feed.items, &members);
                                t.members = members;
                                t.loaded = true;
                            }
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
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
}

// ── Team clip mini player ───────────────────────────────────────────
// A popup player that streams a clip's Bunny MP4 URL through libmpv, mirroring
// the Clips page mini player. The `video()` element self-drives repaints, so no
// separate tick loop is needed.
impl RekaptrWorkspace {
    /// Open `url` in the team mini player, replacing any clip already playing.
    pub fn open_team_clip(
        &mut self,
        url: String,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let old = self.teams.player.take();
        let d3d_device_ptr = self.app_state.d3d11_device.lock().as_ref().map(|h| h.0.0);
        match crate::video_player::Video::new_with_options(
            &url,
            crate::video_player::VideoOptions {
                source_name: Some("team-clip".to_string()),
                ..Default::default()
            },
            d3d_device_ptr,
        ) {
            Ok(video) => {
                self.teams.player = Some(video);
                self.teams.player_title = Some(title);
                self.teams.player_scrubbing = false;
            }
            Err(e) => {
                log::warn!("[Teams] failed to open clip player: {e:?}");
                self.teams.error = Some("Couldn't play this clip.".to_string());
            }
        }
        if let Some(old) = old {
            window.drop_image(old.render_image()).ok();
        }
        cx.notify();
    }

    /// Close the mini player and tear down its mpv instance.
    fn close_team_player(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.teams.player.take() {
            window.drop_image(old.render_image()).ok();
        }
        self.teams.player_title = None;
        self.teams.player_scrubbing = false;
        cx.notify();
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