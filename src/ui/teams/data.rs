//! Teams data layer: cloud fetch/reconcile and actions (sign-in/out, reload,
//! load/share/react, presence heartbeat) plus the API->UI mapping helpers.

use super::*;
use crate::cloud::api;
use crate::ui::RekaptrWorkspace;

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
    pub(super) fn close_team_player(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.teams.player.take() {
            window.drop_image(old.render_image()).ok();
        }
        self.teams.player_title = None;
        self.teams.player_scrubbing = false;
        cx.notify();
    }

}
