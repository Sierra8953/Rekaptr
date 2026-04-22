
## 2024-04-22 - Add Tooltips to Icon-Only Buttons
**Learning:** Icon-only buttons in the adabraka UI framework lack accessibility when not wrapped in tooltips. There are no native HTML aria-labels directly configurable on the button in some implementations.
**Action:** When adding icon-only UI controls (such as the Grid/Table toggles), wrap them in `adabraka_ui::components::tooltip::Tooltip` (`Tooltip::new(...).child(...)`) to ensure accessibility and discoverability.
