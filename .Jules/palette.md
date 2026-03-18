## 2024-05-15 - Add Tooltips to Dashboard Buttons
**Learning:** Icon-only buttons in the Luma UI (GPUI framework) lack inherent accessibility and discoverability. The adabraka_ui framework provides a `Tooltip` component that works nicely by wrapping the target element and accepting a `placement` configuration.
**Action:** When adding or modifying icon-only controls, always wrap them in a `adabraka_ui::components::tooltip::Tooltip` to ensure the action is clear to the user.
