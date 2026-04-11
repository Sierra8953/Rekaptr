# Luma Feature Roadmap — 2026

## Q3 2026 — Social & Sharing

### Cloud Clips
- One-click upload to Luma cloud (generates shareable link)
- Automatic thumbnail generation and clip preview (gif/webm)
- Privacy controls: public, unlisted, friends-only
- Embed support for Discord, Twitter, Reddit
- Configurable auto-upload for instant replays

### Clipping UX Overhaul
- In-app trim editor with frame-accurate scrubbing
- Multi-segment clip stitching (combine moments from different sessions)
- Clip templates: intro/outro overlays, watermarks
- Drag-and-drop timeline with transitions between stitched segments
- Batch export with queue system

### Audio Mixing & Separation
- Per-application audio track routing (extractable in export)
- Real-time audio ducking (auto-lower game audio when mic is active)
- Noise suppression powered by RNNoise/Krisp-style model
- Separate audio tracks in exported files (OBS-style multi-track)
- Audio waveform visualization on timeline

### Scene System (OBS-Parity)
- Named scene presets (e.g., "Gaming", "Desktop", "Camera Only")
- Layer-based composition: game capture + webcam + overlays
- Hotkey scene switching during recording
- Per-scene encoder and audio routing presets
- Source transforms: crop, resize, position, opacity

### Webcam & Overlay Support
- Picture-in-picture webcam overlay with position/size presets
- Chroma key (green screen) for webcam feed
- Custom image/text overlays (watermark, stream labels)
- Animated overlay support (webm/gif/lottie)

---

## Q4 2026 — Intelligence & Platform

### AI Highlights
- Local ML model detects kill feeds, objectives, clutch moments
- Auto-generates highlight reel from a full session
- Game-specific detection profiles (FPS frags, MOBA teamfights, racing finishes)
- Confidence-scored moments with one-click accept/reject
- "Best of the week" auto-compilation

### Luma Cloud Platform
- User profiles with clip feed and stats
- Follow system and activity feed
- Game-specific clip discovery and trending pages
- Clip reactions and comments
- Creator analytics: views, shares, engagement

### Multi-Monitor & Display Capture
- Independent per-monitor capture with separate encoders
- Region capture (arbitrary rectangle on any monitor)
- Auto-detect which monitor the game is on
- Ultra-wide and portrait monitor support
- Virtual desktop capture (Windows 11)

### Performance & Encoding
- AMD AMF encoder support (RX 6000+)
- Intel QuickSync encoder support (Arc GPUs + integrated)
- AV1 software fallback for systems without hardware encoders
- Adaptive bitrate: automatically scale quality based on GPU/disk headroom
- Recording performance overlay (FPS impact, encoder load, disk write speed)

### Streaming (Preview)
- RTMP output to Twitch/YouTube/Kick
- Simultaneous record + stream with independent quality settings
- Stream-specific scene switching and overlays
- Chat overlay integration
- Go-live notifications to followers on Luma Cloud

### Integrations
- Discord Rich Presence (show recording status, link to clips)
- Direct share to Discord channels via webhook
- Steam achievement/screenshot event triggers (auto-clip on achievement)
- Game API integrations for richer highlight detection (Riot, Valve, etc.)
- OBS scene collection import

---

## Future (2027+)

- Mobile companion app (clip management, push notifications for highlights)
- Collaborative clip editing
- Luma Replay Theater (watch clips together in sync)
- Plugin SDK for community extensions
- Linux support (PipeWire + VA-API)
