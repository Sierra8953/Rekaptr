## 2024-03-24 - Tooltips for Playback Controls
**Learning:** Fullscreen playback controls built with icon-only divs (`Icon::new`) lacked contextual hints, making them less discoverable and accessible. The interactive `div` elements required wrapping in `Tooltip` components from the design system to provide text labels.
**Action:** Always verify that all icon-only interactive elements, especially in overlays or full-screen modes, are wrapped with `Tooltip` components to ensure accessibility and usability.
