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

// Online-presence green is a semantic accent, not a brand color, so it stays
// literal rather than tracking `theme.tokens.primary`.
const ONLINE: u32 = 0x34d399;

#[derive(Clone)]
pub struct Member {
    pub user_id: String,
    pub name: String,
    pub initial: String,
    pub tint: u32,
    pub online: bool,
    /// "OWNER" / "ADMIN" / "MEMBER" (empty if unknown).
    pub role: String,
}

#[derive(Clone)]
pub struct TeamClip {
    pub id: String,
    /// `id` as a `SharedString`, precomputed once so render can build the card's
    /// element/group ids as cheap `(el_id, n)` Arc-clones instead of `format!`-
    /// ing four strings per card per frame (mirrors `Clip::path_str`).
    pub el_id: SharedString,
    pub title: String,
    pub game: String,
    pub author: usize, // index into the team's members
    pub when: String,
    /// Clip creation time as a Unix timestamp (seconds), parsed from the API's
    /// `created_at`. Drives the feed sort/filter; `when` is its display form.
    pub created_unix: i64,
    pub duration: String,
    pub thumb_tint: u32,
    /// Bunny-generated thumbnail URL, shown once the clip is READY. Falls back
    /// to `thumb_tint` while transcoding / if absent.
    pub thumb_url: Option<String>,
    /// Per-emoji reaction tallies for this clip, most-used first.
    pub reactions: Vec<ReactionTally>,
    pub comment_count: u32,
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
    /// The signed-in user's own id, as reported for this team. Empty until the
    /// team detail loads. Used to gate author-only clip actions.
    pub me_user_id: String,
    /// The signed-in user's role in this team ("OWNER"/"ADMIN"/"MEMBER"), empty
    /// until loaded. Gates admin-only actions (unshare others' clips, manage).
    pub my_role: String,
    /// Unix timestamp (seconds) of the team's most recent shared clip, 0 if the
    /// feed is empty/unknown. Compared against the local "seen" mark to decide
    /// whether the team has unread activity.
    pub last_activity_unix: i64,
    /// Whether members/clips have been fetched for this team yet.
    pub loaded: bool,
}

impl Team {
    /// Whether the signed-in user authored `clip` (so may rename/delete it).
    pub fn clip_is_mine(&self, clip: &TeamClip) -> bool {
        !self.me_user_id.is_empty()
            && self
                .members
                .get(clip.author)
                .is_some_and(|m| m.user_id == self.me_user_id)
    }

    /// Whether the signed-in user is an admin or owner of this team.
    pub fn i_am_admin(&self) -> bool {
        self.my_role == "OWNER" || self.my_role == "ADMIN"
    }
}

/// One emoji's tally on a clip, for rendering the reaction chips.
#[derive(Clone)]
pub struct ReactionTally {
    pub emoji: String,
    pub count: u32,
    /// Whether the signed-in user reacted with this emoji.
    pub mine: bool,
}

/// The emoji palette offered by the reaction picker (must match the server's
/// allowed set in `clips/[clipId]/reaction/route.ts`).
pub const REACTION_EMOJI: [&str; 8] = ["❤️", "🔥", "😂", "🎯", "💀", "👏", "😮", "🎉"];

/// A single comment, mapped from the API for rendering in the thread panel.
#[derive(Clone)]
pub struct CommentItem {
    pub id: String,
    /// The comment author's user id, used as the avatar seed (empty if unknown).
    pub author_user_id: String,
    pub author_name: String,
    pub author_initial: String,
    pub author_tint: u32,
    pub body: String,
    pub when: String,
    /// Whether the signed-in user can delete this comment (author or moderator).
    pub can_delete: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TeamsPanel {
    None,
    Create,
    Join,
}

/// Ordering applied to the active team's clip feed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipSort {
    /// Most recently shared first (the feed's natural order).
    Newest,
    /// Most-reacted first.
    Top,
    /// Only clips created in the last 7 days, newest first.
    Week,
}

impl ClipSort {
    pub fn label(self) -> &'static str {
        match self {
            ClipSort::Newest => "Newest",
            ClipSort::Top => "Top",
            ClipSort::Week => "This week",
        }
    }

    pub const ALL: [ClipSort; 3] = [ClipSort::Newest, ClipSort::Top, ClipSort::Week];
}

/// All Teams-tab view state, grouped out of the `RekaptrWorkspace` god-object.
/// Local/mock until the rekaptr.dev backend is fully wired.
pub struct TeamsState {
    /// The user's teams (left rail).
    pub list: Vec<Team>,
    pub active: Option<usize>,
    pub member_filter: Option<usize>,
    /// How the active team's feed is ordered/filtered.
    pub sort: ClipSort,
    /// When set, the feed is limited to clips tagged with this game.
    pub game_filter: Option<String>,
    pub panel: TeamsPanel,
    pub name_input: Entity<adabraka_ui::components::input_state::InputState>,
    pub join_code_input: Entity<adabraka_ui::components::input_state::InputState>,
    /// The clip id whose emoji reaction picker is open, if any.
    pub reaction_picker: Option<String>,
    /// The clip id whose comment thread is open, if any.
    pub comments_open: Option<String>,
    /// The loaded comment thread for `comments_open`.
    pub comments: Vec<CommentItem>,
    /// Whether the comment thread is currently being fetched.
    pub comments_loading: bool,
    /// Whether a comment post/delete is in flight.
    pub comment_busy: bool,
    pub comment_input: Entity<adabraka_ui::components::input_state::InputState>,
    /// Whether the member-management panel is open (for the active team).
    pub members_open: bool,
    /// Whether the bottom-rail account menu (profile / settings / sign out) is open.
    pub account_menu_open: bool,
    /// A member-management cloud op (role/remove/invite/leave/delete) is running.
    pub member_busy: bool,
    /// The clip id whose "···" actions menu is currently open, if any.
    pub clip_menu: Option<String>,
    /// The clip id being renamed (drives the rename modal), if any.
    pub rename_target: Option<String>,
    pub rename_input: Entity<adabraka_ui::components::input_state::InputState>,
    /// Cloud (Clerk) sign-in state for the Teams tab.
    pub signed_in: bool,
    /// A cloud request (sign-in / list / create / join / load) is in flight.
    pub busy: bool,
    /// A sign-out is specifically in flight. Distinct from `busy` so the
    /// sign-out control only reads "Signing out…" during an actual sign-out,
    /// not during the first-load team fetch (which also sets `busy`).
    pub signing_out: bool,
    /// Whether the team list has been fetched at least once this session.
    pub listed: bool,
    /// Last cloud error, surfaced inline in the Teams tab.
    pub error: Option<String>,
    /// Whether the presence-heartbeat loop is already running (prevents dupes).
    pub presence_running: bool,
    /// Per-team last-seen marks (team id → unix secs), persisted locally. A team
    /// reads as "unread" when its `last_activity_unix` exceeds its seen mark.
    pub seen: std::collections::HashMap<String, i64>,
    /// Whether `seen` has been loaded from disk this session.
    pub seen_loaded: bool,
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
            sort: ClipSort::Newest,
            game_filter: None,
            panel: TeamsPanel::None,
            name_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            join_code_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            reaction_picker: None,
            comments_open: None,
            comments: Vec::new(),
            comments_loading: false,
            comment_busy: false,
            comment_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            members_open: false,
            account_menu_open: false,
            member_busy: false,
            clip_menu: None,
            rename_target: None,
            rename_input: cx.new(|cx| adabraka_ui::components::input_state::InputState::new(cx)),
            signed_in,
            busy: false,
            signing_out: false,
            listed: false,
            error: None,
            presence_running: false,
            seen: std::collections::HashMap::new(),
            seen_loaded: false,
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

    /// The active team's id, owned (so callers can then take `&mut self`).
    pub fn active_team_id(&self) -> Option<String> {
        self.active_team().map(|t| t.id.clone())
    }

    /// The signed-in user's own display info (user id, name, initial, avatar
    /// tint), derived from whichever loaded team lists them as a member. The id
    /// doubles as the DiceBear avatar seed. Returns `None` until a team detail
    /// has loaded (carrying `me_user_id` + the roster). Avoids a dedicated
    /// `/api/me` round trip — the data is already on hand.
    pub fn my_profile(&self) -> Option<(String, String, String, u32)> {
        self.teams.list.iter().find_map(|t| {
            if t.me_user_id.is_empty() {
                return None;
            }
            t.members
                .iter()
                .find(|m| m.user_id == t.me_user_id)
                .map(|m| (m.user_id.clone(), m.name.clone(), m.initial.clone(), m.tint))
        })
    }

    /// Whether a team has activity newer than the local "seen" mark.
    pub fn team_is_unread(&self, team: &Team) -> bool {
        let seen = self.teams.seen.get(&team.id).copied().unwrap_or(0);
        team.last_activity_unix > seen
    }

    /// How many of the user's teams currently have unread activity (drives the
    /// sidebar nav badge).
    pub fn unread_team_count(&self) -> usize {
        self.teams.list.iter().filter(|t| self.team_is_unread(t)).count()
    }
}
