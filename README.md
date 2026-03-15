# Luma

Luma is a GPU-accelerated screen recording tool for Windows built with [GPUI](https://github.com/zed-industries/zed) and GStreamer. It's designed to provide high-performance capture with minimal CPU overhead by leveraging hardware encoders (NVENC).

## Features

*   **GPU-Accelerated Capture:** Low-latency desktop and application window recording using DXGI/WGC.
*   **Segmented Recording:** Video is stored in small fragments (HLS/ffconcat) to prevent data loss in the event of a crash.
*   **Modern UI:** Built using a custom violet theme on the GPUI framework.
*   **Per-Application Audio:** Capture audio from specific processes without system-wide mixer noise.
*   **Fast Export:** Instant "stream-copy" clipping or full GPU re-encoding via FFmpeg.
*   **Instant Replay:** Save the last N seconds of gameplay with a global hotkey.

## Setup

### Prerequisites

1.  **FFmpeg:** You need `ffmpeg.exe` in the `bin/` directory for clip exporting.
2.  **GStreamer:** Install the GStreamer 1.24+ runtime (MSVC 64-bit) and ensure it's in your PATH.
3.  **NVIDIA GPU:** Currently optimized for NVENC (H.264, HEVC, AV1).

### Building from Source

```powershell
git clone https://github.com/isaac/luma-gpui.git
cd luma-gpui
cargo run
```

*Note: The `bin/` directory in this repository does not include `ffmpeg.exe` or `libmpv-2.dll` due to file size limits. You must provide these binaries manually.*

## Project Structure

*   `src/`: Main application logic and UI views.
*   `crates/gpui/`: Local patched version of the GPUI framework.
*   `assets/`: Icons and UI resources.
*   `ROADMAP.md`: Current development status and planned features.

## License

MIT
