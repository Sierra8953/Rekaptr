//! Teams view — the app's one networked surface: create or join a team and
//! browse clips shared by teammates. Local-first everywhere else, this tab is
//! intended to be backed by the rekaptr.dev backend (accounts, membership, clip
//! hosting). That backend is not wired yet, so the data here is local/mock:
//! "Join a team" pulls in a populated demo team, "Create a team" makes a fresh
//! empty one. The rendering uses the app's theme tokens and component patterns,
//! mirroring the Clips view.

use crate::ui::RekaptrWorkspace;
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

mod data;
mod view;

impl RekaptrWorkspace {
    fn active_team(&self) -> Option<&Team> {
        self.teams.active
            .and_then(|i| self.teams.list.get(i))
            .or_else(|| self.teams.list.first())
    }
}
