# Feature Proposal: Rekaptr Soundboard

## 1. Overview
The Soundboard feature will allow users to manage a library of short audio clips (MP3, WAV, etc.) and trigger them via UI buttons or global hotkeys. These sounds will be mixed into the live microphone stream so they appear in recordings and (optionally) are audible to other applications (Discord, Games).

## 2. User Experience
- **Sidebar Tab:** A new "Soundboard" icon in the main navigation.
- **Library Grid:** A grid of customizable buttons. Each button shows a label, an optional icon/color, and a "Play" indicator.
- **Management:** Users can drag-and-drop audio files to import them.
- **Controls:** Each sound can have its own volume offset and "Loop" toggle.
- **Hotkeys:** Ability to bind any sound to a keyboard shortcut (e.g., `Ctrl + Num1`).

## 3. Technical Architecture

### 3.1 GStreamer Audio Injection
To mix audio into the mic stream without affecting latency, we will modify the `MicProvider` pipeline in `src/audio.rs`:
- **Current Pipeline:** `wasapisrc -> capsfilter -> audiornnoise -> appsink`
- **Proposed Pipeline:** 
  ```text
  wasapisrc -> capsfilter -> audiomixer name=mixer
  appsrc name=sb_injector -> decodebin -> audioconvert -> audioresample -> mixer.
  mixer -> audiornnoise -> appsink
  ```
- **SB Injector:** A background task that feeds raw PCM data from the selected audio file into the `sb_injector` element on demand.

### 3.2 The "Virtual Mic" Problem
To make the soundboard audible in other apps (Discord/Games), we have two paths:
1. **Third-Party Virtual Driver (Recommended):** Instruct the user to install a virtual cable (VB-Audio). Rekaptr outputs the mixed stream to the "Cable Input" which the user selects as their mic in Discord.
2. **WASAPI Loopback:** Attempt to route Rekaptr's processed audio to a hidden system output and capture it back. This is brittle and not recommended for Phase 1.

### 3.3 Data Persistence
- **Table `soundboard_items`:** 
  - `id`: UUID
  - `path`: Local filesystem path to audio file
  - `label`: Display name
  - `hotkey_vk`: Win32 VK code
  - `volume`: Float (0.0 - 1.0)

## 4. Implementation Phases

### Phase 1: Recording-Only Soundboard (MVP)
- Implement the "Soundboard" UI tab.
- Modify GStreamer `MicProvider` to include an `audiomixer`.
- Implement `SoundboardManager` to stream files into `appsrc`.
- Result: Sounds appear in Rekaptr recordings, but the user cannot hear them live, and Discord cannot hear them.

### Phase 2: Live Monitoring
- Add a "Monitor Soundboard" toggle in Settings.
- Route the Soundboard output to the user's default playback device (headphones) using a secondary GStreamer pipeline so they can hear what they are triggering.

### Phase 3: System-Wide Injection
- Add a "Virtual Output" setting.
- Allow the user to select a specific WASAPI device (like a Virtual Cable) as the destination for the combined Mic + Soundboard stream.

## 5. File Changes Required
- `src/state.rs`: Add `SoundboardItem` and `SoundboardState`.
- `src/audio.rs`: Significant refactor of `MicProvider` to support mixing.
- `src/ui/soundboard.rs`: New module for the grid UI.
- `src/hotkeys.rs`: Register new dynamic hotkeys for sound triggers.
- `src/config.rs`: Add soundboard library configuration.

## 6. Risks & Challenges
- **Sample Rate Mismatch:** Audio clips may be 44.1kHz while the mic is 48kHz. Must use `audioresample`.
- **Latency:** Adding a mixer to the mic path could introduce a few milliseconds of delay.
- **Concurrency:** Multiple sounds playing at once requires the `audiomixer` to be configured correctly to handle multiple sinks.
