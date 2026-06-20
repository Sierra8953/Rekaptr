# Modularization Handoff

Status snapshot for the in-progress modularization refactor, so a fresh session
(or a future you) can pick it up without re-deriving context.

**Last updated:** 2026-06-20 (all phases complete)
**Branch:** `refactor/modularize` (branched from `main`; `main` is untouched)
**Build/test status:** `cargo check` clean (only the 3 pre-existing baseline
warnings); `cargo test` → 19 passed. App has been **manually verified working**
by the user (dashboard, clips, settings, teams, record→clip→export all exercised).

---

## TL;DR — where we are

We are executing a multi-phase plan to modularize the Rekaptr codebase. The
headline problem — a 158-field "god object" (`RekaptrWorkspace`) and a
1,456-line `utils.rs` grab-bag — is **largely solved**:

- `utils.rs` → a `utils/` directory of 7 focused submodules (Phase 1 ✅)
- `RekaptrWorkspace`: **158 fields → 34** via 9 per-feature state structs (Phase 3 ✅)
- export/recording *logic* moved into a new `core/` layer (Phase 2 ✅)
- the big render files split into directories: `ui/teams/`, `ui/dashboard/`,
  plus `ui/shared.rs` (Phase 4 ✅)
- architecture documented in `CLAUDE.md`, `//!` docs on every new module (Phase 5 ✅)

**Remaining:** nothing — all phases complete. Pending a final visual pass over
the Dashboard/Teams tabs, the branch is ready to merge to `main`.

---

## The plan & phase status

Recommended execution order was **0 → 1 → 3 → 2 → 4 → 5**.

| Phase | What | Status |
|-------|------|--------|
| 0 | Safety-net branch + clean baseline | ✅ done (`2bf9396`) |
| 1 | Split `utils.rs` into focused submodules | ✅ done (`92dc992`) |
| 3 | Group god-object fields into per-feature structs | ✅ done (9 commits) |
| 5 (partial) | Document new architecture in `CLAUDE.md` | ✅ done (`a835bcf`, `core/` added) |
| **2** | **Move export/recording logic into `core/`** | ✅ done (`ae96f5c`, `771c7db`) |
| **4** | **Split the largest render files** | ✅ done (`2c6b840`, `f6dd767`, `403fb68`) |
| **5 (rest)** | Module-doc comments, final tidy | ✅ done |

### Commits on this branch (newest first)
```
a835bcf docs(claude.md): document grouped UI state structs and utils/ split
e7a56d2 refactor(ui): extract SetupWizardState and StorageState
bf98474 refactor(ui): extract ClipsState
707ea5e refactor(ui): extract AddSourceForm
0c3f21c refactor(ui): extract SourcesState
dbfe677 refactor(ui): extract SettingsForm
0614c94 refactor(ui): extract ClipPreviewState
6693911 refactor(ui): extract MixerState
a9ba0a4 refactor(ui): extract ExportForm
9ea6faa refactor(ui): extract TeamsState
92dc992 refactor(utils): split monolithic utils.rs into focused submodules
2bf9396 checkpoint: pre-modularization baseline
```

> **Heads-up:** the baseline commit `2bf9396` also carries earlier uncommitted
> work that predates this refactor: a new cloud/Teams feature, the in-game
> overlay, and **3 code-review bug fixes** (export `-map 0:a:N?` optional marker;
> `.mov`/`.mkv` clip counting; cloud-auth "stuck signed in" recovery). Those are
> intentional and already verified — don't be surprised they're in the diff
> against `main`.

---

## Phase 1 detail — `utils/` layout (done)

`src/utils.rs` is now `src/utils/` with a thin re-export façade so every existing
`crate::utils::*` call site still resolves unchanged (zero behavior change):

| Submodule | Responsibility |
|-----------|----------------|
| `paths.rs` | storage root, `clean_title`, bundled-binary lookup, dir size, `game_dir_for` |
| `startup.rs` | "start with Windows" Run-key toggle |
| `segments.rs` | fMP4 segment filename parsing + ffprobe duration/end-time + dedup scan (shared `pub(crate)` primitives) |
| `playlist.rs` | HLS master/session playlists, `compute_total_duration`, `fixup_eos_segments`, clip marks + concat lists |
| `steam_assets.rs` | appid resolution + icon/cover/artwork fetch (`appinfo.vdf` parse) |
| `clips_index.rs` | recorded-source discovery, saved-clip library, `SourceStats`/`source_stats` |
| `retention.rs` | rolling-buffer retention + global GB cap (watcher + polling fallback) |

`utils/mod.rs` declares the submodules privately and `pub use`s the public API +
the moved unit tests.

Also unblocked the test suite while here: removed an orphaned `engine.rs` test
for the already-deleted `resolve_pid` fn, and corrected a stale
`compute_total_duration` test whose expectation predated the filename-duration
fallback (now expects `15.0`, not `0.0`).

---

## Phase 3 detail — grouped state structs (done)

Each per-feature struct **lives in the module that renders it**, has a `new(..)`
constructor called from the `RekaptrWorkspace` initializer, and is accessed as
`self.<group>.<field>`.

| Struct | Defined in | Workspace field | ~Fields |
|--------|-----------|-----------------|---------|
| `TeamsState` | `ui/teams.rs` | `teams` | 16 |
| `ExportForm` | `ui/export.rs` | `export` | 14 |
| `MixerState` | `ui/dashboard.rs` | `mixer` | 6 |
| `ClipPreviewState` | `ui/clips.rs` | `clip_preview` | 8 |
| `SettingsForm` | `ui/settings/mod.rs` | `settings` | 36 |
| `SourcesState` | `ui/dashboard.rs` | `sources` | 5 |
| `AddSourceForm` | `ui/add_source.rs` | `add_source` | 28 |
| `ClipsState` | `ui/clips.rs` | `clips` | 12 |
| `SetupWizardState` | `ui/setup_wizard.rs` | `setup` | 5 |
| `StorageState` | `ui/settings/storage.rs` (re-exported from `settings/mod.rs`) | `storage` | 4 |

### Naming notes / gotchas discovered
- `ExportForm` is **not** named `ExportState` — that name is taken by
  `crate::state::ExportState` (the shared cross-thread export *phase/progress* on
  `AppState`). Keep these distinct.
- Prefixes were dropped *inside* the structs: `settings_form_mic_gain` →
  `self.settings.mic_gain`; `preview_video_source` → `self.clip_preview.player`;
  the Teams `Vec` field `teams` → `self.teams.list`.
- `overlay.rs` has its **own** `export_*` and `is_scrubbing` fields on a
  different struct — those were deliberately **left untouched**. When renaming,
  scope edits to the right files.
- `StorageState` is in the private `settings::storage` submodule, so it needs
  `pub use storage::StorageState;` in `settings/mod.rs` to be reachable as
  `crate::ui::settings::StorageState`.

### Fields deliberately LEFT on `RekaptrWorkspace` (the remaining 34)
These are genuinely cross-cutting workspace identity/state, not feature-local, so
forcing them into a bucket would be wrong:
`active_view`, `settings_tab`, `app_state`, `video_source`, `selected_source`,
`session_to_delete`, `clip_start`/`clip_end`/`clip_start_mark`/`clip_end_mark`
(export inputs used by recording+export+playback), `is_scrubbing`/
`scrubbing_progress`/`preview_bar_bounds` (dashboard preview scrub),
`toast_manager`, `last_notified_position`, `is_refreshing_windows`,
`is_loading_video`, `last_volume_update_at`, `recording_start_time`/
`recording_session_id`, `hotkey_listening`/`hotkey_focus_handle`,
`update_state`/`update_has_receipt`, `_quit_subscription`, plus the 9 grouped
sub-struct fields above.

---

## The extraction pattern (reuse this for any future slice)

This worked cleanly 9 times in a row. Per slice:

1. **Measure first.** Find every accessor and which files touch it:
   ```bash
   grep -rhoE 'self\.(field_a|field_b|...)' src --include=*.rs | sort | uniq -c
   grep -rlE '(self|this|workspace)\.(field_a|...)' src --include=*.rs
   ```
   Watch for: name collisions with other structs (e.g. `overlay.rs`), method
   names that share the prefix, and the container name itself.
2. **Define the struct** in the module that renders the feature, with a
   `new(cx)` (or `new(config, cx)`) constructor. Move any non-trivial field
   initializers (closures, widget entities, `DataTable`) into `new`.
3. **Edit `ui/mod.rs`:** replace the flat field block with one
   `pub <group>: <Struct>,` field, and replace the initializer lines with one
   `<group>: <Module>::<Struct>::new(...),`.
4. **Mechanically rename accessors** with `perl -i -pe` substitutions, ordered
   **longest-field-name-first** to avoid prefix shadowing, scoped to only the
   files that legitimately hold the field:
   ```bash
   perl -i -pe 's/\b(self|this)\.old_field\b/$1.group.new_field/g;' file1 file2
   ```
5. **Check for leftovers**, then compile, test, commit:
   ```bash
   grep -rnE '(self|this)\.(old_field|...)\b' src --include=*.rs   # expect empty
   cargo check --message-format=short 2>&1 | grep -E '^error'
   cargo test 2>&1 | grep -E 'test result|FAILED'
   ```

### Pitfall the perl approach hits
A field accessed across a **line break** (`self\n   .field`) is missed by the
line-based regex. Happened once (`self\n.cached_clips`). After the perl pass, if
the compiler reports `no field X`, look for a multi-line method-chain access and
fix it by hand.

---

## What's left to do

### Phase 2 — move export/recording *logic* into `core/` ✅ DONE
**Goal:** separate "what the app does" from "what it shows", mirroring how the
already-clean `cloud/` module is layered.

- `core/export.rs` ✅ (`ae96f5c`): `run_export(ExportParams, AppState)` — the
  ffmpeg command building + run/progress/thumbnail/concat-cleanup, moved verbatim
  out of `RekaptrWorkspace::perform_export`. The UI freezes inputs into
  `ExportParams` and calls it from the same `background_spawn`; progress still
  publishes to `AppState::export.progress`, result flows back as the return value.
  UI keeps mark validation, toasts, output-path derivation, result/state handling.
- `core/recording.rs` ✅ (`771c7db`): the **AppState-only** halves —
  `clear_mic_subscribers`, `emergency_stop` (the `toggle_recording_internal`
  body), `begin_graceful_teardown` (the `stop_recording` teardown thread +
  no-pipeline fallback), and `notify_recording_state` (tray + overlay sync,
  consolidated from three byte-identical sites). **The pipeline *launch* in
  `start_recording` was deliberately left in `ui/recording.rs`** — it's entangled
  with per-step toasts and `spawn_bus_monitor` (needs `cx`), so threading it
  through a `Result` would be a refactor, not a verbatim move. A future pass could
  extract it by returning a launch result and letting the UI render the toasts.

Both verified: `cargo check` clean (no new warnings), `cargo test` → 19 passed.
**Still owed: user re-verify** record start/stop, emergency/error stop, and
export end-to-end (export was confirmed after `ae96f5c`).

### Phase 4 — split the largest render files ✅ DONE
Pure file moves now that state is grouped:
- `ui/mod.rs` ✅ (`2c6b840`): shared free helpers/enums (`prettify_process_name`,
  `track_color`, `toggle_switch`, `audio_track_*`, `ActiveView`/`SettingsTab` +
  impl, `SETTINGS_NAV`) → `ui/shared.rs`, re-exported `pub use shared::*`. mod.rs
  is now the workspace shell + `Render` impl (1,309 → ~1,110 lines).
- `ui/teams.rs` ✅ (`f6dd767`): 2,129-line file → `teams/{mod,data,view}.rs`
  (types/`TeamsState` + `active_team`; fetch/reconcile + actions; render). The
  two methods crossing the data/view boundary (`confirm_teams_panel`,
  `close_team_player`) are `pub(super)`.
- `ui/dashboard.rs` ✅ (`403fb68`): 1,646-line file → `dashboard/{mod,preview,
  mixer,sources}.rs`. The pane renderers called from `render_dashboard` are
  `pub(super)`.

**Pattern used for the directory splits:** `git rm` the file, slice exact
line-ranges with `sed -n 'A,Bp'` into the new files (wrapping moved methods in a
fresh `impl RekaptrWorkspace {…}`), each submodule starts with `use super::*;`
(which transitively provides the parent's glob imports — the compiler will flag
any now-redundant explicit `use gpui::*`/prelude to delete). Watch the **true
file length** (`git show HEAD:file | wc -l`) — an off-by-N at EOF truncates the
last fn (hit once on teams/view.rs).

### Phase 5 — finish docs ✅ DONE
- `//!` module-docs added to every new split file; `CLAUDE.md` UI + Core-logic
  sections updated to describe `core/`, `ui/shared.rs`, and the `dashboard/` and
  `teams/` directories.

---

## Refactor complete

All phases (0–5) are done. `cargo check` is clean (only the pre-existing baseline
warnings: adabraka-gpui `PathBuf`/`BOOL`, `ui/mod.rs` `DropdownState`/`gst`/
`DataTable`, `utils` `generate_master_playlist`, teams `AVATAR_TINTS`/
`team_initials` — none introduced by the refactor), `cargo test` → 19 passed.
Export and recording were re-verified by the user after the Phase 2 commits.

**Still owed before merge:** a final visual pass over the Dashboard and Teams
tabs (Phase 4 was pure file moves, compiler-checked, but the user verifies UI
visually). After that the branch is ready to merge to `main`.

### Optional future cleanups (out of scope, not blocking)
- Extract `start_recording`'s pipeline *launch* into `core/recording.rs` by
  returning a launch result and letting the UI render the per-step toasts (left
  in `ui/recording.rs` because it needs `cx`/`window` for `spawn_bus_monitor`
  and toasts).
- Delete the dead `AVATAR_TINTS` const and `team_initials` fn (pre-existing).

---

## How to resume

```bash
git checkout refactor/modularize
cargo check && cargo test           # baseline: clean + 19 passed
```

Then start Phase 2 (`core/export.rs` first). One slice = one commit; build +
test between each; commit messages follow the existing
`refactor(scope): …` / `Phase N (step M)` style and end with the
`Co-Authored-By: Claude Opus 4.8` trailer.

When the whole plan is green and re-verified, the branch is ready to merge to
`main` (squash or keep the granular history — each commit is independently
green).

## Conventions reminder (from `CLAUDE.md`)
- Windows-only; `parking_lot::Mutex` (non-reentrant) for new locks.
- `release`/`dist` use `panic = "abort"` — no reliance on unwinding.
- Logging via `log` with bracketed subsystem tags, e.g. `log::info!("[Export] …")`.
- This is a `#[tokio::main]` app: never build/drop `reqwest::blocking` in the
  async context — cloud HTTP runs per-call on `cx.background_executor()`.
- `gpui` is a vendored fork (`crates/gpui`, pkg `adabraka-gpui`); UI lib is
  external `adabraka-ui`.
