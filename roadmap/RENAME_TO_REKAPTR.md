# Rename Guide: Luma → Rekaptr

This document is a task brief for an AI agent. The desktop app is being renamed from **Luma** to **Rekaptr** to match the companion web product (`rekaptr-web`, formerly `luma-clips`). The web side is already renamed.

## Scope

Rename every user-facing and identity reference to "Luma" in this repo. Handle migration for existing users so their config, recordings, and startup entry survive the upgrade.

## DO NOT RENAME

These are **false positives** — the word "luma" appears in video-processing contexts unrelated to the product name:

- Anything under `gst_local_docs/` — GStreamer docs use "luma" as the luminance channel (Y in YUV).
- The GStreamer element `lumaxpro` (in `gst_local_docs/opengl_gleffects_lumaxpro.md`).
- Any code comments or identifiers referring to `luma` as a video/color term (e.g. luma keying, luma samples).
- Directories `crates/`, `deps/`, `target/`, `reference/` — third-party or build artifacts.

When in doubt: if the context is GPU/video signal processing, leave it. If it names the app or a user-visible artifact, rename it.

## Files to Update

### 1. Package identity (required)

- `Cargo.toml` — `name = "luma"` → `name = "rekaptr"`. This changes the output binary name from `luma.exe` to `rekaptr.exe`.
- `README.md` — full rewrite or at minimum replace title, description, and the `git clone` URL (`luma-gpui` → `rekaptr`). Note the current README is outdated; flag this for human review rather than silently rewriting.

### 2. Source code — user-facing strings

Update these literal strings in `src/`:

- `src/main.rs:104` — `"[Main] Starting Luma..."` log message.
- `src/main.rs:161` — tray menu `"Open Luma"`.
- `src/main.rs:178` — tray tooltip `"Luma Recording"`.
- `src/main.rs:188` — panic message `"System tray is required for Luma to run"`.
- `src/audio.rs:28` — thread name `"Luma Mic Provider"`.
- `src/hotkeys.rs:88` — thread name `"Luma Hotkeys"`.
- `src/utils.rs:726` — thread name `"Luma Cleanup"`.
- `src/utils.rs:670` — HTTP `user_agent("Luma/1.0")` → `"Rekaptr/1.0"`.
- `src/ui/export.rs:61` — `"Place ffmpeg.exe in the bin/ folder next to Luma..."`.
- `src/ui/settings/about.rs:29` — title `"Luma Replay"`. Decide with the human whether the new name is `"Rekaptr"` alone or `"Rekaptr Replay"`.
- `src/ui/settings/about.rs:55` — copyright `"© 2024 Luma Research & Development"`. Update year and company name per the human's instructions.
- `src/ui/setup_wizard.rs` — four strings at lines 80, 110, 162, 308, 421. All user-visible onboarding copy.
- `examples/memory_stress_test.rs:6`, `examples/synthetic_soak_test.rs:21` — console output headers.
- `tests/audio_test/src/main.rs:34`, `tests/mic_test/src/main.rs:8,17`, `tests/segment_smoothness.rs:3`, `tests/segment_test/src/main.rs:3` — log lines and comments. Non-critical but should be consistent.

### 3. Persistent identity — requires migration logic, not just rename

These are **breaking changes** for existing installs. Do not naively rename; implement a migration path.

#### `%APPDATA%\Luma\` directory (`src/config.rs:158`, `src/utils.rs:18`)

- Current: recordings and config live under `%LOCALAPPDATA%\Luma\Recordings` and similar.
- New: `%LOCALAPPDATA%\Rekaptr\`.
- **Migration**: on first launch after upgrade, if `%LOCALAPPDATA%\Luma\` exists and `%LOCALAPPDATA%\Rekaptr\` does not, move the directory. Use an atomic rename where possible; fall back to copy+delete. Log the migration. Never silently drop user data.

#### `luma.db` SQLite file (`src/config.rs:317-318`)

- Current filename: `luma.db`.
- New: `rekaptr.db`.
- **Migration**: at the same time as the AppData move, rename the file. If an old `luma.db` exists in the legacy location but not the new one, move it. Do **not** create an empty `rekaptr.db` if the old one exists — users lose history.

#### Windows startup registry entry (`src/utils.rs:7`)

- Constant: `STARTUP_REG_VALUE = "Luma"` → `"Rekaptr"`.
- **Migration**: on startup, detect and delete the legacy `Luma` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` if present, then write the new `Rekaptr` value (if auto-start is enabled). Otherwise users will have both entries or lose auto-start.

#### Self-detection in game detector (`src/game_detector.rs:65`)

- Blocklist entry: `"luma.exe"` → `"rekaptr.exe"` (matches new binary name from Cargo.toml change).
- Keep `"luma.exe"` in the list as well for a release or two, so any lingering old installs don't accidentally get detected as "games."

### 4. Internal protocol header (`src/main.rs:598-602`, `src/video_player/video.rs:253`)

- Custom HTTP auth header `X-Luma-Token`.
- Since this is internal (localhost HLS server ↔ embedded player), rename freely to `X-Rekaptr-Token`. Both endpoints ship in the same binary — no cross-version compatibility concern.

### 5. Documentation and planning files

These are internal docs. Update names but do not rewrite content beyond the rename:

- `CLAUDE.md` — title and references.
- `ROADMAP.md` and everything under `roadmap/*.md` — titles and in-text references to "Luma". Leave version numbers, technical content, and file paths that include `luma.db` alone (code handles those).
- `FEATURE_BRAINSTORMING.md`, `FEATURE_DRAFT_SOUNDBOARD.md`, `LUMA_10_OUT_OF_10_PLAN.md` — rename references. Consider renaming `LUMA_10_OUT_OF_10_PLAN.md` → `REKAPTR_10_OUT_OF_10_PLAN.md`.

### 6. GitHub / repo metadata

- Suggest to the human: rename the GitHub repo `Luma-gpui-claude` → `rekaptr` (or whatever they prefer). Update the README `git clone` URL accordingly.
- Search for any hardcoded GitHub URLs — there's at least one at `roadmap/v0.9.0-ecosystem.md:20` (`Sierra8953/Luma-v2`).

## Order of Operations (recommended)

1. Update `Cargo.toml` package name. Run `cargo check` to confirm nothing explicit references the crate name.
2. Update all user-facing strings in `src/` (section 2). Run `cargo build` — verify no warnings, no broken format strings.
3. Implement migration logic for AppData dir, `luma.db`, and registry startup value (section 3). Unit-test the migration on a test directory.
4. Update internal header name (section 4). Confirm player + server still talk to each other.
5. Update docs (section 5).
6. Manually run the app on a machine with an existing Luma install. Verify: recordings still show up, hotkeys still work, auto-start still triggers, no orphaned `Luma` folder or registry entry.
7. Commit as a single "chore: rename Luma to Rekaptr" commit or split into (a) rename + (b) migration if the diff is large.

## Acceptance Criteria

- `rg -i '\bluma\b' src/` returns no results outside of migration/legacy-handling code.
- A fresh install creates `%LOCALAPPDATA%\Rekaptr\` and `rekaptr.db`, never `Luma`.
- An upgrade from a Luma install preserves all user recordings, config, and the auto-start setting.
- Binary is named `rekaptr.exe`. Tray icon says "Rekaptr." About screen says "Rekaptr."
- No compiled-in references to the string "Luma" remain except inside the one-time migration routine.

## Questions to Ask the Human Before Starting

1. Is the legal entity name on the About screen changing too (`Luma Research & Development`)? If yes, to what?
2. Should the About screen read `Rekaptr` or `Rekaptr Replay` (currently `Luma Replay`)?
3. Do they want the legacy `Luma` AppData folder deleted after migration, or preserved as a backup?
4. Is the GitHub repo being renamed? If so, what's the new name and URL?
