# Luma v0.1.0 Release Roadmap

Comprehensive audit of the codebase as of 2026-03-17. This document covers every issue, missing feature, and improvement needed for a stable 0.1.0 release.

**Current state:** v0.1.0-stable reached. Core recording pipeline works (GStreamer + NVENC), UI is polished with standardized cards and theme tokens, background disk monitoring and system tray integration are complete.

---

## P0 — Release Blockers

These must be fixed before any public release.

### ~~1. Hardcoded Storage Path~~ DONE
- Added `storage_path: String` to `AppConfig` with `#[serde(default)]`, defaults to `%LOCALAPPDATA%\Luma\Recordings`
- `get_storage_root()` now reads from config
- Settings UI displays the actual config value

### ~~2. Sessions Tab Calls Non-Existent Function~~ DONE
- Added `SessionInfo` struct to `state.rs`
- Implemented `fetch_all_sessions()` in `utils.rs` — scans storage root for game folders with `.m4s` segments
- Fixed `file_name().unwrap()` in `sessions.rs`

### ~~3. 39 `.unwrap()` Calls Across 12 Files~~ DONE
- Reduced from 39 to 2 (remaining 2 are `CString::new` on static string literals — infallible)
- Replaced with `unwrap_or_default()`, `unwrap_or()`, `if let`/`match`, `?`, `.ok()`, or error-with-return as appropriate
- All 12 files fixed: `config.rs`, `utils.rs`, `main.rs`, `engine.rs`, `audio.rs`, `db.rs`, `state.rs`, `virtual_audio_router.rs`, `ui/mod.rs`, `ui/clips.rs`, `ui/sessions.rs`, `video_player/video.rs`

### ~~4. Unsafe Mutable Static in Win32 Hook~~ DONE
- Replaced `static mut HOOK_TX` with `static HOOK_TX: std::sync::OnceLock<...>`, eliminating the data race

### ~~5. Add Source Form Doesn't Save~~ DONE (was already implemented)
- `add_source.rs:504-505` already calls `config.game_registry.insert()` and `config.save()`
- Verified working — no changes needed

### ~~6. Clip Export Not Functional~~ DONE (was already implemented, hardened)
- Export handler was already complete (`perform_export` with FFmpeg invocation, thumbnail generation, toast notifications)
- Added ffmpeg existence check with user-facing error toast before attempting export
- Fixed `get_ffmpeg_path()` to search exe dir, cwd, then fall back to PATH

---

## P1 — Critical Quality Issues

Should be fixed for 0.1.0 to avoid a bad first impression.

### ~~7. Inconsistent Logging~~ DONE
- All `println!`/`eprintln!` replaced with `log::` calls across all files
- Fixed: `main.rs`, `ui/mod.rs`, `ui/dashboard.rs`, `ui/add_source.rs`, `utils.rs`, `config.rs`

### ~~8. No Error Recovery in Audio Pipeline~~ DONE
- Replaced `bus.iter_timed(gst::ClockTime::NONE)` with 1-second timed poll loop
- Pipeline state is checked each iteration; exits cleanly on Null/Error/EOS

### ~~9. Local HLS Server Has No Security~~ DONE
- Added random token (`OnceLock<String>`) generated at startup via `get_hls_token()`
- Server validates `?token=` query param on every request, returns 403 if missing/wrong
- CORS restricted from `*` to `http://127.0.0.1:8080`
- UI URLs updated to include token

### ~~10. Buffer/Cache Memory Leaks~~ DONE
- Added `CACHE_MAX_ENTRIES = 100` constant and `evict_caches()` method on `AppState`
- Eviction runs on each game detector tick (every focus change / 60s fallback)
- Covers `artwork_cache`, `playlist_cache`, `process_cache`

### ~~11. Segment Duration Parsing Falls Back to ffprobe~~ DONE
- Added `get_ffprobe_path()` with same search strategy as `get_ffmpeg_path()` (exe dir → cwd → PATH)
- Added `DURATION_CACHE` (`OnceLock<Mutex<HashMap>>`) to cache ffprobe results, eliminating N+1 spawns
- Logs warning when ffprobe is not found instead of silently failing

### ~~12. Cleanup Thread Can Crash~~ DONE
- Replaced `.unwrap()` on watcher creation with `match` that logs error and falls back
- Also replaced `.unwrap()` on thread spawn with `.ok()`

---

## P2 — Missing Core Features

Features users will expect from a v0.1.0 recording app.

### ~~13. Configurable Storage Location~~ DONE
- Added "Change" button with `rfd::AsyncFileDialog` folder picker next to Storage Path in Settings > General
- Path persisted to `AppConfig.storage_path`, storage directory auto-created on change

### ~~14. Audio Device Selection~~ DONE
- Added `enumerate_audio_devices()` in `engine.rs` — queries WASAPI `DeviceCollection` for input/output devices
- Cached device lists in `AppState` (`audio_output_devices`, `audio_input_devices`), populated at startup
- Audio track config in source settings now shows device picker buttons for System and Mic tracks

### ~~15. Hotkey Support~~ DONE
- Added `HotkeyConfig` struct to `config.rs` with configurable VK codes and modifiers
- Created `hotkeys.rs` module: dedicated thread with Win32 `RegisterHotKey` + message loop
- Default hotkeys: F9 (toggle recording), F10 (save clip), F11 (toggle mic mute)
- Hotkey actions dispatched to UI via `mpsc::channel`, polled from async task in `main.rs`
- Hotkey display card added to Settings > General showing current bindings

### ~~16. Thumbnail Generation for Clips~~ DONE (was already implemented)
- Export handler (`perform_export` in `ui/mod.rs`) already extracts a thumbnail frame via FFmpeg at the clip midpoint
- Saved as `.jpg` alongside the exported `.mp4` — clips UI checks for and displays these thumbnails

### ~~17. GPU/Encoder Auto-Detection~~ DONE
- Added `validate_and_fix_encoder()` to `AppConfig` — probes GStreamer element factories at startup
- Fallback priority: NVENC H.264 → NVENC H.265 → NVENC AV1 → x264
- Called at startup in `main.rs` before UI initialization
- Settings > General now displays the active encoder with friendly label

### ~~18. Recording Performance Overlay~~ DONE
- Added `RecordingStats` struct to `state.rs` with atomic/mutex fields for live metrics
- Stats updated from GStreamer bus: bitrate (from segment file size/duration), disk write rate, dropped frames (from QoS messages), segment count
- Semi-transparent overlay rendered in top-right of video during recording showing: REC indicator + elapsed time, bitrate (kbps/Mbps), disk write rate (MB/s), dropped frame count (orange if >0), segment count
- Stats reset on each new recording via `reset_rec_stats()`

### ~~19. Startup with Windows~~ DONE
- Added `set_startup_with_windows()` and `is_startup_with_windows()` in `utils.rs` — manages registry key at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- Toggle button in Settings > General with description, persisted to `AppConfig.startup_with_windows`
- Uses `winreg` crate (already a dependency)

### ~~20. Sessions Tab Implementation~~ DONE (was already implemented)
- `fetch_all_sessions()` in `utils.rs` scans storage root for game folders with `.m4s` segments
- `sessions.rs` renders session cards with game artwork (from artwork cache), date, segment count, and play button
- Play button loads the session into the dashboard video player via `load_session_path()`

---

## P3 — Code Quality & Robustness

Important for maintainability and preventing regressions.

### ~~21. AppState Refactor~~ DONE
- Split `AppState` into `RecordingState` (pipeline, phase, duration, session blocks, stats) and `ExportState` (phase, progress) sub-structs
- Removed 10 unused fields (`mic_monitor_pipeline`, `clip_state`, `manual_window_handles`, `audio_device_map`, `last_seek_time`, `current_session_id`, `pending_seek`, `was_playing_before_scrub`, `playlist_cache`, `process_cache`)
- Removed 3 dead structs (`Toast`, `ToastManager`, `ClipState`)
- AppState reduced from 24 unorganized fields to 11 fields + 2 logical sub-structs

### ~~22. State Machine for Recording Lifecycle~~ DONE
- Added `RecordingPhase` enum: `Idle → Starting → Recording → Stopping → Idle`
- Added `ExportPhase` enum: `Idle ↔ Exporting`
- Replaced `AtomicBool` flags with `Arc<Mutex<Phase>>` enums with proper lifecycle transitions
- Error paths (encoder validation, pipeline cast, pipeline launch) correctly revert `Starting → Idle`
- Stopping phase: pipeline teardown thread transitions `Stopping → Idle` after EOS + cleanup

### ~~23. Test Coverage~~ DONE
- Added 17 unit tests across 3 modules (was 1 test with no assertions):
  - `engine::tests`: `parse_res` (standard + fallback), `resolve_pid` (empty input)
  - `utils::tests`: `clean_title` (basic + special chars), `parse_segment_index` (valid, no-duration, invalid), `get_segment_duration` (from filename, missing), `compute_total_duration` (empty, with segments, nonexistent dir)
  - `config::tests`: serialization round-trip, deserialize with missing fields (serde defaults), default config values
- All 21 tests pass (17 unit + 4 integration)

### ~~24. Error Type Consolidation~~ DONE
- Replaced all `.unwrap()` on file path operations in `fixup_eos_segments()` with safe `match`/`unwrap_or_default` patterns
- Converted `map_err(|e| anyhow!(...))` to idiomatic `.context()` in `engine.rs` and `virtual_audio_router.rs`
- No bare panics remain; 2 remaining `unwrap()` calls are on infallible operations (CString on static ASCII)

### ~~25. Game Detection Efficiency~~ DONE
- Replaced `refresh_processes()` with `refresh_processes_specifics(ProcessRefreshKind::new())` — only refreshes process names, skips CPU/memory/disk stats

### ~~26. D3D11 Device Lifetime Safety~~ DONE
- Added `AddRef` via COM `Clone` when storing the D3D11 device handle in `main.rs`, preventing premature release
- Added safety documentation to `boost_device_gpu_priority()` explaining the ManuallyDrop borrow pattern
- Added safety comments at GStreamer context usage (ui/mod.rs) and video player transmute_copy (video.rs) documenting lifetime guarantees

---

## P4 — Polish & UX

Nice-to-haves for a good 0.1.0 experience.

### ~~27. Graceful Shutdown~~ DONE
- Registered `on_app_quit` handler on `LumaWorkspace` via gpui's app lifecycle
- On quit: stops recording pipeline with EOS + 3s timeout + Null state + segment fixup, releases mic provider and clears subscribers, drops virtual audio routers, drops video players
- Daemon threads (cleanup watcher, HLS server, hotkey listener) exit naturally with the process

### ~~28. First-Run Experience~~ DONE
- Added `first_run_completed: bool` to `AppConfig` (defaults `false` via `#[serde(default)]`)
- 4-step setup wizard modal: Welcome → Storage Location (with folder picker via `rfd`) → Encoder Selection (auto-detects available GStreamer encoders) → Summary
- On finish: saves storage path + encoder to config, sets `first_run_completed = true`, creates storage directory, shows success toast

### ~~29. Disk Space Warnings~~ DONE
- Background task in `main.rs` polls free space on recording drive every 10s via `sysinfo`
- Shows "Disk Space Low" toast warning at 5GB
- Auto-stops recording and shows "Disk Full" destructive toast at 1GB to prevent data loss

### 30. Multi-Monitor Awareness
- `d3d11screencapturesrc` captures a single monitor or window
- **Need:** Let user select which monitor to capture in the source picker (list available displays)

### ~~31. Recording Indicator~~ DONE
- System tray icon initialized in `main.rs` via `tray-icon` crate
- Context menu: "Open Luma", "Stop Recording" (dynamic), "Quit"
- "Stop Recording" menu item enabled/disabled based on active recording state

### 32. Segment Continuity Verification
- No verification that segments are contiguous and valid after recording
- **Need:** Post-recording check that all segments decode correctly, warn user of corruption

### 33. Config Migration/Versioning
- No version field in `AppConfig` — adding new fields will break existing configs
- **Need:** Add `config_version: u32`, implement migration logic in `AppConfig::load()`

### 34. Video Player Seek Bounds
- **File:** `src/video_player/` — seeking has no bounds checking
- **Need:** Clamp seek position to `[0, duration]`

### 35. Clip Library Virtual Scrolling
- All clips loaded into memory at once in `ui/clips.rs`
- **Need:** Paginate or virtualize the list for users with hundreds of clips

---

## Feature Inventory: What Works Today

| Feature | Status | Notes |
|---------|--------|-------|
| Screen capture (DXGI/WGC) | Working | Single monitor or window capture |
| NVENC encoding (H.264/H.265/AV1) | Working | Requires NVIDIA GPU |
| x264 software fallback | Working | Ultrafast preset |
| HLS segment recording | Working | 6-second segments, ISO-MP4 |
| Multi-track audio (System) | Working | WASAPI loopback |
| Multi-track audio (Mic) | Working | WASAPI shared mode |
| Multi-track audio (App) | Partial | Per-app capture works, routing to tracks incomplete |
| Instant replay buffer | Working | Retention-based cleanup |
| Game detection | Working | Process name matching with blacklist |
| Auto-record on game launch | Working | Event-driven via Win32 hook |
| Video playback (mpv) | Working | D3D11-accelerated via HLS server |
| Clip in/out markers | Working | UI timeline markers functional |
| Clip export | Working | FFmpeg-based with thumbnail generation |
| Per-game settings | Working | Saved to config on Add Source |
| Sessions history | Working | Scans storage root for game folders |
| Settings persistence | Partial | Global settings save, many fields unused |
| Storage management | Working | Auto-cleanup works, path configurable |
| GPU priority boost | Working | D3DKMT + IDXGIDevice priority |
| Steam artwork lookup | Working | HTTP lookup with caching |

---

## Suggested Milestone Plan

### ~~v0.1.0-alpha (Release Blockers Only)~~ COMPLETE
All P0 items #1-6 resolved. App no longer crashes on a clean Windows install and core features function.

### ~~v0.1.0-beta (Quality + Core Features)~~ COMPLETE
All P1 items #7-12 and P2 items #13-20 resolved. App is usable as a daily driver for recording.

### ~~v0.1.0-rc (Polish)~~ COMPLETE
Fix P3 items #21-24, and P4 items #27-29, #31. App is ready for public release.

### ~~v0.1.0~~ COMPLETE
All P0-P3 resolved, critical P4 items addressed. v0.1.0 stable release reached.
