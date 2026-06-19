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
#[derive(Debug, Clone, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub name: String,
    pub badge_tint: u32,
    pub initials: String,
    pub member_count: u32,
    pub online_count: u32,
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct TeamDetail {
    pub id: String,
    pub name: String,
    pub badge_tint: u32,
    pub initials: String,
    pub member_count: u32,
    pub clip_count: u32,
    pub members: Vec<MemberDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorDto {
    pub user_id: String,
    pub display_name: String,
    pub initial: String,
    pub avatar_tint: u32,
}

#[derive(Debug, Clone, Deserialize)]
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
    pub reaction_count: u32,
    pub reacted_by_me: bool,
}

#[derive(Debug, Deserialize)]
pub struct FeedPage {
    pub items: Vec<ClipDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReactionState {
    pub reaction_count: u32,
    pub reacted_by_me: bool,
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

// ── Endpoints ───────────────────────────────────────────────────────
/// `GET /api/teams` — the user's teams (left rail).
pub fn list_teams(auth: &CloudAuth) -> Result<Vec<TeamSummary>> {
    let r: TeamsResponse = get_json(auth, "/api/teams")?;
    Ok(r.teams)
}

/// `GET /api/teams/{id}` — header + members.
pub fn get_team(auth: &CloudAuth, team_id: &str) -> Result<TeamDetail> {
    get_json(auth, &format!("/api/teams/{team_id}"))
}

/// `GET /api/teams/{id}/clips` — the shared-clip feed (first page).
pub fn get_feed(auth: &CloudAuth, team_id: &str) -> Result<FeedPage> {
    get_json(auth, &format!("/api/teams/{team_id}/clips"))
}

/// `POST /api/teams` — create a team; returns it plus the default invite code.
pub fn create_team(auth: &CloudAuth, name: &str) -> Result<CreatedTeam> {
    post_json(auth, "/api/teams", &serde_json::json!({ "name": name }))
}

/// `POST /api/invites/{code}/accept` — join a team by code.
pub fn accept_invite(auth: &CloudAuth, code: &str) -> Result<AcceptedInvite> {
    post_json(auth, &format!("/api/invites/{}/accept", code.trim()), &EMPTY)
}

/// `POST /api/teams/{id}/clips` — "Share a clip", step 1. Registers a Bunny
/// Stream video + a PENDING clip row and returns TUS upload auth. The caller
/// then TUS-uploads the MP4 and calls [`complete_clip`].
pub fn create_team_clip(
    auth: &CloudAuth,
    team_id: &str,
    title: &str,
    file_name: &str,
    size: u64,
    game: Option<&str>,
) -> Result<CreatedClip> {
    post_json(
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
pub fn complete_clip(auth: &CloudAuth, clip_id: &str) -> Result<()> {
    let resp = send(
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
pub fn send_presence(auth: &CloudAuth, team_id: &str) -> Result<()> {
    let resp = send(
        auth,
        reqwest::Method::POST,
        &format!("/api/teams/{team_id}/presence"),
        None::<&()>,
    )?;
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(CloudAuthError::Token(status.to_string()));
    }
    Ok(())
}

/// Toggle the ❤ reaction on a clip (`PUT` to add, `DELETE` to remove).
pub fn set_reaction(auth: &CloudAuth, clip_id: &str, on: bool) -> Result<ReactionState> {
    let path = format!("/api/clips/{clip_id}/reaction");
    let method = if on { reqwest::Method::PUT } else { reqwest::Method::DELETE };
    let resp = send(auth, method, &path, None::<&()>)?;
    json_or_err(resp)
}

// ── transport ───────────────────────────────────────────────────────
const EMPTY: serde_json::Value = serde_json::Value::Null;

fn get_json<T: DeserializeOwned>(auth: &CloudAuth, path: &str) -> Result<T> {
    let resp = send(auth, reqwest::Method::GET, path, None::<&()>)?;
    json_or_err(resp)
}

fn post_json<B: Serialize, T: DeserializeOwned>(auth: &CloudAuth, path: &str, body: &B) -> Result<T> {
    let resp = send(auth, reqwest::Method::POST, path, Some(body))?;
    json_or_err(resp)
}

/// Send an authenticated request, retrying once after a forced token refresh on
/// a `401`.
fn send<B: Serialize>(
    auth: &CloudAuth,
    method: reqwest::Method,
    path: &str,
    body: Option<&B>,
) -> Result<reqwest::blocking::Response> {
    let url = format!("{}{}", auth.api_base(), path);
    let client = super::auth::blocking_client();
    let build = |token: &str| {
        let mut req = client.request(method.clone(), &url).bearer_auth(token);
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
