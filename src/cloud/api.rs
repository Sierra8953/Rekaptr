//! Typed wrappers over the rekaptr.dev Teams API, authenticated with the
//! cloud session ([`CloudAuth`]). Blocking — call from a background thread.
//!
//! Each call attaches the bearer access token and, on a `401`, forces one
//! token refresh and retries before giving up (the refresh-on-401 path).
#![allow(dead_code)] // consumed by the Teams UI wiring (Phase 7)

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::auth::{CloudAuth, CloudAuthError};

type Result<T> = std::result::Result<T, CloudAuthError>;

// ── Response DTOs (snake_case, matching the API) ─────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub name: String,
    pub badge_tint: u32,
    pub initials: String,
    pub member_count: u32,
    pub online_count: u32,
    /// RFC 3339 timestamp of the team's most recent shared clip, or `None` if
    /// the feed is empty. Drives the "unread since last looked" badge.
    #[serde(default)]
    pub last_activity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TeamsResponse {
    teams: Vec<TeamSummary>,
}

#[derive(Debug, Deserialize)]
pub struct InviteInfo {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatedTeam {
    pub team: TeamSummary,
    pub invite: InviteInfo,
}

#[derive(Debug, Deserialize)]
pub struct AcceptedInvite {
    pub team_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberDto {
    pub user_id: String,
    pub display_name: String,
    pub handle: Option<String>,
    pub initial: String,
    pub avatar_tint: u32,
    pub online: bool,
    #[serde(default)]
    pub role: Option<String>,
}

/// The caller's own identity/role within a team, returned by `GET
/// /api/teams/{id}` so the client can gate author/owner-only actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeDto {
    pub user_id: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDetail {
    pub id: String,
    pub name: String,
    pub badge_tint: u32,
    pub initials: String,
    pub member_count: u32,
    pub clip_count: u32,
    #[serde(default)]
    pub me: Option<MeDto>,
    pub members: Vec<MemberDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorDto {
    pub user_id: String,
    pub display_name: String,
    pub initial: String,
    pub avatar_tint: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipDto {
    pub id: String,
    pub title: String,
    pub game: String,
    pub duration_ms: Option<u64>,
    pub created_at: String,
    pub status: String,
    pub thumb_url: Option<String>,
    pub video_url: Option<String>,
    #[serde(rename = "new")]
    pub is_new: bool,
    pub author: Option<AuthorDto>,
    #[serde(default)]
    pub reactions: Vec<ReactionTally>,
    #[serde(default)]
    pub comment_count: u32,
}

/// One emoji's tally on a clip: the glyph, how many gave it, and whether the
/// caller is one of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionTally {
    pub emoji: String,
    pub count: u32,
    pub reacted_by_me: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentAuthor {
    pub user_id: String,
    pub display_name: String,
    pub initial: String,
    pub avatar_tint: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentDto {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub mine: bool,
    #[serde(default)]
    pub author: Option<CommentAuthor>,
}

#[derive(Debug, Deserialize)]
struct CommentsResponse {
    items: Vec<CommentDto>,
}

#[derive(Debug, Deserialize)]
pub struct FeedPage {
    pub items: Vec<ClipDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReactionState {
    pub reactions: Vec<ReactionTally>,
}

/// Bunny Stream TUS upload auth returned by `POST /api/teams/{id}/clips`.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadAuth {
    pub endpoint: String,
    #[serde(rename = "libraryId")]
    pub library_id: u64,
    pub guid: String,
    pub signature: String,
    pub expire: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedClip {
    pub clip_id: String,
    pub upload: UploadAuth,
}

// ── Session ─────────────────────────────────────────────────────────
/// A reusable API client. Holds one `reqwest::blocking::Client`, so a chain of
/// calls (the cold load: teams → detail → feed) shares a single keep-alive
/// connection instead of paying a fresh TCP+TLS handshake per request. Cheap to
/// `clone()` (the client's connection pool is shared). Build and drop it on a
/// background thread — the blocking client owns a private runtime (see
/// `auth::blocking_client`).
#[derive(Clone)]
pub struct ApiSession {
    client: reqwest::blocking::Client,
}

impl Default for ApiSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiSession {
    pub fn new() -> Self {
        Self { client: super::auth::blocking_client() }
    }

    // ── Endpoints ───────────────────────────────────────────────────
    /// `GET /api/teams` — the user's teams (left rail).
    pub fn list_teams(&self, auth: &CloudAuth) -> Result<Vec<TeamSummary>> {
        let r: TeamsResponse = self.get_json(auth, "/api/teams")?;
        Ok(r.teams)
    }

    /// `GET /api/teams/{id}` — header + members.
    pub fn get_team(&self, auth: &CloudAuth, team_id: &str) -> Result<TeamDetail> {
        self.get_json(auth, &format!("/api/teams/{team_id}"))
    }

    /// `GET /api/teams/{id}/clips` — the shared-clip feed (first page).
    pub fn get_feed(&self, auth: &CloudAuth, team_id: &str) -> Result<FeedPage> {
        self.get_json(auth, &format!("/api/teams/{team_id}/clips"))
    }

    /// `POST /api/teams` — create a team; returns it plus the default invite code.
    pub fn create_team(&self, auth: &CloudAuth, name: &str) -> Result<CreatedTeam> {
        self.post_json(auth, "/api/teams", &serde_json::json!({ "name": name }))
    }

    /// `POST /api/invites/{code}/accept` — join a team by code.
    pub fn accept_invite(&self, auth: &CloudAuth, code: &str) -> Result<AcceptedInvite> {
        self.post_json(auth, &format!("/api/invites/{}/accept", code.trim()), &EMPTY)
    }

    /// `POST /api/teams/{id}/clips` — "Share a clip", step 1. Registers a Bunny
    /// Stream video + a PENDING clip row and returns TUS upload auth.
    pub fn create_team_clip(
        &self,
        auth: &CloudAuth,
        team_id: &str,
        title: &str,
        file_name: &str,
        size: u64,
        game: Option<&str>,
    ) -> Result<CreatedClip> {
        self.post_json(
            auth,
            &format!("/api/teams/{team_id}/clips"),
            &serde_json::json!({
                "title": title,
                "fileName": file_name,
                "size": size,
                "gameName": game.filter(|g| !g.trim().is_empty()),
            }),
        )
    }

    /// `POST /api/clips/{id}/complete` — "Share a clip", step 3. Confirms the TUS
    /// upload landed on Bunny and flips the clip to READY.
    pub fn complete_clip(&self, auth: &CloudAuth, clip_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::POST,
            &format!("/api/clips/{clip_id}/complete"),
            None::<&()>,
        )?;
        if !resp.status().is_success() {
            let status = resp.status();
            let msg = resp
                .json::<ApiError>()
                .map(|e| e.error)
                .unwrap_or_else(|_| status.to_string());
            return Err(CloudAuthError::Token(format!("{status}: {msg}")));
        }
        Ok(())
    }

    /// `POST /api/teams/{id}/presence` — heartbeat so the team's "X online" count
    /// stays live while the Teams tab is open. Best-effort; the body is ignored.
    pub fn send_presence(&self, auth: &CloudAuth, team_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::POST,
            &format!("/api/teams/{team_id}/presence"),
            None::<&()>,
        )?;
        if !resp.status().is_success() {
            return Err(CloudAuthError::Token(resp.status().to_string()));
        }
        Ok(())
    }

    /// Add or remove an emoji reaction on a clip (`PUT` to add, `DELETE` to
    /// remove). Returns the clip's updated per-emoji tallies.
    pub fn set_reaction(
        &self,
        auth: &CloudAuth,
        clip_id: &str,
        emoji: &str,
        on: bool,
    ) -> Result<ReactionState> {
        let path = format!("/api/clips/{clip_id}/reaction");
        let method = if on { reqwest::Method::PUT } else { reqwest::Method::DELETE };
        let resp = self.send(auth, method, &path, Some(&serde_json::json!({ "emoji": emoji })))?;
        json_or_err(resp)
    }

    /// `PATCH /api/clips/{id}` — rename a clip (author only).
    pub fn rename_clip(&self, auth: &CloudAuth, clip_id: &str, title: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::PATCH,
            &format!("/api/clips/{clip_id}"),
            Some(&serde_json::json!({ "title": title })),
        )?;
        expect_ok(resp)
    }

    /// `DELETE /api/clips/{id}` — permanently delete a clip from the author's
    /// library; it disappears from every team it was shared into. Author only.
    pub fn delete_clip(&self, auth: &CloudAuth, clip_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::DELETE,
            &format!("/api/clips/{clip_id}"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    /// `DELETE /api/teams/{id}/shares?videoId=` — remove a clip from one team's
    /// feed without deleting the media. Author or team admin/owner.
    pub fn unshare_clip(&self, auth: &CloudAuth, team_id: &str, clip_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::DELETE,
            &format!("/api/teams/{team_id}/shares?videoId={clip_id}"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    /// `GET /api/clips/{id}/comments` — the clip's comment thread (oldest first).
    pub fn get_comments(&self, auth: &CloudAuth, clip_id: &str) -> Result<Vec<CommentDto>> {
        let r: CommentsResponse =
            self.get_json(auth, &format!("/api/clips/{clip_id}/comments"))?;
        Ok(r.items)
    }

    /// `POST /api/clips/{id}/comments` — add a comment; returns the created row.
    pub fn post_comment(&self, auth: &CloudAuth, clip_id: &str, body: &str) -> Result<CommentDto> {
        self.post_json(
            auth,
            &format!("/api/clips/{clip_id}/comments"),
            &serde_json::json!({ "body": body }),
        )
    }

    /// `DELETE /api/comments/{id}` — delete a comment (author or team moderator).
    pub fn delete_comment(&self, auth: &CloudAuth, comment_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::DELETE,
            &format!("/api/comments/{comment_id}"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    /// `GET /api/teams/{id}/invites` — the team's current active invite code
    /// (mints one if none). Admin+.
    pub fn get_invite(&self, auth: &CloudAuth, team_id: &str) -> Result<String> {
        let r: InviteInfo = self.get_json(auth, &format!("/api/teams/{team_id}/invites"))?;
        Ok(r.code)
    }

    /// `POST /api/teams/{id}/invites` — mint a fresh invite code. Admin+.
    pub fn regenerate_invite(&self, auth: &CloudAuth, team_id: &str) -> Result<String> {
        let r: InviteInfo =
            self.post_json(auth, &format!("/api/teams/{team_id}/invites"), &EMPTY)?;
        Ok(r.code)
    }

    /// `DELETE /api/teams/{id}/members/{userId}` — remove a member. Admin+.
    pub fn remove_member(&self, auth: &CloudAuth, team_id: &str, user_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::DELETE,
            &format!("/api/teams/{team_id}/members/{user_id}"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    /// `PATCH /api/teams/{id}/members/{userId}` — change a member's role (owner
    /// only). `role` of "OWNER" transfers ownership.
    pub fn set_member_role(
        &self,
        auth: &CloudAuth,
        team_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::PATCH,
            &format!("/api/teams/{team_id}/members/{user_id}"),
            Some(&serde_json::json!({ "role": role })),
        )?;
        expect_ok(resp)
    }

    /// `POST /api/teams/{id}/leave` — leave a team.
    pub fn leave_team(&self, auth: &CloudAuth, team_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::POST,
            &format!("/api/teams/{team_id}/leave"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    /// `DELETE /api/teams/{id}` — soft-delete a team (owner only).
    pub fn delete_team(&self, auth: &CloudAuth, team_id: &str) -> Result<()> {
        let resp = self.send(
            auth,
            reqwest::Method::DELETE,
            &format!("/api/teams/{team_id}"),
            None::<&()>,
        )?;
        expect_ok(resp)
    }

    // ── transport ───────────────────────────────────────────────────
    fn get_json<T: DeserializeOwned>(&self, auth: &CloudAuth, path: &str) -> Result<T> {
        let resp = self.send(auth, reqwest::Method::GET, path, None::<&()>)?;
        json_or_err(resp)
    }

    fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        auth: &CloudAuth,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.send(auth, reqwest::Method::POST, path, Some(body))?;
        json_or_err(resp)
    }

    /// Send an authenticated request over this session's client, retrying once
    /// after a forced token refresh on a `401`.
    fn send<B: Serialize>(
        &self,
        auth: &CloudAuth,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<reqwest::blocking::Response> {
        let url = format!("{}{}", auth.api_base(), path);
        let build = |token: &str| {
            let mut req = self.client.request(method.clone(), &url).bearer_auth(token);
            if let Some(b) = body {
                req = req.json(b);
            }
            req
        };

        let token = auth.access_token()?;
        let resp = build(&token).send()?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let token = auth.force_refresh()?;
            return Ok(build(&token).send()?);
        }
        Ok(resp)
    }
}

// ── Free-function wrappers ──────────────────────────────────────────
// One-shot calls keep these (each builds its own connection). Chains that want
// connection reuse build one `ApiSession` and call its methods directly.
const EMPTY: serde_json::Value = serde_json::Value::Null;

pub fn list_teams(auth: &CloudAuth) -> Result<Vec<TeamSummary>> {
    ApiSession::new().list_teams(auth)
}
pub fn get_team(auth: &CloudAuth, team_id: &str) -> Result<TeamDetail> {
    ApiSession::new().get_team(auth, team_id)
}
pub fn get_feed(auth: &CloudAuth, team_id: &str) -> Result<FeedPage> {
    ApiSession::new().get_feed(auth, team_id)
}
pub fn create_team(auth: &CloudAuth, name: &str) -> Result<CreatedTeam> {
    ApiSession::new().create_team(auth, name)
}
pub fn accept_invite(auth: &CloudAuth, code: &str) -> Result<AcceptedInvite> {
    ApiSession::new().accept_invite(auth, code)
}
pub fn create_team_clip(
    auth: &CloudAuth,
    team_id: &str,
    title: &str,
    file_name: &str,
    size: u64,
    game: Option<&str>,
) -> Result<CreatedClip> {
    ApiSession::new().create_team_clip(auth, team_id, title, file_name, size, game)
}
pub fn complete_clip(auth: &CloudAuth, clip_id: &str) -> Result<()> {
    ApiSession::new().complete_clip(auth, clip_id)
}
pub fn send_presence(auth: &CloudAuth, team_id: &str) -> Result<()> {
    ApiSession::new().send_presence(auth, team_id)
}
pub fn set_reaction(
    auth: &CloudAuth,
    clip_id: &str,
    emoji: &str,
    on: bool,
) -> Result<ReactionState> {
    ApiSession::new().set_reaction(auth, clip_id, emoji, on)
}
pub fn rename_clip(auth: &CloudAuth, clip_id: &str, title: &str) -> Result<()> {
    ApiSession::new().rename_clip(auth, clip_id, title)
}
pub fn delete_clip(auth: &CloudAuth, clip_id: &str) -> Result<()> {
    ApiSession::new().delete_clip(auth, clip_id)
}
pub fn unshare_clip(auth: &CloudAuth, team_id: &str, clip_id: &str) -> Result<()> {
    ApiSession::new().unshare_clip(auth, team_id, clip_id)
}
pub fn get_comments(auth: &CloudAuth, clip_id: &str) -> Result<Vec<CommentDto>> {
    ApiSession::new().get_comments(auth, clip_id)
}
pub fn post_comment(auth: &CloudAuth, clip_id: &str, body: &str) -> Result<CommentDto> {
    ApiSession::new().post_comment(auth, clip_id, body)
}
pub fn delete_comment(auth: &CloudAuth, comment_id: &str) -> Result<()> {
    ApiSession::new().delete_comment(auth, comment_id)
}
pub fn get_invite(auth: &CloudAuth, team_id: &str) -> Result<String> {
    ApiSession::new().get_invite(auth, team_id)
}
pub fn regenerate_invite(auth: &CloudAuth, team_id: &str) -> Result<String> {
    ApiSession::new().regenerate_invite(auth, team_id)
}
pub fn remove_member(auth: &CloudAuth, team_id: &str, user_id: &str) -> Result<()> {
    ApiSession::new().remove_member(auth, team_id, user_id)
}
pub fn set_member_role(auth: &CloudAuth, team_id: &str, user_id: &str, role: &str) -> Result<()> {
    ApiSession::new().set_member_role(auth, team_id, user_id, role)
}
pub fn leave_team(auth: &CloudAuth, team_id: &str) -> Result<()> {
    ApiSession::new().leave_team(auth, team_id)
}
pub fn delete_team(auth: &CloudAuth, team_id: &str) -> Result<()> {
    ApiSession::new().delete_team(auth, team_id)
}

/// Like [`json_or_err`] but for endpoints whose body we don't need — just
/// confirm a 2xx, mapping a non-success status to the API's `{ error }` message.
fn expect_ok(resp: reqwest::blocking::Response) -> Result<()> {
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let msg = resp
        .json::<ApiError>()
        .map(|e| e.error)
        .unwrap_or_else(|_| status.to_string());
    Err(CloudAuthError::Token(format!("{status}: {msg}")))
}

fn json_or_err<T: DeserializeOwned>(resp: reqwest::blocking::Response) -> Result<T> {
    if !resp.status().is_success() {
        let status = resp.status();
        // The API returns `{ "error": "...", "code"?: "..." }`.
        let msg = resp
            .json::<ApiError>()
            .map(|e| e.error)
            .unwrap_or_else(|_| status.to_string());
        return Err(CloudAuthError::Token(format!("{status}: {msg}")));
    }
    Ok(resp.json()?)
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}
