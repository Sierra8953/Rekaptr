1. **Analyze UX/A11y Issue:** In `src/ui/dashboard.rs`, there are several icon-only buttons for video control (Record, Rewind, Play/Pause, Forward, Refresh) that lack tooltips or accessible labels. This makes their functionality hard to discover and inaccessible for users relying on screen readers or looking for explanations.

2. **Add Tooltips to Icon-only Buttons:**
    * Import `Tooltip` and `TooltipPlacement` from `adabraka_ui::components::tooltip`.
    * Wrap the `Button::new("btn-record", "")` with `Tooltip::new("Record/Stop").child(...)`.
    * Wrap the `Button::new("btn-back", "")` with `Tooltip::new("Rewind 10s").child(...)`.
    * Wrap the `Button::new("btn-play", "")` with `Tooltip::new("Play/Pause").child(...)`.
    * Wrap the `Button::new("btn-fwd", "")` with `Tooltip::new("Forward 30s").child(...)`.
    * Wrap the `Button::new("btn-refresh", "")` with `Tooltip::new("Refresh Video").child(...)`.
    * Wrap the marker buttons generated in `children()` with appropriate tooltips based on `kind.label()`.

3. **Verify Changes:**
    * Ensure the code compiles (`cargo check --bin luma`).
    * Check if the UI renders correctly.

4. **Complete Pre-commit Steps:**
    * Run the exact phrasing: 'Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.'

5. **Submit PR:**
    * Title: `🎨 Palette: [UX improvement] Add tooltips to dashboard video controls`
    * Description explaining What, Why, Impact, and Accessibility.
