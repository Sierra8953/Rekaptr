//! Teams data layer: cloud fetch/reconcile and actions (sign-in/out, reload,
//! load/share/react, presence heartbeat) plus the API->UI mapping helpers.

use super::*;
use crate::cloud::api;
use crate::ui::RekaptrWorkspace;

// ── Persisted cache (stale-while-revalidate) ────────────────────────
// The last successful teams fetch, written to disk so a cold launch can paint
// the Teams tab instantly from cache while the startup prefetch revalidates in
// the background. Cleared on sign-out so a different account never sees it.

#[derive(serde::Serialize, serde::Deserialize)]
struct TeamsCache {
    teams: Vec<api::TeamSummary>,
    #[serde(default)]
    active_detail: Option<api::TeamDetail>,
    #[serde(default)]
    active_feed: Option<Vec<api::ClipDto>>,
}

fn teams_cache_path() -> std::path::PathBuf {
    let dir = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Rekaptr");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("teams_cache.json")
}

/// Persist the latest fetch (best-effort). Call from a background thread.
fn write_teams_cache(teams: &[api::TeamSummary], active: Option<(&api::TeamDetail, &[api::ClipDto])>) {
    let cache = TeamsCache {
        teams: teams.to_vec(),
        active_detail: active.map(|(d, _)| d.clone()),
        active_feed: active.map(|(_, f)| f.to_vec()),
    };
    if let Ok(bytes) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(teams_cache_path(), bytes);
    }
}

fn read_teams_cache() -> Option<TeamsCache> {
    serde_json::from_slice(&std::fs::read(teams_cache_path()).ok()?).ok()
}

fn clear_teams_cache() {
    let _ = std::fs::remove_file(teams_cache_path());
}

// ── Per-team "seen" marks (unread badge) ─────────────────────────────
// team id → unix secs of when the user last viewed that team's feed. Persisted
// so the unread badge survives restarts. Cleared on sign-out.

fn teams_seen_path() -> std::path::PathBuf {
    let dir = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Rekaptr");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("teams_seen.json")
}

fn read_seen() -> std::collections::HashMap<String, i64> {
    std::fs::read(teams_seen_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_seen(seen: &std::collections::HashMap<String, i64>) {
    if let Ok(bytes) = serde_json::to_vec(seen) {
        let _ = std::fs::write(teams_seen_path(), bytes);
    }
}

fn clear_seen() {
    let _ = std::fs::remove_file(teams_seen_path());
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
        me_user_id: String::new(),
        my_role: String::new(),
        last_activity_unix: s.last_activity.as_deref().map(parse_unix).unwrap_or(0),
        loaded: false,
    }
}

/// Fill a team's members/clips/identity from a freshly fetched detail + feed.
/// Shared by the cold-load, per-team load, and cache-seed paths.
fn populate_team_from_detail(t: &mut Team, detail: &api::TeamDetail, feed_items: &[api::ClipDto]) {
    let members = members_from_detail(detail);
    t.clips = clips_from_feed(feed_items, &members);
    t.members = members;
    t.me_user_id = detail.me.as_ref().map(|m| m.user_id.clone()).unwrap_or_default();
    t.my_role = detail
        .me
        .as_ref()
        .and_then(|m| m.role.clone())
        .unwrap_or_default();
    t.loaded = true;
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
            role: m.role.clone().unwrap_or_default(),
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
                el_id: c.id.clone().into(),
                title: c.title.clone(),
                game: c.game.clone(),
                author,
                when: relative_time(&c.created_at),
                created_unix: parse_unix(&c.created_at),
                duration: c.duration_ms.map(fmt_duration).unwrap_or_default(),
                thumb_tint: thumb_tint_for(&c.id),
                thumb_url: c.thumb_url.clone(),
                reactions: c.reactions.iter().map(tally_from_dto).collect(),
                comment_count: c.comment_count,
                new: c.is_new,
                video_url: c.video_url.clone(),
            }
        })
        .collect()
}

/// Optimistically apply (or undo) the caller's reaction with `emoji` to a
/// clip's tally list, in place. Adds/removes the tally entry as needed.
fn apply_reaction_toggle(tallies: &mut Vec<ReactionTally>, emoji: &str, on: bool) {
    if let Some(pos) = tallies.iter().position(|t| t.emoji == emoji) {
        let t = &mut tallies[pos];
        if on && !t.mine {
            t.mine = true;
            t.count += 1;
        } else if !on && t.mine {
            t.mine = false;
            t.count = t.count.saturating_sub(1);
            if t.count == 0 {
                tallies.remove(pos);
            }
        }
    } else if on {
        tallies.push(ReactionTally {
            emoji: emoji.to_string(),
            count: 1,
            mine: true,
        });
    }
}

fn tally_from_dto(t: &api::ReactionTally) -> ReactionTally {
    ReactionTally {
        emoji: t.emoji.clone(),
        count: t.count,
        mine: t.reacted_by_me,
    }
}

fn comment_from_dto(c: api::CommentDto, can_moderate: bool) -> CommentItem {
    let (author_user_id, author_name, author_initial, author_tint) = c
        .author
        .as_ref()
        .map(|a| (a.user_id.clone(), a.display_name.clone(), a.initial.clone(), a.avatar_tint))
        .unwrap_or_else(|| (String::new(), "Unknown".to_string(), "?".to_string(), 0x6b7280));
    CommentItem {
        id: c.id,
        author_user_id,
        author_name,
        author_initial,
        author_tint,
        body: c.body,
        when: relative_time(&c.created_at),
        can_delete: c.mine || can_moderate,
    }
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

/// Parse an RFC 3339 timestamp to a Unix timestamp (seconds). Returns 0 on a
/// malformed value so such clips sort to the end of a newest-first list.
fn parse_unix(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
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
    /// Resolve the Create / Join modal. With no backend yet, this builds local
    /// state: Create makes a fresh team with just "You"; Join pulls in the
    /// populated demo team. Selects the resulting team and closes the modal.
    /// Resolve the Create / Join modal against the cloud API. Both paths run on
    /// a background thread (the calls are blocking) and then refresh the team
    /// list and select the resulting team.
    pub(super) fn confirm_teams_panel(&mut self, is_create: bool, window: &mut Window, cx: &mut Context<Self>) {
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
                            // One session → create + re-list share a connection.
                            let s = api::ApiSession::new();
                            let created =
                                s.create_team(&auth, &name).map_err(|e| e.to_string())?;
                            let teams = s.list_teams(&auth).map_err(|e| e.to_string())?;
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
                            // One session → accept + re-list share a connection.
                            let s = api::ApiSession::new();
                            let accepted =
                                s.accept_invite(&auth, &code).map_err(|e| e.to_string())?;
                            let teams = s.list_teams(&auth).map_err(|e| e.to_string())?;
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
                            this.sync_seen_marks();
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
    /// Seed the Teams state from the on-disk cache so the tab paints instantly
    /// on a cold launch. The startup prefetch (`reload_teams`) then revalidates
    /// and replaces this with fresh data. No-op if already loaded or no cache.
    pub fn seed_teams_from_cache(&mut self) {
        if self.teams.listed {
            return;
        }
        let Some(cache) = read_teams_cache() else { return };
        if cache.teams.is_empty() {
            return;
        }
        self.teams.list = cache.teams.into_iter().map(team_from_summary).collect();
        self.teams.listed = true;
        self.teams.active = Some(0);
        if let (Some(detail), Some(feed)) = (cache.active_detail, cache.active_feed) {
            if let Some(t) = self.teams.list.iter_mut().find(|t| t.id == detail.id) {
                populate_team_from_detail(t, &detail, &feed);
            }
        }
        self.sync_seen_marks();
    }

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
                // Cold load over a single connection: list the teams, then fetch
                // the default-active (first) team's detail + feed on the *same*
                // session, so the whole first paint pays one TCP+TLS handshake
                // instead of three.
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let s = api::ApiSession::new();
                        let teams = s.list_teams(&auth).map_err(|e| e.to_string())?;
                        let active_detail = match teams.first() {
                            Some(t) => {
                                let id = t.id.clone();
                                let detail = s.get_team(&auth, &id).map_err(|e| e.to_string())?;
                                let feed = s.get_feed(&auth, &id).map_err(|e| e.to_string())?;
                                Some((detail, feed))
                            }
                            None => None,
                        };
                        // Persist for the next cold launch's instant paint.
                        write_teams_cache(
                            &teams,
                            active_detail.as_ref().map(|(d, f)| (d, f.items.as_slice())),
                        );
                        Ok::<_, String>((teams, active_detail))
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.busy = false;
                    match result {
                        Ok((summaries, active_detail)) => {
                            this.teams.list = summaries.into_iter().map(team_from_summary).collect();
                            this.teams.listed = true;
                            this.teams.error = None;
                            this.sync_seen_marks();
                            if this.teams.active.map_or(true, |i| i >= this.teams.list.len()) {
                                this.teams.active = (!this.teams.list.is_empty()).then_some(0);
                            }
                            // Populate the first team from the same fetch (no extra
                            // round trip). This matches the default active selection.
                            if let Some((detail, feed)) = active_detail {
                                if let Some(t) =
                                    this.teams.list.iter_mut().find(|t| t.id == detail.id)
                                {
                                    populate_team_from_detail(t, &detail, &feed.items);
                                }
                            }
                            // If the active team isn't the one we prefetched (e.g.
                            // a non-default selection survived), fetch it normally.
                            if this.teams.active.is_some()
                                && this.active_team().is_some_and(|t| !t.loaded)
                            {
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
        self.teams.signing_out = true;
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
                clear_teams_cache();
                clear_seen();
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.busy = false;
                    this.teams.signing_out = false;
                    this.teams.signed_in = false;
                    this.teams.listed = false;
                    this.teams.seen.clear();
                    this.teams.seen_loaded = false;
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
        let start_dir = self.export.destination.clone();
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

                // Event-driven: the upload sends a fraction per chunk; this
                // listener repaints on each and ends when the sender drops at
                // upload completion (no flag polling, no fixed-rate timer).
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::unbounded_channel::<f32>();
                let progress_listener = this.clone();
                cx.spawn(move |cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        while let Some(p) = progress_rx.recv().await {
                            let _ = progress_listener.update(&mut cx, |this, cx| {
                                this.teams.share_progress = p;
                                cx.notify();
                            });
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
                                if total > 0 {
                                    let _ = progress_tx.send(sent as f32 / total as f32);
                                }
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
                        // Revalidate the active team's feed on the same cadence,
                        // so clips removed server-side (e.g. expired past their
                        // retention window) drop out while the user sits on the
                        // tab — not just on the next reload/tab switch.
                        let _ = this.update(&mut cx, |this, cx| {
                            this.refresh_active_feed(cx);
                        });
                    }

                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(30))
                        .await;
                }
            }
        })
        .detach();
    }

    /// Toggle an emoji reaction on a clip. Updates the tallies optimistically,
    /// calls `set_reaction` on a background thread, then replaces the tallies
    /// with the server's authoritative list (and surfaces any error).
    pub fn toggle_clip_reaction(
        &mut self,
        team_id: String,
        clip_id: String,
        emoji: String,
        cx: &mut Context<Self>,
    ) {
        self.teams.reaction_picker = None;
        let Some(team) = self.teams.list.iter_mut().find(|t| t.id == team_id) else {
            return;
        };
        let Some(clip) = team.clips.iter_mut().find(|c| c.id == clip_id) else {
            return;
        };
        // Optimistic toggle of this emoji's tally.
        let desired = !clip.reactions.iter().any(|t| t.emoji == emoji && t.mine);
        apply_reaction_toggle(&mut clip.reactions, &emoji, desired);
        cx.notify();

        let auth = self.app_state.cloud_auth.clone();
        let clip_id_bg = clip_id.clone();
        let emoji_bg = emoji.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::set_reaction(&auth, &clip_id_bg, &emoji_bg, desired)
                            .map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    let mut pending_err = None;
                    if let Some(team) = this.teams.list.iter_mut().find(|t| t.id == team_id) {
                        if let Some(clip) = team.clips.iter_mut().find(|c| c.id == clip_id) {
                            match result {
                                // Replace with the server's authoritative tallies.
                                Ok(state) => {
                                    clip.reactions =
                                        state.reactions.iter().map(tally_from_dto).collect();
                                }
                                Err(e) => {
                                    // Revert the optimistic toggle.
                                    apply_reaction_toggle(&mut clip.reactions, &emoji, !desired);
                                    pending_err = Some(e);
                                }
                            }
                        }
                    }
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
                // The members and the clip feed are independent endpoints, so
                // fire both concurrently — each blocks on its own background
                // thread and the page waits on the slower one, not their sum.
                let detail_task = {
                    let auth = auth.clone();
                    let team_id = team_id.clone();
                    cx.background_executor().spawn(async move {
                        api::get_team(&auth, &team_id).map_err(|e| e.to_string())
                    })
                };
                let feed_task = {
                    let auth = auth.clone();
                    let team_id = team_id.clone();
                    cx.background_executor().spawn(async move {
                        api::get_feed(&auth, &team_id).map_err(|e| e.to_string())
                    })
                };
                let (detail, feed) = (detail_task.await, feed_task.await);

                let _ = this.update(&mut cx, |this, cx| {
                    match (detail, feed) {
                        (Ok(detail), Ok(feed)) => {
                            if let Some(t) = this.teams.list.iter_mut().find(|t| t.id == team_id) {
                                populate_team_from_detail(t, &detail, &feed.items);
                            }
                        }
                        (Err(e), _) | (_, Err(e)) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    /// Revalidate *only* the active team's clip feed (no members round trip)
    /// and replace its clips with the server's authoritative list. Used by the
    /// tab-(re)open path and the presence-heartbeat poll so clips the backend
    /// has removed — e.g. expired past their retention window — stop showing
    /// locally instead of lingering until a full reload.
    ///
    /// Stale-while-revalidate: the current feed stays painted until the fetch
    /// returns, so there's no flicker. Failures are swallowed — a transient
    /// poll error shouldn't flash a banner over a working feed; the next
    /// trigger retries.
    pub fn refresh_active_feed(&mut self, cx: &mut Context<Self>) {
        let Some(team_id) = self.active_team_id() else {
            return;
        };
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let team_id_bg = team_id.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::get_feed(&auth, &team_id_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    if let Ok(feed) = result {
                        if let Some(t) = this.teams.list.iter_mut().find(|t| t.id == team_id) {
                            t.clips = clips_from_feed(&feed.items, &t.members);
                        }
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

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

    // ── Comments ────────────────────────────────────────────────────
    /// Open the comment thread for a clip and fetch it.
    pub(super) fn open_comments(&mut self, clip_id: String, cx: &mut Context<Self>) {
        self.teams.comments_open = Some(clip_id.clone());
        self.teams.comments = Vec::new();
        self.teams.comments_loading = true;
        self.teams.clip_menu = None;
        self.teams.error = None;
        cx.notify();

        let can_moderate = self.active_team().is_some_and(|t| t.i_am_admin());
        let auth = self.app_state.cloud_auth.clone();
        let clip_bg = clip_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::get_comments(&auth, &clip_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.comments_loading = false;
                    // Ignore a stale response if the user closed/switched threads.
                    if this.teams.comments_open.as_deref() != Some(clip_id.as_str()) {
                        return;
                    }
                    match result {
                        Ok(items) => {
                            let count = items.len() as u32;
                            this.teams.comments = items
                                .into_iter()
                                .map(|c| comment_from_dto(c, can_moderate))
                                .collect();
                            this.set_clip_comment_count(&clip_id, count);
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Post the comment currently in the input box.
    pub(super) fn submit_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(clip_id) = self.teams.comments_open.clone() else { return };
        if self.teams.comment_busy {
            return;
        }
        let body = self.teams.comment_input.read(cx).content.trim().to_string();
        if body.is_empty() {
            return;
        }
        // Clear the box now (we have a window here; the async result handler
        // doesn't), so the user sees their message accepted immediately.
        self.teams.comment_input.update(cx, |s, cx| s.set_value("", window, cx));
        self.teams.comment_busy = true;
        cx.notify();

        let can_moderate = self.active_team().is_some_and(|t| t.i_am_admin());
        let auth = self.app_state.cloud_auth.clone();
        let clip_bg = clip_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::post_comment(&auth, &clip_bg, &body).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.comment_busy = false;
                    match result {
                        Ok(dto) => {
                            if this.teams.comments_open.as_deref() == Some(clip_id.as_str()) {
                                this.teams.comments.push(comment_from_dto(dto, can_moderate));
                            }
                            this.bump_clip_comment_count(&clip_id, 1);
                        }
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Delete a comment from the open thread.
    pub(super) fn delete_comment(&mut self, comment_id: String, cx: &mut Context<Self>) {
        if self.teams.comment_busy {
            return;
        }
        self.teams.comment_busy = true;
        cx.notify();
        let clip_id = self.teams.comments_open.clone();
        let auth = self.app_state.cloud_auth.clone();
        let id_bg = comment_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::delete_comment(&auth, &id_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.comment_busy = false;
                    match result {
                        Ok(()) => {
                            this.teams.comments.retain(|c| c.id != comment_id);
                            if let Some(cid) = clip_id {
                                this.bump_clip_comment_count(&cid, -1);
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

    /// Set a clip's comment count everywhere it appears in the loaded teams.
    fn set_clip_comment_count(&mut self, clip_id: &str, count: u32) {
        for team in self.teams.list.iter_mut() {
            for clip in team.clips.iter_mut() {
                if clip.id == clip_id {
                    clip.comment_count = count;
                }
            }
        }
    }

    /// Adjust a clip's comment count by `delta` (saturating at 0).
    fn bump_clip_comment_count(&mut self, clip_id: &str, delta: i32) {
        for team in self.teams.list.iter_mut() {
            for clip in team.clips.iter_mut() {
                if clip.id == clip_id {
                    clip.comment_count =
                        (clip.comment_count as i32 + delta).max(0) as u32;
                }
            }
        }
    }

    // ── Member management panel ─────────────────────────────────────
    /// Open the members panel and, for admins, fetch the team's invite code.
    pub(super) fn open_members_panel(&mut self, cx: &mut Context<Self>) {
        self.teams.members_open = true;
        self.teams.error = None;
        cx.notify();

        let Some(team) = self.active_team() else { return };
        if !team.i_am_admin() {
            return; // members can view the roster but not the invite code
        }
        let team_id = team.id.clone();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::get_invite(&auth, &team_id).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    match result {
                        Ok(code) => {
                            if let Some(t) = this.active_team_mut() {
                                t.invite_code = Some(code);
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

    /// Copy the active team's invite code to the clipboard.
    pub(super) fn copy_invite_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(code) = self.active_team().and_then(|t| t.invite_code.clone()) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(code));
            self.show_toast(
                "Invite code copied",
                None::<&str>,
                adabraka_ui::overlays::toast::ToastVariant::Success,
                window,
                cx,
            );
        }
        cx.notify();
    }

    /// Mint a fresh invite code, invalidating the old one.
    pub(super) fn regenerate_invite(&mut self, cx: &mut Context<Self>) {
        if self.teams.member_busy {
            return;
        }
        let Some(team_id) = self.active_team_id() else { return };
        self.teams.member_busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::regenerate_invite(&auth, &team_id).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.member_busy = false;
                    match result {
                        Ok(code) => {
                            if let Some(t) = this.active_team_mut() {
                                t.invite_code = Some(code);
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

    /// Change a member's role (owner only); "OWNER" transfers ownership. Reloads
    /// the team afterward so roles (and our own) reflect the change.
    pub(super) fn change_member_role(
        &mut self,
        user_id: String,
        role: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.member_mutate(cx, move |auth, team_id| {
            api::set_member_role(&auth, &team_id, &user_id, role)
        });
    }

    /// Remove a member from the active team (admin+). Reloads afterward.
    pub(super) fn remove_team_member(&mut self, user_id: String, cx: &mut Context<Self>) {
        self.member_mutate(cx, move |auth, team_id| {
            api::remove_member(&auth, &team_id, &user_id)
        });
    }

    /// Shared body for role/remove ops: run `op` on a background thread against
    /// the active team, then reload the team detail to reflect the change.
    fn member_mutate(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(
                std::sync::Arc<crate::cloud::CloudAuth>,
                String,
            ) -> std::result::Result<(), crate::cloud::CloudAuthError>
            + Send
            + 'static,
    ) {
        if self.teams.member_busy {
            return;
        }
        let Some(team_id) = self.active_team_id() else { return };
        self.teams.member_busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move { op(auth, team_id).map_err(|e| e.to_string()) })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.member_busy = false;
                    match result {
                        Ok(()) => this.load_active_team(cx),
                        Err(e) => this.note_cloud_error(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Leave the active team. On success it drops out of the list.
    pub(super) fn leave_active_team(&mut self, cx: &mut Context<Self>) {
        self.team_remove_op(cx, |auth, team_id| api::leave_team(&auth, &team_id));
    }

    /// Delete the active team (owner only). On success it drops out of the list.
    pub(super) fn delete_active_team(&mut self, cx: &mut Context<Self>) {
        self.team_remove_op(cx, |auth, team_id| api::delete_team(&auth, &team_id));
    }

    /// Shared body for leave/delete: run `op`, then remove the team locally and
    /// reselect a remaining team.
    fn team_remove_op(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(
                std::sync::Arc<crate::cloud::CloudAuth>,
                String,
            ) -> std::result::Result<(), crate::cloud::CloudAuthError>
            + Send
            + 'static,
    ) {
        if self.teams.member_busy {
            return;
        }
        let Some(team_id) = self.active_team_id() else { return };
        self.teams.member_busy = true;
        self.teams.error = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        let team_id_done = team_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move { op(auth, team_id).map_err(|e| e.to_string()) })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    this.teams.member_busy = false;
                    match result {
                        Ok(()) => {
                            this.teams.list.retain(|t| t.id != team_id_done);
                            this.teams.members_open = false;
                            this.teams.member_filter = None;
                            this.teams.game_filter = None;
                            this.teams.active =
                                (!this.teams.list.is_empty()).then_some(0);
                            if this.teams.active.is_some()
                                && this.active_team().is_some_and(|t| !t.loaded)
                            {
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

    /// Mutable access to the active team (mirrors `active_team`).
    fn active_team_mut(&mut self) -> Option<&mut Team> {
        let idx = self.teams.active.or(if self.teams.list.is_empty() {
            None
        } else {
            Some(0)
        })?;
        self.teams.list.get_mut(idx)
    }

    // ── Unread / "seen" tracking ────────────────────────────────────
    /// Load the persisted seen-marks once per session, then baseline any team
    /// we don't yet have a mark for to its current activity — so a freshly seen
    /// team (or first launch) doesn't show every existing clip as unread.
    pub(super) fn sync_seen_marks(&mut self) {
        if !self.teams.seen_loaded {
            self.teams.seen = read_seen();
            self.teams.seen_loaded = true;
        }
        let mut changed = false;
        for team in &self.teams.list {
            if !self.teams.seen.contains_key(&team.id) {
                self.teams.seen.insert(team.id.clone(), team.last_activity_unix);
                changed = true;
            }
        }
        if changed {
            write_seen(&self.teams.seen);
        }
    }

    /// Mark a team's feed as seen up to now, clearing its unread badge.
    pub fn mark_team_seen(&mut self, team_id: &str) {
        let now = chrono::Utc::now().timestamp();
        let entry = self.teams.seen.entry(team_id.to_string()).or_insert(0);
        if *entry < now {
            *entry = now;
            write_seen(&self.teams.seen);
        }
    }

    // ── Per-clip actions (the "···" menu) ───────────────────────────
    /// Copy a clip's public watch link to the clipboard.
    pub(super) fn copy_clip_link(
        &mut self,
        clip_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = format!("https://rekaptr.dev/v/{clip_id}");
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(url));
        self.teams.clip_menu = None;
        self.show_toast(
            "Link copied",
            Some("Clip link is on your clipboard."),
            adabraka_ui::overlays::toast::ToastVariant::Success,
            window,
            cx,
        );
        cx.notify();
    }

    /// Download a clip's MP4 to a user-chosen path. Picks the save location via a
    /// native dialog, then streams the file on a background thread.
    pub(super) fn download_clip(
        &mut self,
        url: String,
        suggested_name: String,
        cx: &mut Context<Self>,
    ) {
        self.teams.clip_menu = None;
        cx.notify();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title("Download clip")
                    .set_file_name(&suggested_name)
                    .add_filter("Video", &["mp4"])
                    .save_file()
                    .await
                else {
                    return; // cancelled
                };
                let path = handle.path().to_path_buf();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        // Build the blocking client on this background thread (never
                        // in the async/tokio context — see cloud::auth).
                        let resp = crate::cloud::auth::blocking_client()
                            .get(&url)
                            .send()
                            .map_err(|e| e.to_string())?;
                        if !resp.status().is_success() {
                            return Err(format!("download failed: {}", resp.status()));
                        }
                        let bytes = resp.bytes().map_err(|e| e.to_string())?;
                        std::fs::write(&path, &bytes).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    if let Err(e) = result {
                        this.teams.error = Some(format!("Couldn't download clip: {e}"));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Open the rename modal for a clip, prefilled with its current title.
    pub(super) fn begin_rename_clip(
        &mut self,
        clip_id: String,
        current_title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.teams.clip_menu = None;
        self.teams.rename_target = Some(clip_id);
        self.teams
            .rename_input
            .update(cx, |s, cx| s.set_value(&current_title, window, cx));
        cx.notify();
    }

    /// Confirm the rename: PATCH the clip title, then update it everywhere it
    /// appears in the loaded teams.
    pub(super) fn confirm_rename_clip(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(clip_id) = self.teams.rename_target.clone() else {
            return;
        };
        let title = self.teams.rename_input.read(cx).content.trim().to_string();
        if title.is_empty() {
            self.teams.error = Some("Enter a clip name.".to_string());
            cx.notify();
            return;
        }
        self.teams.rename_target = None;
        self.teams.error = None;
        cx.notify();

        let auth = self.app_state.cloud_auth.clone();
        let clip_id_bg = clip_id.clone();
        let title_bg = title.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::rename_clip(&auth, &clip_id_bg, &title_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            for team in this.teams.list.iter_mut() {
                                for clip in team.clips.iter_mut() {
                                    if clip.id == clip_id {
                                        clip.title = title.clone();
                                    }
                                }
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

    /// Delete a clip from the author's library (removes it from every team).
    pub(super) fn delete_clip_everywhere(&mut self, clip_id: String, cx: &mut Context<Self>) {
        self.teams.clip_menu = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        let clip_id_bg = clip_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::delete_clip(&auth, &clip_id_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            for team in this.teams.list.iter_mut() {
                                team.clips.retain(|c| c.id != clip_id);
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

    /// Remove a clip from a single team's feed (unshare; keeps the media).
    pub(super) fn unshare_clip_here(
        &mut self,
        team_id: String,
        clip_id: String,
        cx: &mut Context<Self>,
    ) {
        self.teams.clip_menu = None;
        cx.notify();
        let auth = self.app_state.cloud_auth.clone();
        let team_id_bg = team_id.clone();
        let clip_id_bg = clip_id.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        api::unshare_clip(&auth, &team_id_bg, &clip_id_bg).map_err(|e| e.to_string())
                    })
                    .await;
                let _ = this.update(&mut cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            if let Some(team) =
                                this.teams.list.iter_mut().find(|t| t.id == team_id)
                            {
                                team.clips.retain(|c| c.id != clip_id);
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

    /// Close the mini player and tear down its mpv instance.
    pub(super) fn close_team_player(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.teams.player.take() {
            window.drop_image(old.render_image()).ok();
        }
        self.teams.player_title = None;
        self.teams.player_scrubbing = false;
        cx.notify();
    }

}
