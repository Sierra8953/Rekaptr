## 2024-04-14 - Icon-only buttons lacking context
**Learning:** Found that multiple icon-only buttons in the main video player control bar (Record, Rewind, Play/Pause, Forward, Reload, Markers) lacked any accessible labels or tooltips, meaning screen readers would announce "button" and visual users wouldn't know exactly what action it performs without trial and error.
**Action:** When adding or modify icon-only UI controls, always wrap them in an `adabraka_ui::components::tooltip::Tooltip` to ensure accessibility and discoverability for the user.
