
## 2024-05-18 - Tooltips vs ARIA in Adabraka UI
**Learning:** In GPUI/Adabraka UI, standard HTML ARIA labels are not used. Instead, when adding or modifying icon-only UI controls, they must be wrapped in an `adabraka_ui::components::tooltip::Tooltip` to ensure accessibility and discoverability.
**Action:** When adding icon-only buttons in the future, always wrap them with `Tooltip::new("Text").placement(TooltipPlacement::Top).child(Button::new(...))` instead of looking for aria attributes.
