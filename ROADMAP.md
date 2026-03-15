# Luma 0.1.0 Release Roadmap

## P0 — Must ship

### Installer & distribution
- [ ] Create an NSIS or WiX installer that bundles `luma.exe`, `bin/ffmpeg.exe`, GStreamer runtime DLLs, and `assets/`
- [ ] Sign the executable and installer (or at minimum suppress SmartScreen with a known publisher name)
- [x] Add a `--version` flag and embed the version string in the binary (`env!("CARGO_PKG_VERSION")`)

### Crash resilience
- [x] Audit remaining `unwrap()` / `expect()` calls — replace with graceful fallbacks or user-visible error toasts
- [x] Catch panics on the recording thread so a GStreamer failure doesn't take down the whole app
- [x] Handle the case where `bin/ffmpeg.exe` is missing at startup with a clear error instead of silent failures during export

### Recording stability
- [x] Port the EOS segment fixup (`fixup_eos_segments`) into the main stop-recording path (currently proven in test binary)
- [x] Validate that stop → immediate restart doesn't lose the first segment (race between pipeline teardown and new pipeline start)
- [x] Handle GPU driver crash / device-lost mid-recording (currently the pipeline hangs)

### Local server port conflict
- [x] Make the HLS server port configurable or auto-select a free port instead of hardcoding `127.0.0.1:8080`
- [x] Fail gracefully if port 8080 is already in use (another Luma instance, dev server, etc.)

---

## P1 — Should ship

### Testing
- [ ] Add unit tests for config load/save round-trip, segment filename parsing edge cases, and storage path resolution
- [ ] Add an integration test that records 5 seconds, stops, and verifies the output segments are playable
- [ ] Set up a GitHub Actions CI workflow that runs `cargo check`, `cargo test`, and `cargo clippy`

### Export polish
- [x] Replace simulated export progress with real progress (parse ffmpeg stderr for `time=` updates)
- [x] Show actual export errors to the user instead of silently failing

### Storage & cleanup
- [ ] Surface disk usage per game in the library view so users know what's eating space
- [ ] Warn the user when available disk space drops below a threshold during recording
- [ ] Respect `storage_path` everywhere — audit all paths that still derive from `current_exe()` parent

### First-run experience
- [ ] Detect available GPU encoders on first launch and auto-select the best default (NVENC AV1 → NVENC HEVC → NVENC H.264)
- [ ] Show a setup wizard or toast if no NVIDIA GPU is detected (current defaults assume NVENC)
- [ ] Create the storage directory if it doesn't exist instead of relying on the user

---

## P2 — Nice to have

### Quality of life
- [x] System tray icon with recording status indicator
- [x] Global hotkey for instant clip save (last N seconds → mp4, no UI interaction needed)
- [x] Notification toast (Windows native) when a clip finishes exporting
- [x] Remember window position and size across launches

### Performance
- [x] Profile memory usage during long recording sessions (4+ hours) — check for unbounded growth in segment lists or playlist caches
- [x] Benchmark startup time and lazy-load anything that isn't needed immediately

### AMD / Intel support
- [ ] Add AMF encoder support for AMD GPUs
- [ ] Add QSV encoder support for Intel Arc / integrated graphics
- [ ] Auto-detect GPU vendor and expose only relevant encoder options in settings

### Update system
- [ ] Add a lightweight update checker (GitHub Releases API) that shows a non-intrusive banner when a new version is available

---

## Out of scope for 0.1.0
- Linux / macOS support
- Cloud upload or sharing
- Multi-monitor recording
- Webcam overlay
- Streaming (RTMP/SRT output)
