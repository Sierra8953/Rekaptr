# Rekaptr Development Guide

## Build & Test Commands
- `cargo check` — fast compilation check, run after every code change
- `cargo test` — run all unit tests, run after every completed feature or fix
- `cargo clippy -- -W clippy::all` — lint check, run before commits

## Project Structure
- `src/engine.rs` — GStreamer pipeline generation, encoder validation, audio capture
- `src/ui/recording.rs` — recording lifecycle, bus monitoring, segment handling
- `src/ui/clips.rs` — clip library UI, mini player, clip cards
- `src/ui/mod.rs` — RekaptrWorkspace state, main render, shared methods
- `src/ui/dashboard.rs` — main dashboard with video player timeline
- `src/ui/export.rs` — clip export with ffmpeg
- `src/ui/settings/*.rs` — settings panels (video, audio, storage, hotkeys, general)
- `src/config.rs` — AppConfig, SQLite persistence, favorites
- `src/db.rs` — per-game session database
- `src/utils.rs` — segment file operations, Steam artwork, clip scanning
- `src/video_player/` — mpv integration, D3D11 texture rendering
- `src/audio.rs` — mic provider with optional RNNoise denoising
- `src/hotkeys.rs` — global hotkey registration
- `src/game_detector.rs` — game window detection
- `src/state.rs` — shared app state, recording stats

## Conventions
- UI framework: GPUI (from Zed) with adabraka_ui component library
- State management: Entity-based (GPUI entities with Context)
- Config storage: SQLite (luma.db) with JSON serialization
- Video pipeline: GStreamer string-based pipelines via gst::parse::launch
- Video playback: libmpv with D3D11/OpenGL interop
- Audio: WASAPI (system + app capture), GStreamer appsrc (mic)
- Icons: Lucide icon set via IconSource::Named("icon-name")
