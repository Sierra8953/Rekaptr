# Postmortem: Main-thread freeze / UI lockup

_Last updated: 2026-06-01_

This documents a long-standing intermittent freeze in Rekaptr, how it was
diagnosed, the root cause, and the fixes — so if anything like it shows up again
we don't start from zero.

---

## TL;DR

**Root cause:** the auto-record focus watcher (`src/main.rs`, the
`cx.spawn(...)` near "Auto-Record Event-Driven Logic") used `tokio::select!`
with a **gpui/smol timer future** (`cx.background_executor().timer(...)`) as one
of its branches, running on gpui's **main-thread (foreground) executor**.

A gpui/smol timer future does not cooperate with `tokio::select!`. The select
returned immediately on every poll instead of parking, so the task busy-looped
at **~250,000 iterations/second**, each iteration re-waking the main thread
(`dispatch_on_main_thread` → `PostMessageW`) and re-running
`enumerate_windows()` + `AppConfig::load()`. That pegged the main thread and
starved the Windows message loop (input + paint), which presented as a frozen /
unresponsive window.

**The one-line rule:** _never put a gpui/smol future (timer, channel, etc.) inside
`tokio::select!`, and never run a busy/blocking loop on the main-thread executor._

---

## Symptoms (what it looked like)

- Window froze / became unresponsive; **audio kept playing** (mpv's own threads
  are independent of the UI thread).
- Happened **even when idle, minimized, and not recording** — because the
  auto-record watcher runs unconditionally from startup.
- **Inconsistent timing** and **worse under UI activity** (classic busy-loop /
  contention signature).
- Early on it was a *hard* freeze (couldn't interact at all). After the
  wake-coalescing fix (below) it downgraded to *severe lag* (clicks took up to a
  minute to register) — same root cause, different downstream effect.
- Logs sometimes showed `ERROR_NOT_ENOUGH_QUOTA` (`HRESULT 0x80070718`) from
  `PostMessageW` in `dispatcher.rs`.

### Red herrings we ruled out
- `VSyncProvider::wait_for_vsync() took less time than expected` — **benign.**
  It just means `DwmFlush` returned early (monitor occluded/asleep). It spams
  when you change refresh rate. Not the freeze.
- A "34-minute freeze" in the logs — that was the **PC going to sleep**
  (every thread, including the 1-second watchdog loop, skipped the same ~34 min
  and resumed together). Not an app hang.
- mpv / GStreamer — the call stacks pointed at the gpui scheduler, not these.

---

## Why it escalated the way it did

1. **Hard freeze:** Windows caps a thread's message queue at ~10,000 undelivered
   posted messages. The 250k/s wake flood saturated it → `PostMessageW` failed
   with `ERROR_NOT_ENOUGH_QUOTA`, and once the queue is full Windows can't post
   **input or `WM_PAINT`** either → dead window.
2. **After wake-coalescing:** the queue could no longer saturate, so it became
   severe lag instead — the main thread was still 100% busy running the spin and
   only serviced input occasionally.

---

## The fixes (all currently in the tree)

### 1. The real fix — don't busy-loop the main-thread executor
`src/main.rs`, auto-record watcher. Replaced the `tokio::select!`-over-a-gpui-timer
with a gpui-native poll: `await` a 250ms gpui timer per iteration, `try_recv()`
the focus-event channel each tick, with a ~60s fallback scan. Reacts to focus
changes within ~250ms and **cannot spin** (always yields ≥250ms). Dispatch rate
from this task dropped from ~250,000/s to ~4/s.

### 2. Wake coalescing in the dispatcher (defense-in-depth)
`crates/gpui/src/platform/windows/dispatcher.rs` + `platform.rs` + `events.rs`.
`dispatch_on_main_thread` previously posted **one `PostMessageW` per runnable**.
The handler drains the *entire* channel per message, so only one pending wake is
ever needed. Added `MAIN_THREAD_WAKE_PENDING`: post only on the `false→true`
transition; the drain sites clear it before draining. This makes it impossible
for *any* future flood of main-thread dispatches to saturate the message queue
and hard-freeze the window. Worth keeping regardless of the root cause.

### 3. mpv audio-mix de-churn (perf, unrelated but real)
`src/ui/mod.rs` `update_mpv_audio_mix`. It used to re-apply an identical
`lavfi-complex` filter graph to mpv ~20×/second during playback, forcing mpv to
tear down and rebuild its audio filter chain (a blocking call). Now it caches the
last-applied graph (`last_audio_mix_sig`) and skips the mpv call when unchanged;
the cache is invalidated whenever the video source changes.

---

## How it was diagnosed (repeat this if it recurs)

The decisive tool was a temporary debug module (`src/debug_trace.rs`, since
removed) plus a few one-off instrumentation points. If you need to do this again,
the playbook that worked:

1. **Capture everything.** Install a global `log` logger at `TRACE` that writes
   to a dedicated file, flushing every line (a freeze is diagnosed by killing the
   process, so unflushed buffers are lost). Route GStreamer via `GST_DEBUG` /
   `GST_DEBUG_FILE`. Filter out high-frequency third-party `TRACE` spam
   (`wasapi`, `notify`).
2. **Main-thread stall detector.** A background OS thread that watches a tick
   bumped only from the main thread, and screams when it goes stale. **Important
   gotcha:** the async "heartbeat" task in `main.rs` is *not* a reliable
   main-thread signal — gpui can resume it off-thread; it stayed fresh during a
   real freeze. Use a tick bumped from a place that provably runs on the main
   thread (e.g. the UI poll loop's `entity.update` closure or `Render::render`).
   **Also:** ignore process suspension (system sleep/hibernate *and* debugger
   "Break All") by comparing the detector's own sleep duration / wall-clock jump,
   or it cries wolf.
3. **Capture a live dump.** Task Manager → Details → right-click the process →
   **Create dump file** (works with zero setup). Open in Visual Studio
   ("Debug with Native Only" → Parallel Stacks / Call Stack) or
   `cdb -z dump.dmp -c "~*k; q"`. The dev build has debug info, so stacks
   symbolize.
4. **Read the main-thread stack.** It showed the main thread spinning in the gpui
   scheduler (`run_foreground_task` → `Runnable::run` → … → `schedule` →
   `PostMessageW`) — i.e. a `cx.spawn` task re-waking itself every poll, **not**
   blocked on a lock/syscall (CPU-bound, ~100% on the main thread).
5. **Identify the exact task.** gpui's `ForegroundExecutor::spawn` is
   `#[track_caller]`, so the task carries its `cx.spawn(...)` source location.
   Recording that location on each dispatch and logging the spin rate printed:
   `dispatch rate = 254967/s — hottest spawn site: src\main.rs:793`. That named
   the offending task directly.

### Diagnostic signatures cheat-sheet
- **Busy-loop / spin:** main thread at ~100% CPU; stack in the scheduler
  (`schedule`/`PostMessageW`) rather than a syscall; `PostMessageW` quota errors.
  Find it with per-spawn-location dispatch-rate logging.
- **Blocking deadlock:** main thread at ~0% CPU, parked in a lock/`WaitFor…`/FFI
  call. Need an **all-threads** dump to see which other thread holds the lock.
- **System sleep / debugger pause:** *every* thread stalls the same multi-minute
  span and resumes together; the 1s watchdog loop also skips it. Not a bug.

---

## Rules to avoid a repeat

- **Never mix executor ecosystems in a `select!`.** Don't poll gpui/smol futures
  inside `tokio::select!` (or vice-versa). Use `futures::select!` with fused
  futures, or stick to one executor's primitives. Better: don't `select!` a timer
  against a channel on the main thread at all — poll on a fixed cadence.
- **Never run a busy or blocking loop on the main-thread (foreground) executor.**
  Anything that loops must `await` a real yield point (a timer/event) every
  iteration. Heavy work (window enumeration, `AppConfig::load()` disk I/O,
  ffmpeg/gst `set_state`, libmpv FFI) should run on the background executor or a
  dedicated thread, hopping to the main thread only to touch the UI.
- **Keep wake coalescing.** It's the safety net that turns "flood the message
  queue → dead window" into "merely slow," which is far easier to diagnose.
