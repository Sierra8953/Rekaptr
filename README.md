# Rekaptr

A background screen recorder for Windows that keeps the last few minutes of gameplay in a rolling buffer, so you can save the clip you wish you'd been recording — after it happens.

Built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and [GStreamer](https://gstreamer.freedesktop.org/). Hardware-encoded via NVENC / QuickSync / AMF.

## Install

One-liner (PowerShell, downloads and runs the latest installer):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Sierra8953/Rekaptr/releases/latest/download/rekaptr-installer.ps1 | iex"
```

Or grab the `.exe` directly from the [latest release](https://github.com/Sierra8953/Rekaptr/releases/latest). A portable `.zip` is also available if you'd rather not install.

Rekaptr ships with its own GStreamer/libmpv runtime — no separate framework install needed.

## Features

- **Rolling buffer** — Configurable retention; old footage drops off automatically. Set it to 10 minutes, hit Save Clip after a great play, get the last 10 minutes.
- **Hardware encoding** — NVENC, QuickSync, or AMF via GStreamer. Low CPU overhead, low latency, high quality.
- **Per-game settings** — Detects running game processes and applies the encoder, bitrate, and audio routing you configured for that title.
- **Six audio tracks** — System audio, microphone (with optional noise gate/compressor/limiter), and per-app capture. Each track is independently muxed.
- **Global hotkeys** — Toggle recording, save clip, mute mic, drop markers — all from in-game.
- **Clip library** — Browse, favorite, and export saved sessions without leaving the app.
- **In-app updates** — Checks for new releases and installs them in place.

## Requirements

- Windows 10 or 11 (64-bit)
- A GPU with a working hardware encoder — basically any NVIDIA card from the last ~8 years, recent AMD/Intel also supported via GStreamer's `*enc` elements.
- ~200 MB for the install, plus whatever you allocate to the rolling buffer.

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

To produce installer artifacts, see [`RELEASING.md`](RELEASING.md).

## Architecture

- **UI** — [GPUI](https://github.com/zed-industries/zed) (Adabraka fork), GPU-accelerated, single-threaded render with async background work.
- **Capture & encode** — GStreamer pipeline with `d3d11screencapturesrc` → encoder → `splitmuxsink` writing HLS fragments.
- **Audio** — WASAPI loopback for system, per-app capture via process audio sessions, optional RNNoise / DSP chain on the mic path.
- **Playback** — libmpv rendering into a D3D11 texture shared with GPUI via the NV_DX_interop OpenGL extension.
- **HTTP** — A local-only HLS server on port 8080 (auto-falls-back to 8081–8089 or an ephemeral port) feeds clip previews to libmpv. Token-authenticated; no remote exposure.
- **Storage** — SQLite for config and session metadata; recordings live as HLS segments on disk for instant seek and lossless clip export.

## Status

Pre-release. v0.1.0 is the first installable build; v0.1.1 (the current `main`) is a stability pass that fixes a handful of crashes, races, and silent failures — see [`CHANGELOG.md`](CHANGELOG.md).

## License

MIT
