## 2024-05-24 - EnumWindows Performance Optimization
**Learning:** In Windows UI programming (e.g., `src/game_detector.rs`), a key performance pattern when using `EnumWindows` is to filter invisible/nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries like PID lookups or process tree queries to prevent performance degradation.
**Action:** Always filter by window title length and visibility first before resolving processes during window enumeration.
