## 2024-04-03 - Tooltips for Icon-only Buttons
**Learning:** Icon-only buttons in the UI should be wrapped in `adabraka_ui::components::tooltip::Tooltip` to improve accessibility and provide context for users unfamiliar with the icons.
**Action:** Always verify if an icon-only button requires a tooltip, and if so, import and wrap it using `Tooltip::new("Action description").child(Button::new(...))`.
