## 2024-04-15 - GPUI Accessibility for Icon-only Buttons
**Learning:** GPUI/Adabraka UI does not use standard HTML ARIA labels. Instead, accessibility and discoverability for icon-only buttons are provided by wrapping the component in a `Tooltip` (e.g. `Tooltip::new("Label").placement(TooltipPlacement::Right).child(...)`).
**Action:** Always wrap icon-only UI controls in a `Tooltip` component to ensure accessibility and discoverability for the user.
