## 2025-02-24 - Missing Tooltips on Icon-Only Buttons
**Learning:** Found several icon-only buttons (like refresh, back, forward, record) in the dashboard without tooltips, making them less discoverable/accessible.
**Action:** When adding icon-only UI controls using `Button::new("id", "").icon(...)`, wrap them in an `adabraka_ui::components::tooltip::Tooltip` component.
