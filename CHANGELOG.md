# Changelog

## v0.1.4

### Fixed

- Fixed an intermittent freeze / UI lockup where the window became unresponsive (even while idle or minimized) while audio kept playing. The auto-record focus watcher was busy-looping the main thread and starving the Windows message loop.
- Fixed a related hard freeze where a flood of UI wake-ups could saturate the Windows message queue and kill input and repainting entirely. Wake-ups are now coalesced.
- Fixed a crash when playing back older clips after adding a new audio track (e.g. a mic): the player no longer tries to mix audio tracks that don't exist in the recorded segment.

### Optimization

- Stopped re-applying an identical audio mix filter to the player ~20 times per second during playback, which forced a blocking teardown and rebuild of mpv's audio filter chain each time. The filter is now cached and only re-applied when it actually changes.
- Made the Clips library noticeably lighter to render, especially with large clip collections. Clip grouping and the sidebar game/favorite counts no longer rescan the whole library quadratically on every redraw, and clip file paths are no longer re-allocated for every clip on every frame.

### Cleanup

- Removed ~500 lines of dead code across the codebase: unused functions (`clear_all_buffers`, `fetch_all_sessions`, `get_exact_duration`, `generate_ffconcat_playlist`, `resolve_pid`, `format_hotkey`, `section_header`, `refresh_storage_info`), unused structs/fields (`SessionInfo`, `ClipsViewMode::Table`, `SettingsTab::ALL`, `export_crf`, `hovered_clip_idx`, `TimelineMarker.label`), and stub methods from a prior GStreamer video backend (`buffer_capacity`, `buffered_len`, `set_frame_buffer_capacity`, `load_file`, `set_display_size`).
- Deleted orphaned files: `video_player/window_helper.rs` (dead module, never imported) and `ui/sessions.rs` (never declared as a module).
- Stripped 9 unused GStreamer-era error variants from the video player error enum, removing unnecessary `glib`/`gstreamer` dependencies from that path.
- Removed unused `VideoOptions` fields (`frame_buffer_capacity`, `looping`, `speed`) that were accepted but silently ignored.
- Cleaned up blanket `#[allow(dead_code)]` annotations across `video_player` and `RekaptrWorkspace`.

## v0.1.3

### Fixed

- Fixed audio track app selection not showing which apps were already assigned to a track.
- Fixed being unable to remove an app from an audio track once it was added.
- Fixed assigned apps disappearing from the audio selection screen when their window was closed.
- Fixed audio routing not displaying all configured tracks when editing an existing source.
- Fixed the app allowing multiple copies to run at the same time; launching it again now brings the existing window to the front.
- Fixed installer dropping install paths that contain spaces (e.g. `C:\Program Files\Rekaptr`), which caused in-app updates to land in the wrong directory.

### New Features

- Redesigned the clip export dialog into a 3-stage Configure → Exporting → Done flow: editable clip title, mode cards, encoder/quality/container options, per-track audio toggles, destination picker, and a success screen.
- Added an app-audio picker to the add-source flow.
- Made the database path configurable, resolved through `AppConfig::get_db_path` (which creates the parent directory) instead of a hardcoded filename.
- Restructured the settings panels and removed the Updates card from the Startup tab.
- Replaced the app icon with a new design.

## v0.1.1 — Stability

### Fixed

- Random crashes when opening clip previews (COM refcount bug in video player).
- Recording silently capturing nothing when the encoder fails — now shows an error.
- Last segment corrupting on emergency stop / pipeline crash.
- Race when stopping and immediately restarting a recording.
- Video playback silently broken when port 8080 is taken — now falls back to 8081–8089.
- Microphone subscriber leak causing CPU/memory growth across record cycles.
- UI jank from config being re-read from SQLite ~60 times per second.
