# Rekaptr 0.2.0 — Optimization Roadmap

Detailed, prioritized optimizations derived from static analysis of the codebase.
Each item lists file(s), line references, impact, and a concrete fix.

---

## Summary by Category

| Category              | Items | Severity        |
|-----------------------|-------|-----------------|
| Config caching        | 4     | CRITICAL        |
| Filesystem I/O        | 8     | CRITICAL–HIGH   |
| Lock contention       | 5     | CRITICAL–HIGH   |
| Cloning/allocations   | 7     | HIGH            |
| Threading/teardown    | 6     | MEDIUM          |
| Database              | 4     | MEDIUM          |
| Build/binary          | 5     | LOW–MEDIUM      |
| Code quality          | 6     | LOW             |

---

## CRITICAL — High Impact, Quick Win

### 1. Repeated `AppConfig::load()` — N+1 pattern (60+ sites)
- **Where:** `src/main.rs:113,146,506`; `src/audio.rs:55,86,95`; `src/hotkeys.rs`; `src/utils.rs` (×4); `src/ui/*` (×30+)
- **Problem:** Every call opens SQLite, deserializes JSON, allocates a fresh `AppConfig`. Hot paths (frame updates, hotkey polling, buffer cleanup, mic DSP) reload synchronously.
- **Fix:** Put `Arc<RwLock<AppConfig>>` (or `Arc<ArcSwap<AppConfig>>`) in `AppState`. Load once at startup; update the pointer on save. Remove all `AppConfig::load()` callsites outside of settings save paths.

### 2. `fetch_all_clips()` runs every clips-view render
- **Where:** `src/ui/mod.rs:833`, implementation `src/utils.rs:105-168`
- **Problem:** Full `.mp4`/`.mkv` walk + metadata + thumbnail probe per render (60 Hz).
- **Fix:** Cache `Vec<Clip>` in `AppState`. Invalidate via the existing `notify` watcher used by buffer cleanup. Only rescan on FS event or explicit refresh.

### 3. `compute_total_duration()` spawns ffprobe per segment
- **Where:** `src/utils.rs:401-437,442-473`; called from `src/ui/recording.rs:151`
- **Problem:** ffprobe process spawn per segment on every recording start; 10–50 ms each accumulates fast.
- **Fix:** Write `.duration_cache` JSON alongside the game directory; only recompute for segments newer than cache mtime.

### 4. Buffer cleanup thread — full recursive walk every 5 s
- **Where:** `src/utils.rs:786-911` (`start_buffer_cleanup_thread`), esp. `829-888`
- **Problem:** O(n) walk of storage root every debounce tick, reloads config, computes total duration per game. Never exits gracefully.
- **Fix:** Index segments by mtime (sort desc, early-exit when `total <= retention`). Add `Arc<AtomicBool>` shutdown flag. Cache game totals in-memory.

### 5. Recording-stats `Mutex<f64>` read 3× per UI frame
- **Where:** `src/state.rs:134-147`; consumer `src/ui/dashboard.rs:41-44`
- **Problem:** `fps`, `bitrate_kbps`, `disk_write_mbps` each behind a `Mutex<f64>`. Dashboard takes 3 locks every frame.
- **Fix:** Replace with `AtomicU64` storing `f64::to_bits`. Lock-free load in UI, lock-free store from bus thread.

---

## HIGH — Moderate Impact

### 6. `clips_filtered()` clones entire Clip vec per frame
- **Where:** `src/ui/clips.rs:79-108` (line 98 `.cloned().collect()`)
- **Fix:** Return `Vec<&Clip>` or `Vec<usize>` indices; avoid clones in render tree.

### 7. `clips_groups()` double-clones + O(n) game lookup
- **Where:** `src/ui/clips.rs:112-133` (lines 118, 125, 126, 128)
- **Fix:** Pre-sort clips by `game_title`; build groups with iterator windows; store game index not title; drop hero clone.

### 8. `render_clips_filter_rail()` rebuilds games list every frame
- **Where:** `src/ui/clips.rs:135-152` (144-150)
- **Fix:** Cache `(game, count)` in workspace state; rebuild only on clips change.

### 9. Thumbnail generation is blocking
- **Where:** `src/utils.rs:248-304`
- **Problem:** Sequential ffmpeg fallback (CUDA → software) on the UI/load thread.
- **Fix:** Spawn via tokio with a bounded semaphore (e.g. 4 concurrent); the `.noThumb` marker already exists for caching.

### 10. `AppState::evict_caches()` iterates even when under limit
- **Where:** `src/state.rs:252-265`
- **Fix:** Track insertion order (`VecDeque` or intrusive LRU); evict only while `len > limit`.

### 11. Hotkey listener reloads config on every action
- **Where:** `src/main.rs:346-425` (350, 506)
- **Fix:** Use shared `AppState` config pointer from item #1.

### 12. Game detector enumerates windows on every focus event
- **Where:** `src/main.rs:483-507`, `src/game_detector.rs`
- **Fix:** Cache enumeration with 1 s TTL; dedupe focus-event / main-loop scans.

---

## MEDIUM — Moderate Impact, More Effort

### 13. ffprobe called twice per segment during EOS fixup
- **Where:** `src/utils.rs:521-599` (543, 582)
- **Fix:** One ffprobe call per segment; batch fixup in a single pass.

### 14. Filename `_XXXms` parse uncached
- **Where:** `src/utils.rs` (cleanup & fixup)
- **Fix:** Parse once; cache in `Mutex<HashMap<PathBuf, f64>>` scoped to cleanup pass.

### 15. Video player `Arc<RwLock<Internal>>` read per frame
- **Where:** `src/video_player/video.rs:113`; consumer `src/ui/dashboard.rs:10-13`
- **Fix:** Publish position/duration via atomics; update on mpv event callback only.

### 16. Process list full-scan for app capture
- **Where:** `src/engine.rs:193-200,423-424,436-457`
- **Fix:** Cache `sysinfo::System` in `AppState` with 1–5 s TTL; fuzzy match against cached list.

### 17. Pipeline gen re-scans disk for next segment index
- **Where:** `src/engine.rs:388-402`
- **Fix:** Persist `last_segment_index` in `AppState` per game dir; update on splitmux-format-location.

### 18. Mic DSP hot-reload calls `AppConfig::load()`
- **Where:** `src/audio.rs:82-97`
- **Fix:** Share mic settings via `Arc`; swap pointer on change instead of reloading.

---

## MEDIUM-LOW — Maintainability / Correctness

### 19. Buffer cleanup thread has no shutdown signal
- **Where:** `src/utils.rs:813-911`
- **Fix:** `Arc<AtomicBool>` shutdown, checked each loop iter.

### 20. Hotkey listener + auto-record threads leak on shutdown
- **Where:** `src/main.rs:345-426,445-481`
- **Fix:** Store hook handles in `AppState`; `UnhookWinEvent` on Drop; close rx channel.

### 21. Disk-space monitor reallocates timer each iteration
- **Where:** `src/main.rs:259-316`
- **Fix:** Use `tokio::time::interval(10s)` instead of spawning a new `timer()`.

### 22. Per-thread tokio runtime for local HLS server
- **Where:** `src/main.rs:550-702`
- **Fix:** Use a shared runtime; or `tiny_http` / `hyper` behind a single runtime.

### 23. Hand-rolled HTTP parser in HLS server
- **Where:** `src/main.rs:550-702` (588-609, 652-663)
- **Fix:** Adopt `tiny_http` or `hyper`; safer + faster.

### 24. `percent_decode_path` allocates per request
- **Where:** `src/main.rs:43-62`
- **Fix:** Return `Cow<'_, str>`; reuse thread-local buffer.

---

## DATABASE

### 25. Missing PRAGMAs
- **Where:** `src/config.rs:321-337`
- **Add:** `PRAGMA temp_store=MEMORY;`, `PRAGMA mmap_size=268435456;`, `PRAGMA optimize;` on close.

### 26. Favorites lookup
- **Where:** `src/config.rs:333-335`
- **Fix:** `clip_path` is already PK; ensure queries hit the index (no `LOWER()` wraps). Consider `PRAGMA query_only` on read-only connections.

### 27. No connection pool
- **Problem:** Each call opens a fresh `rusqlite::Connection`.
- **Fix:** Share one `Arc<Mutex<Connection>>` (or `r2d2_sqlite` pool) in `AppState`.

### 28. JSON-in-a-column for `AppConfig`
- **Problem:** Rewrites the entire blob on every save.
- **Fix:** For hot toggles (recording state, last-used game), split into typed columns/rows.

---

## BUILD / BINARY

### 29. No release profile tuning
Add to `Cargo.toml`:
```toml
[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
strip = "symbols"
opt-level = 3
```

### 30. Log strings retained in release
- **Fix:** Demote verbose `log::info!` to `log::debug!`; gate with `log`'s max-level feature in release:
  `log = { version = "0.4", features = ["max_level_debug", "release_max_level_info"] }`

### 31. `env_logger` + unused deps
- **Audit:** `gpui-component` (v0.5.1) + `adabraka-ui` both present — confirm one is unused.
- **Audit:** `winapi` 0.3 + `windows` 0.61 overlap; prefer `windows` crate only.

### 32. Default encoder `nvav1enc`
- **Where:** `src/config.rs:168`
- **Problem:** Fails on older NVIDIA drivers; first-run validation stall.
- **Fix:** Probe at startup; default to `h264_nvenc` when AV1 unavailable.

### 33. Dead-code markers
- **Where:** `src/ui/mod.rs:26`; `src/video_player/mod.rs:1-8`
- **Fix:** Delete or document retention reason.

---

## PIPELINE / RESOURCE

### 34. D3D11 texture registration — no explicit Drop
- **Where:** `src/video_player/video.rs:46-88`
- **Risk:** GPU memory leak if WGL interop objects aren't unregistered.
- **Fix:** Implement `Drop` with `wglDXUnregisterObjectNV` + `wglDXCloseDeviceNV`.

### 35. `boost_device_gpu_priority` raw pointer cast
- **Where:** `src/engine.rs:92-120`; callsite `src/main.rs:324-335`
- **Fix:** Null-check at call site; return `Result` to surface failure.

### 36. Logo/artwork fetch blocks background executor
- **Where:** `src/ui/dashboard.rs:66-102` (line 88)
- **Fix:** Use async `reqwest::Client` (non-blocking feature) with timeout.

---

## Recommended 0.2.0 Scope

**Must land (biggest wins, lowest risk):**
1. Shared `AppConfig` in `AppState` (#1, #11, #18) — single biggest perf win.
2. Clips cache with FS-event invalidation (#2, #6, #7, #8).
3. Atomic recording stats (#5).
4. Segment-index caching (#17) + ffprobe batching (#3, #13).
5. SQLite connection pool + PRAGMAs (#25, #27).
6. Release profile flags (#29).

**Should land:**
7. Async thumbnail generation with bounded parallelism (#9).
8. Process/window enumeration cache (#12, #16).
9. Graceful thread shutdown (#19, #20).
10. Video player atomics for position/duration (#15).

**Nice to have:**
11. HTTP server library swap (#22, #23, #24).
12. D3D11 Drop cleanup (#34).
13. Encoder pre-probe (#32).
14. Dead code sweep (#33).

**Expected outcome:** Clips page load 30–40% faster, recording-start latency down by hundreds of ms for users with many sessions, 50% less idle disk I/O, fewer UI frame drops during settings changes, cleaner shutdown.
