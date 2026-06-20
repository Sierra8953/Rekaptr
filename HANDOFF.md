# Modularization Handoff

Status snapshot for the in-progress modularization refactor, so a fresh session
(or a future you) can pick it up without re-deriving context.

**Last updated:** 2026-06-20
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
- Architecture documented in `CLAUDE.md` (Phase 5 partial ✅)

**Remaining:** Phase 2 (move export/recording *logic* into `core/`), Phase 4
(split the big render files), Phase 5 (finish docs).

---

## The plan & phase status

Recommended execution order was **0 → 1 → 3 → 2 → 4 → 5**.

| Phase | What | Status |
|-------|------|--------|
| 0 | Safety-net branch + clean baseline | ✅ done (`2bf9396`) |
| 1 | Split `utils.rs` into focused submodules | ✅ done (`92dc992`) |
| 3 | Group god-object fields into per-feature structs | ✅ done (9 commits) |
| 5 (partial) | Document new architecture in `CLAUDE.md` | ✅ done (`a835bcf`) |
| **2** | **Move export/recording logic into `core/`** | ⏳ **not started — do next** |
| **4** | **Split the largest render files** | ⏳ not started |
| **5 (rest)** | Module-doc comments, final tidy | ⏳ not started |

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

### Phase 2 — move export/recording *logic* into a new `core/` (DO NEXT, highest risk)
**Goal:** separate "what the app does" from "what it shows", mirroring how the
already-clean `cloud/` module is layered.

- `core/export.rs`: the ffmpeg command building + spawn/run/progress thread
  currently inside `RekaptrWorkspace::perform_export` (`ui/export.rs`). Make it
  take plain inputs (paths, marks, frozen track→stream mapping, options) and
  report progress/result via `AppState`. The UI's `perform_export` becomes a thin
  wrapper that gathers state and calls it.
- `core/recording.rs`: `toggle_recording_internal`, pipeline start/stop, and the
  tray/overlay sync from `ui/recording.rs`. UI keeps only the button wiring.

**⚠️ Risk:** these are the exact hot paths we just fixed 3 bugs in, and
`cargo test` covers **none** of the ffmpeg/GStreamer behavior. Strategy: move
code *verbatim* first, change only call sites, keep behavior identical — then
**ask the user to re-run and verify** record→clip→export and emergency-stop
before moving on. Do not refactor logic and move it in the same commit.

### Phase 4 — split the largest render files (low risk)
Pure file moves now that state is grouped:
- `ui/teams.rs` (~2,050 lines) → `teams/data.rs` (fetch/reconcile) + `teams/view.rs` (render) + `teams/mod.rs` (types/`TeamsState`).
- `ui/dashboard.rs` (~1,600) → preview / mixer / sources-list render submodules.
- `ui/mod.rs` (~1,300) → move shared free helpers (`prettify_process_name`,
  `track_color`, `toggle_switch`, the `ActiveView`/`SettingsTab` enums) into a
  small `ui/shared.rs`; keep `mod.rs` to the workspace shell + `Render` impl.

### Phase 5 — finish docs (trivial)
- Add a one-line `//!` module-doc to each new module stating its single job
  (most already have one; the Phase-3 structs have struct-level docs).
- Re-check `CLAUDE.md`/`MEMORY.md` for any other stale `utils.rs`/`timeline.rs`
  references.

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
