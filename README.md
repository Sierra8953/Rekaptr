# Rekaptr

A background screen recorder for Windows that keeps the last few minutes of gameplay in a rolling buffer, so you can save the clip you wish you'd been recording — after it happens.

Built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and [GStreamer](https://gstreamer.freedesktop.org/). Hardware-encoded via NVENC.

## Install

One-liner (PowerShell, downloads and runs the latest installer):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Sierra8953/Rekaptr/releases/latest/download/rekaptr-installer.ps1 | iex"
```

Or grab the `.exe` directly from the [latest release](https://github.com/Sierra8953/Rekaptr/releases/latest). A portable `.zip` is also available if you'd rather not install.

Rekaptr ships with its own GStreamer/libmpv runtime — no separate framework install needed.

## Features

- **Rolling buffer** — Configurable retention per source; old footage drops off automatically. Set it to 10 minutes, hit Save Clip after a great play, get the last 10 minutes.
- **Hardware encoding** — NVENC via GStreamer (H.264, HEVC, AV1). Low CPU overhead, low latency, high quality.
- **Per-game settings** — Detects running game processes and applies the encoder, bitrate, resolution, and audio routing you configured for that title.
- **Multi-track audio** — System audio, microphone (with noise gate, compressor, limiter, and RNNoise suppression), and per-app capture. Each track is independently muxed and controllable during playback.
- **Timeline markers** — Drop flag / kill / death / highlight markers via hotkey during gameplay. Markers appear on the timeline for quick navigation.
- **Global hotkeys** — Toggle recording, save clip, mute mic, push-to-talk, drop markers — all remappable.
- **Clip library** — Browse, search, favorite, batch-delete, and export saved clips with a 3-stage export flow (configure, progress, done).
- **Clip export** — Lossless remux or re-encode with selectable encoder, bitrate, container, and per-track audio toggles.
- **In-app updates** — Checks for new releases and installs them in place.
- **System tray** — Runs in the background with tray controls for recording and quick access.
- **Game artwork** — Automatically fetches Steam artwork and logos for detected games.

## Requirements

- Windows 10 or 11 (64-bit)
- An NVIDIA GPU with NVENC support (GeForce GTX 600 series or newer)
- ~200 MB for the install, plus whatever you allocate to the rolling buffer

## Default hotkeys

| Action | Key |
|---|---|
| Toggle recording | `F9` |
| Save clip | `F10` |
| Toggle mic mute | `F11` |

All hotkeys are remappable in Settings → Hotkeys, including push-to-talk and marker keys (flag / kill / death / highlight).

## Building from source

1. Install [Rust](https://rustup.rs/) (stable).
2. Install the [GStreamer 1.24+ MSVC 64-bit](https://gstreamer.freedesktop.org/download/#windows) **Runtime** and **Development** packages.
3. Ensure GStreamer's `bin/` is on your `PATH`.
4. Clone and run:

```powershell
git clone https://github.com/Sierra8953/Rekaptr.git
cd Rekaptr
cargo run --release
```

## Architecture

- **UI** — [GPUI](https://github.com/zed-industries/zed) (Adabraka fork), GPU-accelerated, single-threaded render with async background work.
- **Capture & encode** — GStreamer pipeline with `d3d11screencapturesrc` → NVENC encoder → `splitmuxsink` writing 6-second HLS fragments.
- **Audio** — WASAPI loopback for system audio, per-app capture via process audio sessions, optional RNNoise / DSP chain on the mic path.
- **Playback** — libmpv rendering into a D3D11 texture shared with GPUI via the NV_DX_interop OpenGL extension.
- **HTTP** — A local-only HLS server (port 8080, auto-fallback to 8081–8089) feeds segment playback to libmpv. Token-authenticated; no remote exposure.
- **Storage** — SQLite for config and favorites; recordings live as fragmented MP4 segments on disk for instant seek and lossless clip export.

## License

MIT
