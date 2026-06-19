# Rekaptr Codebase Review

*Reviewed 2026-06-09 against `main` @ `640f7f4` (v0.1.4). Release build verified clean (`cargo build --release`, 4m 45s, only warnings from the vendored `adabraka-gpui` fork).*

## Verdict

A genuinely well-engineered app for what it is — the architecture fits the problem, the hard parts (cross-session timestamps, crash recovery, stop-path consistency) show real thought, and the comments are unusually good. The weaknesses are mostly in the "last 10%" category: a few real bugs, some UI-thread blocking, and one god object.

## What it does right

- **The core architecture is the right one.** Rolling buffer as fragmented MP4 segments on disk means crash-safe recordings, cheap retention (just delete files), instant stream-copy export, and no giant RAM buffer. Serving segments to libmpv over a local HLS server instead of building a custom player pipeline was a pragmatic call.
- **The cross-session timestamp logic** (`src/utils.rs:303`) is the hardest problem in the app and it's handled carefully: max-tfdt across all sessions rather than the latest, a filename-duration-sum floor so a missing ffprobe degrades instead of corrupting the timeline, and comments explaining *why* each choice was made (including why the playlist deliberately omits `#EXT-X-DISCONTINUITY`).
- **Diagnosability is unusually good.** Flush-per-line logging so a Task Manager kill loses nothing, a panic hook that captures background-thread panics with backtraces, a watchdog thread plus UI heartbeat to localize freezes, and `diagnose_pipeline_failure` dumping factory availability when GStreamer fails.
- **The recording state machine is disciplined.** `Starting/Recording/Stopping` phases, the `teardown_in_progress` guard against restarting before splitmuxsink flushes, and every stop path (normal, emergency, tray) resets phase *and* tray state — with comments explaining the bug that motivated it.
- **The local HLS server takes security seriously** for a localhost service: random per-launch token, canonicalize-then-prefix-check against path traversal, 127.0.0.1 bind only.
- **Config is forward/backward compatible** (`serde(default)` everywhere, round-trip tests, legacy JSON→SQLite and exe-dir→LOCALAPPDATA migrations), and the encoder auto-fallback chain means a config from an NVIDIA machine won't brick the app on other hardware.
- **Performance instincts are right**: config cache to stop SQLite-per-frame, precomputed `path_str` to avoid per-frame allocs, mimalloc, GPU scheduling priority boost, below-normal-priority cleanup thread that's idle until a file event fires, storage scans on the background executor.

## What it does wrong

### Real bugs

1. **The export progress bar is fake.** `src/ui/export.rs:181-194` animates 1→100% on a fixed timer (5s re-encode, 0.5s copy) regardless of what ffmpeg is doing. A long re-encode sits at 100% looking hung. ffmpeg's `-progress pipe:1` gives real numbers. The "Done" screen compounds it by showing the *estimated* size (`estimated_size_mb`) instead of reading the actual file size.
2. **The GOP setting is dead.** Settings, add-source, and dashboard all read/write `gop_size`, but `generate_pipeline_string` ignores it — `src/engine.rs:292` and `:316` hardcode `gop = v.fps * 2`. The user's knob does nothing.
3. **The cleanup fallback never cleans.** If the file watcher fails to create, `src/utils.rs:926-929` logs "falling back to polling" then loops on `sleep` forever doing nothing. Retention and the global GB cap silently stop being enforced — unbounded disk growth.
4. **Disk-space toast spam.** The monitor loop (`src/main.rs:618-675`) fires the "Disk Space Low" toast every 10 seconds, unconditionally, the whole time the drive is under 5 GB. Needs a once-per-crossing latch.
5. **The mic-mute hotkey likely doesn't mute the recording.** `src/main.rs:741-757` flips `workspace.form_audio_tracks[].enabled` — that's the add-source *form* state. The live pipeline's mic flows through `MicProvider` subscribers and a `volume` element, neither of which is touched, so mid-recording the hotkey shows a "Mic muted" toast while the mic keeps being recorded. Push-to-talk is similarly a stub (acknowledged in a comment).
6. **Suffix byte-ranges are mishandled** in the HLS server: for `Range: bytes=-500` (last 500 bytes), `src/main.rs:1046-1057` leaves `start=0` and sets `end=500` — it serves the *first* 501 bytes as a 206. The server also assumes the whole request arrives in one 8 KB `read()`.

### Design / code-health issues

7. **`RekaptrWorkspace` is a ~150-field god object** (`src/ui/mod.rs:26-183`) holding every view's form state, export config, wizard steps, and settings forms in one struct. GPUI supports child entities; views owning their own state would cut most of this file's 62 KB.
8. **Recording start blocks the UI thread.** `start_recording` runs on the GPUI main thread and synchronously shells out to ffprobe (once per session directory in `compute_total_duration`), then does `gst::parse::launch` + `set_state(Playing)`. With several sessions on a slow disk that's a visible freeze on the F9 press. The emergency stop also parks the main thread up to 500 ms waiting for EOS (`src/ui/recording.rs:20-26`).
9. **Everything polls.** Tray loop every 100 ms, hotkeys every 50 ms, auto-record every 250 ms, heartbeat every 500 ms — five timers spinning forever. The `tokio::select!` busy-spin incident explains the caution, but for a background app that's meaningful idle wakeup load; foreground-executor channel wakeups would do this for free.
10. **`clean_title` is destructive and collision-prone.** Fine for directory names, but `src/ui/export.rs:147` runs the user's clip title through it — "Clutch 1v4 on Mirage" becomes `clutch1v4onmirage.mp4`. Sanitizing only invalid filename chars would preserve the title. Two distinct titles can also silently merge into one buffer directory.
11. **Instant-copy exports can start broken.** With `-c:v copy`, ffmpeg's cut starts at a packet that may not be a keyframe, so the first moments of the clip can be frozen/garbled until the next IDR. With the 2-second GOP it's bounded, but snapping the in-point to the previous keyframe would make it exact.
12. **Small stuff:** `evict_caches` claims to evict "oldest" entries but takes arbitrary DashMap iteration order, and never touches `logo_cache` (`src/state.rs:278-285`); the fuzzy process match in `start_app_capture` (`target_name.contains(name_no_ext)`, `src/engine.rs:465-475`) can grab the wrong process for short names; `src/ui/mod.rs_snippet.txt` is committed to the repo; tests cover the easy parsers but not the genuinely tricky code (clip-mark math, playlist generation, cleanup retention).

## Suggested priority

1. **#3 cleanup fallback** and **#5 mic mute** — user-facing trust issues (silent disk growth; "muted" mic that isn't).
2. **#1 fake export progress** — the most visible polish gap.
3. **#2 dead GOP setting** — a five-minute fix.
