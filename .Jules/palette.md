## 2024-05-15 - Add Tooltips to Icon-Only Buttons
**Learning:** Icon-only UI controls without accessible labels or tooltips significantly degrade the discoverability and accessibility of primary media actions (like record, play/pause, seek).
**Action:** Always wrap `Button::new("...", "").icon(...)` patterns inside `adabraka_ui::components::tooltip::Tooltip` to provide on-hover context and screen reader visibility.
