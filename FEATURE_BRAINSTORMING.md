# Rekaptr Feature Brainstorming

This document contains ideas for new features and improvements for the Rekaptr application.

## 1. Voice-Activated Clipping ("Rekaptr, clip that!")

**Description:**
Allow users to trigger a clip capture completely hands-free using a wake word or phrase. This is especially useful in intense gaming moments where reaching for a hotkey might disrupt gameplay.

**Architectural Fit:**
Integrate a lightweight, offline Speech-to-Text (STT) engine like Whisper (via `whisper.cpp` Rust bindings) as a background task. Since the `MicProvider` stream is implemented in `src/audio.rs` and `src/mic_dsp.rs`, the STT engine could passively listen to this stream. When the wake phrase is detected, it would send a signal to the `AppState` to trigger the `save_clip` function, similar to how global hotkeys in `src/main.rs` and `src/hotkeys.rs` operate.

## 2. Integrated Virtual Soundboard

**Description:**
Let users bind sound effects or memes to hotkeys, which are then injected directly into their outgoing microphone feed (for Discord/in-game chat) and the recording.

**Architectural Fit:**
A natural extension of the existing audio routing (`src/virtual_audio_router.rs`). Create a `SoundboardProvider` that decodes local audio files. Because `engine.rs` already has the infrastructure for mixing multiple `appsrc` elements into the GStreamer pipeline, injecting these soundboard clips into the mic mix and the final recording pipeline is feasible with the current setup.

## 3. Replay Buffer "Quick Share" (Discord Webhook Integration)

**Description:**
Allow users to instantly compress and share a quick 15-second or 30-second clip directly to a pre-configured Discord channel via a Webhook, bypassing the need to open the full editor or manually upload a large file.

**Architectural Fit:**
Rekaptr's rolling buffer makes this fast. A secondary, low-quality re-encode pipeline could be added in `src/engine.rs` that outputs a small temporary file and then POSTs it to a Discord Webhook.

## 4. Interactive Timeline Bookmarks (User Annotations)

**Description:**
Expand the current hotkey system so users can drop specific types of markers while playing (e.g., F9 for "Funny", F10 for "Clutch/Good Play", F11 for "Bug/Error"). When reviewing the clip later in the editor, these markers appear as different colored flags on the timeline, making it easy to find specific moments in long sessions.

**Architectural Fit:**
This would involve expanding `src/hotkeys.rs` to handle multiple marker types and emit different events to `AppState`. `src/ui/timeline.rs` would then be updated to parse and render these different markers visually. You would also update `src/db.rs` to ensure the marker types are saved with the clip metadata.

## 5. Cloud Backup / Sync Integration

**Description:**
Automatically or manually push finished clips directly to cloud storage providers (Google Drive, Dropbox, AWS S3, or even a personal NAS) to permanently free up local disk space while keeping memories safe.

**Architectural Fit:**
This would involve adding a new module (e.g., `src/cloud.rs`) that uses the `reqwest` crate or provider-specific SDKs. It could hook into the end of the export process in `src/ui/export.rs` or run as a background queue, pushing the final `.mp4` files to the configured remote destination.

## 6. Game-Specific Metadata Tagging (API Integration)

**Description:**
Automatically tag clips with rich data from the game being played. For example, if playing League of Legends or Valorant, the clip could automatically be named and tagged with the current Champion, the match score, or the map name.

**Architectural Fit:**
Expand `src/game_detector.rs` to not only detect the `.exe` name but also make local API calls (like the Riot Client API) or read local log files. This metadata would then be saved to the database via `src/db.rs` and displayed in the GPUI dashboard (`src/ui/dashboard.rs`) to make searching for specific clips incredibly powerful.

## 7. In-Game Overlay (DirectX/Vulkan Hook)

**Description:**
Inject a minimal, non-intrusive overlay directly into the game to show recording status, microphone mute state, and "Clip Saved!" notifications so users don't need a second monitor to know Rekaptr is working.

**Architectural Fit:**
You would need a hooking mechanism (like `minhook` or creating a custom injection DLL in C++ that communicates back to Rekaptr via named pipes/RPC). This hook would draw directly onto the DirectX/Vulkan swapchain, receiving state updates from `src/main.rs`.

## 8. Optical Character Recognition (OCR) for Search

**Description:**
Periodically scan video frames for text (kill feeds, scoreboards, player names) and index it in the local database. Users could then search their clip library for specific players they encountered or game modes they played.

**Architectural Fit:**
Extend the GStreamer pipeline in `src/engine.rs` to sample frames (e.g., 1 fps) and send them to a lightweight OCR engine. The extracted text would be asynchronously pushed into the database (`src/db.rs`), making the search function in the dashboard (`src/ui/dashboard.rs`) context-aware.

## 9. VST3 Plugin Support for Microphone

**Description:**
Allow users to load professional VST3 audio plugins (like EQs, Compressors, or custom Noise Gates) directly into their microphone routing path for studio-quality voice capture without needing external software like Voicemeeter.

**Architectural Fit:**
Utilize a crate like `vst` or `baseplug` to load VST3 plugins inside `src/mic_dsp.rs` or `src/audio.rs`. The `MicProvider` would route its raw audio buffer through the plugin's process block before sending it down the GStreamer pipeline or to the WASAPI output.

## 10. Network Drive (NAS) Direct Recording with Smart Buffering

**Description:**
Optimize the GStreamer pipeline to record directly to a slow network drive (SMB/NFS share) without dropping frames, using a large intermediate memory or fast-SSD buffer to offload the storage burden from the main gaming drive.

**Architectural Fit:**
In `src/engine.rs`, configure elements like `queue` with large memory sizes or create a custom intermediate file sink on an SSD that gets asynchronously flushed to the network path. This ensures Rekaptr handles potential network latency spikes without interrupting the capture process.
