## 2025-03-22 - Add Tooltips for Icon-Only Buttons
**Learning:** Icon-only buttons without tooltips decrease discoverability and accessibility, leaving users guessing what buttons like "Play", "Record", or "Settings" do.
**Action:** When adding or modifying icon-only UI controls, wrap them in an `adabraka_ui::components::tooltip::Tooltip` to ensure accessibility and discoverability for the user.
