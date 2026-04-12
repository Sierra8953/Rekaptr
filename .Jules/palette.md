## 2024-05-18 - Sidebar Navigation Accessibility
**Learning:** Icon-only navigation buttons in the primary sidebar lacked textual labels or tooltips, severely impacting discoverability and accessibility. The app uses an immediate-mode UI paradigm via GPUI, where adding tooltips is elegantly handled by chaining the `.child()` method onto a `Tooltip::new()` builder.
**Action:** Always wrap `Icon` or purely visual components in an `adabraka_ui::components::tooltip::Tooltip` when designing core navigation elements.
