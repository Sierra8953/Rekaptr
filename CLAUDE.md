# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Rekaptr is a Windows-only background screen recorder. It keeps the last few minutes of gameplay in a rolling buffer (continuous recording into capped, fragmented MP4 segments) so the user can save a clip *after* the moment happens. UI is GPUI; capture/encode is GStreamer + NVENC; playback is libmpv into a shared D3D11 texture.

## Build & run

```powershell
cargo run --release     # primary dev loop — release is the realistic target (NVENC, LTO)
cargo build             # debug build, faster compile; fine for non-pipeline UI work
cargo test              # unit tests (notably config round-trip in src/config.rs)
```

Prerequisites (the build links against them — there is no pure-Rust fallback):
- **GStreamer 1.24+ MSVC 64-bit**, both **Runtime** and **Development** packages, with its `bin/` on `PATH`.
- `gstreamer::init()` runs at startup in `main`; missing/old GStreamer fails there.
- At runtime the app shells out to bundled **`ffmpeg.exe` / `ffprobe.exe`** expected next to the exe (see "Cross-session timestamps" below). During `cargo run` these come from the GStreamer/dev environment; the shipped build bundles them in `runtime/`.

`gpui` is a **vendored local fork** (`crates/gpui`, package `adabraka-gpui`) wired in via `[patch.crates-io]`. Editing GPUI itself means editing that crate. UI component library is the external `adabraka-ui` crate.

## Releasing

`pwsh scripts/build-release.ps1` produces the portable zip (`dist build`), the Inno Setup installer (`iscc installer.iss`), and the bootstrap `rekaptr-installer.ps1`. Full procedure — version bump, `runtime/` refresh, GitHub Release tag conventions (`v<version>`, hardcoded in installer URLs) — is in `RELEASING.md`. Version is read from `CARGO_PKG_VERSION`; bumping `Cargo.toml` is the only code change needed.

## Architecture

### Startup and threading model (`src/main.rs`)
`main` is `#[tokio::main]`, but the app body runs inside `gpui::Application::run(move |cx| { ... })`, which executes on the **GPUI main thread**. Everything created there — the `Arc<AppState>`, the system tray, the `RekaptrWorkspace` entity — lives on that thread. `cx.spawn(...)` schedules futures back onto this same foreground (main-thread) executor; background work uses `cx.background_executor()` or raw `std::thread`. This thread-affinity matters: tray (`tray-icon`) and Win32 shell calls must happen on the main thread.

`main` also enforces **single-instance** via a named mutex (`Rekaptr_SingleInstance_Mutex_v1`) plus a shared event; a second launch signals the running instance to surface its window rather than starting over.

Long-lived background loops are each a detached `cx.spawn` polling on a timer:
- **Tray loop** — owns the `TrayIcon`, drains `TrayCommand`s from a channel, and handles tray menu events.
- **Hotkey loop** — drains `HotkeyAction`s from `start_hotkey_listener()` (`src/hotkeys.rs`).
- **Disk-space monitor** — auto-stops recording when the target drive is critically low.

### Shared state (`src/state.rs`)
`AppState` is the single `Arc`-shared hub; `RekaptrWorkspace` (`src/ui/mod.rs`) holds a clone of the same `Arc`. Cross-thread/cross-task communication is via the fields here: `tray_tx` (channel to the tray loop), `DashMap` caches (artwork/logo/game registry), and `parking_lot::Mutex`-guarded device lists. Recording lifecycle is a state machine: `RecordingPhase { Idle, Starting, Recording, Stopping }` behind `Arc<Mutex<_>>`, where **only `Recording` counts as `is_recording()`**. Any code that ends a recording must both reset the phase and notify the tray (`TrayCommand::SetRecording(false)` / `SetStopEnabled(false)`) — there are multiple stop paths (normal stop, emergency/error stop, app shutdown) and they must stay in sync.

### Capture & encode (`src/engine.rs`, `src/ui/recording.rs`)
The pipeline is built as a GStreamer launch string: `d3d11screencapturesrc` → NVENC encoder (H.264/HEVC/AV1) → `splitmuxsink` with `isofmp4mux`, writing fragmented `seg_%010d_*.m4s` segments (~6s each) into a per-title directory under the storage root. `engine.rs` also boosts process GPU scheduling priority (`D3DKMTSetProcessSchedulingPriorityClass`) so encoding doesn't starve the game. Pipeline bus messages are watched in `recording.rs::spawn_bus_monitor` — EOS ends a clean stop; a bus **Error** triggers the emergency-stop path (`toggle_recording_internal`).

### Audio (`src/audio.rs`, `src/mic_dsp.rs`, `src/virtual_audio_router.rs`)
WASAPI loopback for system audio, per-app capture via process audio sessions, and a mic path with an optional DSP chain (noise gate, compressor, limiter, RNNoise). Mic audio flows through a shared `MicProvider` (publish/subscribe; recording subscribes a key and must unsubscribe on stop). Tracks are muxed as independent audio streams so they can be toggled per-track at export time.

### Playback (`src/video_player/`)
libmpv2 renders into a **D3D11 texture shared with GPUI** via the NV_DX_interop OpenGL extension. The shared D3D11 device handle lives in `AppState::d3d11_device`. `video_player/element.rs` is the custom GPUI element that draws the texture; `Video` is the high-level handle held by the workspace.

### Local HLS server (`src/main.rs::start_local_server`)
A local-only HTTP server serves the on-disk fragmented-MP4 segments to libmpv for playback and scrubbing. It binds **port 8080, auto-falling back through 8081–8089** (port stored in `HLS_PORT`), and is **token-authenticated** (`HLS_TOKEN`, random per launch) with no remote exposure. Past path-traversal/auth issues here were security-sensitive — treat segment-path handling carefully.

### Cross-session timestamps (`src/utils.rs`)
Recordings span multiple sessions on one continuous timeline. Each new session's `splitmuxsink` is given a `decode-time-offset` computed by probing the previous segments' end time with **ffprobe** (`ffprobe_segment_end_time`, tfdt-based, with a filename-duration-sum fallback if ffprobe is missing). Get this wrong and successive sessions reset to ~0, overlapping and making playback jump to the start at the session seam. `fixup_eos_segments` renames partially-flushed final fragments out of the playable set after a stop.

### UI (`src/ui/`)
Single root entity `RekaptrWorkspace` with an `active_view` switch (dashboard, clips, settings, etc.). Views are modules under `src/ui/` (`dashboard.rs`, `clips.rs`, `timeline.rs`, `export.rs`, `recording.rs`, `add_source.rs`, `setup_wizard.rs`) and `src/ui/settings/`. GPUI is immediate-mode-ish: render functions rebuild the element tree each frame, so per-frame work in render paths is hot (e.g. `Clip::path_str` is precomputed to avoid re-allocating every frame).

**Before debugging GPUI layout/spacing/positioning, read [`docs/gpui-layout.md`](docs/gpui-layout.md)** — verified flexbox defaults for this fork (`div()` is `display: Block`, the `min-width: auto` shrink gotcha, what the `Styled`/`VStack`/`HStack` helpers actually set) plus taffy traps. For layout bugs, tint suspect containers with `.bg()` and screenshot early rather than reasoning about taffy blind.

## Config & data locations

- App data root: `%LOCALAPPDATA%\Rekaptr\` (config, logs, gst-registry, recordings index) — **not** next to the exe, even for portable builds.
- Recordings: storage root from `AppConfig` (default `%LOCALAPPDATA%\Rekaptr\Recordings`), one subdirectory per `clean_title(source)`.
- Config and favorites: SQLite via `rusqlite` (`src/db.rs`) plus JSON config (`src/config.rs`); `clean_title` sanitizes titles into filesystem-safe directory names.

## Conventions

- **Windows-only.** Direct Win32 calls via the `windows`/`winapi` crates are expected throughout; don't add cross-platform shims.
- Use `parking_lot::Mutex` (not `std::sync::Mutex`) for new locks, matching the codebase. Note its guards are **not** reentrant.
- `release`/`dist` profiles use `panic = "abort"` and heavy LTO — code must not rely on unwinding.
- Logging is via the `log` crate with bracketed subsystem tags, e.g. `log::info!("[Recording] ...")`, `[TrayIcon]`, `[GStreamer]`, `[GPU Priority]`. Follow that prefix style.
