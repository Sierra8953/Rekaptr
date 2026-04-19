## 2024-04-19 - Use Tooltip for Icon-only Buttons
**Learning:** In GPUI/Adabraka UI, standard HTML ARIA labels are not used. Icon-only controls must be wrapped in `adabraka_ui::components::tooltip::Tooltip` to ensure accessibility and discoverability.
**Action:** When adding or modifying icon-only UI controls, always wrap them in a Tooltip.
