# Rekaptr
A background screen recorder for Windows that uses a rolling buffer. It continuously records to temporary segments and lets you save a clip of the recent past instantly.

Built using GPUI and GStreamer.

## Features
- **Rolling buffer** — Configurable buffer length that automatically clears old footage.
- **Hardware encoding** — Uses NVENC, QuickSync, or AMD AMF via GStreamer for low CPU overhead.
- **Game detection** — Detects game processes to start/stop sessions and apply per-game settings.
- **Audio routing** — Captures system audio, specific app windows, and microphone input with optional RNNoise suppression.
- **Clip management** — Integrated library to browse and manage saved recordings.
- **Global hotkeys** — System-wide shortcuts to save clips or toggle the UI.

## Requirements
- Windows 10/11 (64-bit)
- NVIDIA, AMD, or Intel GPU with hardware encoder support.
- [GStreamer 1.24+ MSVC 64-bit](https://gstreamer.freedesktop.org/download/#windows) (Both **Runtime** and **Development** packages are required for building).

## Building from source

1. Install [Rust](https://rustup.rs/).
2. Install the GStreamer MSVC 64-bit **Runtime** and **Development** installers.
3. Ensure GStreamer is in your PATH.
4. Run:
```powershell
git clone https://github.com/Sierra8953/rekaptr.git
cd rekaptr
cargo run --release
```

## Technical Details
- **UI:** GPU-accelerated via GPUI.
- **Pipeline:** GStreamer for capture and encoding.
- **Audio:** WASAPI for system and loopback capture.
- **Storage:** SQLite for session metadata and configuration.

## License
MIT
