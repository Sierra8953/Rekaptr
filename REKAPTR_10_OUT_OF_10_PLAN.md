# Rekaptr: The Path to 10/10

This document outlines a comprehensive, phased refactoring plan to elevate Rekaptr from a highly capable, functioning prototype to a production-ready, tier-1 Windows application. The focus is on eradicating technical debt, replacing fragile heuristics with deterministic logic, and ensuring long-term maintainability.

## Phase 1: Core Architecture & Maintainability

### 1.1 Programmatic GStreamer Pipelines
**The Problem:** The current `generate_pipeline_string` function creates massive, stringly-typed pipelines using `gst::parse::launch`. This is virtually impossible to unit test and fails opaquely.
**The Solution:**
*   Rewrite the pipeline generation using GStreamer's programmatic object API (`gst::ElementFactory::make`, `gst::Bin::add`, `gst::Element::link`).
*   **Benefits:** This allows for dynamic property updates (e.g., changing bitrate on the fly), precise error handling when a specific element (like `nvd3d11h264enc`) fails to load, and completely eliminates syntax errors from missing spaces or typos.

### 1.2 State Management & Mutex Sprawl
**The Problem:** `AppState` is a massive struct wrapped in an `Arc`, riddled with `Mutex` and `OnceLock` fields accessed concurrently across UI threads, background tasks, and GStreamer bus callbacks.
**The Solution:**
*   Adopt a more structured state management pattern, such as the Actor model. Create a dedicated "Recording Engine" thread/actor that owns the GStreamer pipeline and hardware handles.
*   The UI should communicate with the engine via message passing (`mpsc` channels) rather than acquiring mutex locks on shared state.
*   **Benefits:** Prevents deadlocks, makes data flow predictable, and decouples the GPUI render loop from media pipeline blocking operations.

### 1.3 Upstream GPUI Patches
**The Problem:** Rekaptr relies on a local, patched version of GPUI (`crates/gpui/`), specifically for custom hardware texture painting (`paint_hardware_texture`). This prevents the project from cleanly pulling upstream updates from Zed.
**The Solution:**
*   Isolate the D3D11 rendering logic. Create an abstraction layer or work with the GPUI maintainers to upstream the hardware texture integration.
*   If upstreaming isn't immediately possible, minimize the patch surface area so that rebasing against upstream GPUI is a trivial script rather than a manual merge conflict nightmare.

## Phase 2: Robustness & Eradicating "Hacks"

### 2.1 Deterministic File Operations (No More Sleep Hacks)
**The Problem:** Post-processing relies on arbitrary thread sleeps (e.g., `std::thread::sleep(std::time::Duration::from_millis(200))`) to wait for GStreamer to release file locks on segments.
**The Solution:**
*   Hook deeply into the `splitmuxsink` element's signals. Wait for the explicit `format-location` or `fragment-closed` signals and verify file handles are dropped.
*   Alternatively, use the `notify` crate to watch for explicit `CloseWrite` filesystem events on the segment files before attempting to rename or probe them.
*   **Benefits:** Ensures 100% reliability on fast NVMe drives and slow mechanical HDDs alike.

### 2.2 Robust Local HLS Server
**The Problem:** The custom HLS server manually parses raw HTTP TCP streams, making it vulnerable to edge cases, malformed requests, and path traversal vulnerabilities.
**The Solution:**
*   Replace the raw `TcpListener` parsing loop with a lightweight, battle-tested asynchronous web framework like `axum` or `warp`.
*   Implement proper routing, header parsing, and static file serving with built-in range request support (e.g., `tower-http`'s `ServeDir`).
*   **Benefits:** Eliminates parsing bugs, handles HTTP/1.1 edge cases automatically, and drastically improves security and stability.

### 2.3 Accurate Process Targeting
**The Problem:** `resolve_pid` guesses the main game process by looking for the executable with the highest memory consumption. This breaks for multi-process games or launchers.
**The Solution:**
*   Use robust Windows APIs to identify the primary game window. Correlate `HWND` (from the UI capture selection) to the actual rendering PID using `GetWindowThreadProcessId`.
*   For audio capture targeting, trace the WASAPI session directly to the PID associated with the `HWND` being captured.

## Phase 3: Developer Experience & Ecosystem

### 3.1 Database Migrations
**The Problem:** `history.db` is initialized with a raw `CREATE TABLE IF NOT EXISTS` query. Future schema updates will break existing installations.
**The Solution:**
*   Integrate a lightweight SQLite migration library like `rusqlite_migration` or `refinery`.
*   **Benefits:** Allows safely adding new features (e.g., tracking recording resolution, encoder used, or bookmarks/highlights) without destroying user data.

### 3.2 Dynamic Metadata & Steam Integration
**The Problem:** `resolve_steam_app_id` contains hardcoded game slugs (e.g., `eldenring`, `baldursgate`).
**The Solution:**
*   Completely remove hardcoded lists. Rely entirely on the Steam Search API or the local Steam installation (parsing `appmanifest_*.acf` files in the Steam library folder).
*   Add a UI element allowing users to manually search for or override the game artwork if the automatic detection fails.

### 3.3 Seamless Dependency Management
**The Problem:** The app silently fails or degrades if `ffmpeg.exe`, `ffprobe.exe`, or `libmpv-2.dll` are not manually placed in the `bin/` directory by the user.
**The Solution:**
*   Implement a "First Launch Setup" wizard that automatically downloads and verifies the correct, statically compiled versions of FFmpeg and the required MPV libraries from a trusted source.
*   Alternatively, use an installer framework (like WiX Toolset) to bundle these dependencies securely.

## Summary

By executing this plan, Rekaptr will transition from relying on "optimistic heuristics" to "deterministic guarantees." The resulting 10/10 application will be a rock-solid, maintainable, and scalable piece of Windows software that rivals commercial screen recording solutions.
