## 2024-03-27 - Icon-only buttons must have tooltips
**Learning:** Icon-only buttons without labels or tooltips present significant accessibility and discoverability barriers for users. Memory explicitly instructs: 'When adding or modifying icon-only UI controls, wrap them in an `adabraka_ui::components::tooltip::Tooltip` to ensure accessibility and discoverability for the user.'
**Action:** Always wrap icon-only buttons in `adabraka_ui::components::tooltip::Tooltip`.
