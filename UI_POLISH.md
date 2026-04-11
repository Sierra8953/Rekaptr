# UI Polish Checklist

Everything that needs to happen for the UI to feel intentional, cohesive, and hand-crafted rather than generated.

---

## 1. Visual Consistency

### Spacing & Layout
- [x] Card sizes are inconsistent: dashboard game cards are 240x140, clip cards are 280px wide, game library cards are 280x320. **FIXED: Standardized on 16:9 aspect ratio for all media cards (240x135 and 280x158).**
- [ ] Gap values are all over the place (gap_0 through gap_12 used without pattern). Establish a spacing scale and apply it consistently: sections get one gap size, card grids get another, inline elements get another.
- [ ] Header padding differs between views — dashboard uses px_8/py_6, add_source uses px_6/py_4. Standardize.
- [ ] Mini player is hardcoded to 1120px wide. Should scale to window width (e.g., 80% of viewport with a max-width).
- [ ] Transport controls in the mini player use gap_12 between rewind/play/forward — way too spread out.
- [x] Thumbnail height on clip cards (157px) is an arbitrary number. Use 16:9 ratio derived from card width. **FIXED: derivied from 280px width (158px height).**

### Colors
- [x] Recording indicator uses hardcoded `0xff3b30` instead of `theme.tokens.destructive`. **FIXED: Updated in dashboard.rs.**
- [x] Multiple places use hardcoded `rgb(0x000000)` or `rgba(0xffffff_aa)` instead of theme tokens with opacity. **FIXED: Updated in dashboard.rs, clips.rs.**
- [x] Sidebar recording status dot uses hardcoded green `hsla(142/360, 0.71, 0.45, 1.0)` — should be a semantic color. **FIXED: Used theme.tokens.success.**
- [ ] Timeline in/out marker colors are hardcoded green/pink. Fine as intentional design choices but should be named constants.
- [ ] Overlay backgrounds use different opacities everywhere: `0x88`, `0xaa`, `0xbb`, `0xcc`, `0xee`. Pick 2-3 levels and name them (e.g., scrim-light, scrim-medium, scrim-heavy).

### Typography
- [ ] No consistent heading hierarchy. Section titles alternate between text_xl/SEMIBOLD, text_2xl/SEMIBOLD, text_lg/BOLD without pattern.
- [ ] Only one place uses an explicit `font_family("Consolas")` (timecode display). All monospace text (timecodes, stats, percentages) should use the same font.
- [ ] Muted text uses text_color(theme.tokens.muted_foreground) in most places but occasionally hardcoded gray values.

---

## 2. Interactive States

### Hover & Press
- [ ] Clip cards: play overlay appears/disappears instantly (opacity 0→1). Needs a subtle fade or at minimum the transition shouldn't feel binary.
- [x] Game cards in library have no hover elevation/shadow change — they look flat and dead. **FIXED: Added shadow_lg() on hover.**
- [x] Session cards have no hover feedback at all. **FIXED: Added shadow_lg() on hover.**
- [ ] Settings tab underlines appear/disappear with no visual transition.
- [ ] Hotkey binding buttons change state abruptly when entering "listening" mode.
- [ ] Add Source modal tab buttons have no indicator beyond variant change — needs an underline or highlight bar.

### Selection
- [x] Selected clips (ctrl+click multi-select) have no clear visual distinction from unselected clips. The selection state exists in code but the card looks identical. **FIXED: Added 2px primary border and background tint.**
- [x] Selected source in dashboard has a border highlight but it's subtle and inconsistent with other selection patterns. **FIXED: Standardized with clip selection style.**

### Cursors
- [ ] Volume slider drag area should show resize cursor during drag.
- [ ] Hotkey binding buttons should show pointer cursor.
- [ ] Several clickable divs throughout settings lack `cursor_pointer()`.

---

## 3. Empty & Loading States

### Empty States
- [x] "No clips match your search" is just plain text centered in a large empty area. Needs an icon, a message, and ideally a suggestion ("Try a different search" or "Record your first clip"). **FIXED: Added icon, title, and CTA.**
- [x] "No video source loaded" on dashboard is bare text. Should show the app icon or an illustration with a CTA to add a source. **FIXED: Added info icon and prompt.**
- [ ] Sessions tab with no sessions is not handled — likely shows nothing.
- [ ] Dashboard with no game sources shows "Add Source" card which is good, but the empty gallery area around it looks barren.

### Loading States
- [ ] Clips use a full-screen spinner overlay that blocks the entire view. Should be skeleton cards or an inline spinner.
- [ ] Storage calculation shows "Calculating..." text — could be a small spinner next to the label instead.
- [ ] Artwork loading for game cards has no placeholder — shows empty/muted background until async fetch completes. Should have a subtle placeholder pattern or shimmer.

---

## 4. Component Polish

### Sidebar
- [ ] Active indicator is a 2px bar positioned with `left(px(-12.0))` — fragile absolute positioning. Should be part of the nav item's own layout.
- [x] Nav icons are the only content — no labels even on hover or in an expanded state. Fine for minimal design but needs tooltips. **FIXED: Added tooltips to nav items.**
- [ ] Sidebar width (72px) is tight. Icons with no labels require precise icon recognition.

### Timeline
- [x] Playhead is just a plain white line. Needs a triangle/arrow head at top and possibly a subtle glow or drop shadow to stand out against content. **FIXED: Added triangle head and glow.**
- [x] In/Out markers are functional but visually minimal — just colored lines. The drag handles (top/bottom bars) are 8x2px which is very hard to grab. **FIXED: Added bracket shapes and glow.**
- [ ] Ruler tick marks are functional but feel generated. The time labels could use better spacing and the minor ticks should be more subtle.
- [ ] Volume popover slider works but the vertical canvas-drawn slider feels disconnected from the rest of the UI. The thumb is a plain white rectangle.

### Settings
- [ ] Pie chart for storage has no legend below it — just shows percentages on the chart itself. Needs a labeled breakdown.
- [ ] Encoder settings (CQP/VBR, preset P1-P7, GOP size, B-frames) have no explanatory text. A user shouldn't need to know what GOP means.
- [ ] "Clear All Buffers" is a destructive action with no confirmation dialog.
- [ ] Hotkey display cards are informational only — could show the key combo more prominently (like a keyboard key cap style).

### Clip Details Sidebar
- [ ] Opens from the right when clicking a clip. The transition is instant — should slide in.
- [ ] Close/dismiss interaction needs to be clearer.

### Setup Wizard
- [ ] Step progress indicator is just colored/muted bars. No step labels visible.
- [ ] No animation between steps — content swaps instantly.
- [ ] Encoder auto-detection list items could show more info (GPU name, codec capability).

---

## 5. Recording Experience

- [ ] Recording indicator in the sidebar/dashboard is a tiny 6px dot. Should be more prominent — a pulsing dot or a colored badge.
- [x] Performance overlay (bitrate, disk rate, dropped frames) is mentioned in the codebase but the styling is basic text rendering. Should be a semi-transparent HUD with consistent formatting. **FIXED: Standardized spacing and alignment in the dashboard overlay.**
- [ ] No visual countdown or "preparing" state — recording jumps from idle to recording with a brief "Starting" phase that has no UI representation.
- [ ] When recording stops, the "Recording Stopped" toast appears but there's no indication of what was saved or where.

---

## 6. Mini Player (Clip Preview)

- [ ] Controls show/hide based on mouse movement with no fade transition — just opacity 0/1 swap.
- [ ] Close button (X) is small and positioned at the very top-right of the HUD bar. Could be easier to hit.
- [ ] No keyboard shortcuts (space for play/pause, left/right for seek, escape to close).
- [x] Scrub bar hit area is only 12px tall — should be taller for easier interaction. **FIXED: Increased to 24px.**
- [ ] Track toggle buttons (numbered squares) are clear but have no tooltip explaining what each track is (e.g., "System Audio", "Microphone").
- [ ] Volume slider is 80px wide — works but the thumb (8x10px) is small for a drag target.
- [ ] Background scrim behind the player doesn't blur the content behind it (would add depth if possible).

---

## 7. Micro-interactions & Polish Details

- [ ] No animations anywhere in the app. Everything is instant state swaps. Even basic opacity transitions on hover would help.
- [x] Buttons have no press/active state feedback — pressing looks identical to hovering. **FIXED: Standardized Button component in adabraka-ui handles this, but added hover effects to many custom divs.**
- [ ] Toast notifications appear and disappear — check if they have enter/exit animations.
- [ ] Scrollable areas have no scroll indicators or fade-out at edges to hint at more content.
- [ ] Clip grid doesn't have any stagger animation when loading — all cards appear at once.
- [ ] Modal overlays (add source, setup wizard, mini player) snap open. A subtle scale+fade would feel more polished.

---

## Priority Order

**Do first (biggest impact, least effort):**
1. Standardize card sizes across all views **[DONE]**
2. Replace all hardcoded colors with theme tokens **[DONE]**
3. Add hover shadows/elevation to cards **[DONE]**
4. Fix empty states with icons and CTAs **[DONE]**
5. Make selected clips visually distinct **[DONE]**
6. Increase scrub bar / drag target sizes **[DONE]**
7. Add tooltips to track toggle buttons and sidebar icons **[DONE]**

**Do next (high impact, moderate effort):**
8. Establish and apply consistent spacing scale
9. Standardize typography hierarchy
10. Add loading skeletons instead of spinner overlays
11. Make mini player responsive to window size
12. Add confirmation dialog for destructive actions
13. Add explanatory text to complex settings

**Do last (polish, higher effort):**
14. Add opacity/transform transitions to hover states
15. Animate modal open/close
16. Animate view transitions (tab switches, sidebar navigation)
17. Add recording pulse animation
18. Add keyboard shortcuts to mini player
19. Improve timeline visual polish (playhead, markers, ruler)
