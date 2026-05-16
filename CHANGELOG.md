# Changelog

## v0.1.1 — Stability (unreleased)

Theme: fix every bug that causes crashes, data loss, or silent failures.

### Fixed

- **Video player no longer triggers COM use-after-free.** The transmuted `ID3D11Device` is now wrapped in `ManuallyDrop`, so every clip preview no longer silently decrements AppState's only D3D11 device reference. Eliminates intermittent crashes inside `d3d11.dll` when opening previews rapidly or under GPU load. (`src/video_player/video.rs`)

- **Recording start failures now surface instead of pretending to record.** `set_state(Playing)` is checked; on failure the bus is drained for the real error, the pipeline and mic subscribers are torn down, phase returns to `Idle`, and a descriptive toast is shown. Previously the UI would show a running timer with zero data being captured. (`src/ui/recording.rs`)

- **Stop/start race that could begin a new recording on top of finalizing files.** Added `RecordingState::teardown_in_progress`; the background teardown thread holds the flag until `fixup_eos_segments` completes. `toggle_recording_ext` refuses to start while the flag is set and shows a "Please wait" toast. (`src/state.rs`, `src/ui/recording.rs`)

- **Emergency stop no longer corrupts the last segment.** When the GStreamer bus reports a fatal error, the pipeline now gets EOS first with a 500ms bus wait before forced `set_state(Null)`, giving `splitmuxsink` a chance to finalize `mfra`/`mdat` boxes. (`src/ui/recording.rs`)

- **HLS server falls back when port 8080 is taken.** Probes 8080–8089, then an OS-assigned ephemeral port. The chosen port is stored in a global `AtomicU16`; URL construction and the CORS `Access-Control-Allow-Origin` header read it via `get_hls_port()`. Previously a port collision silently broke all video playback. (`src/main.rs`, `src/ui/mod.rs`)

- **Mic provider subscriber leak.** `RecordingState::mic_subscriber_keys` tracks every subscriber inserted for a session; `clear_mic_subscribers()` runs on stop, on emergency teardown, on pipeline-launch failure, and on `set_state(Playing)` failure. Eliminates CPU/memory growth across many record/stop cycles. (`src/state.rs`, `src/ui/recording.rs`)

- **Config no longer hits SQLite ~60 times per second.** Added an in-memory cache (`OnceLock<RwLock<Option<AppConfig>>>`). `AppConfig::load()` clones from the cache instead of opening a connection on every render frame; `AppConfig::save()` writes through to both disk and cache. Disk I/O isolated to private `load_from_disk` / `write_to_disk` helpers. Noticeably reduces UI jank on slow storage. (`src/config.rs`)

- **`static mut OPENGL32_DLL` replaced with `OnceLock`.** Removed an unsynchronized static mut that two `Video::new_with_options` callers could race on. New `OpenGl32Module` Send/Sync newtype around `HMODULE`; reads go through `opengl32_handle()`. (`src/video_player/video.rs`)
