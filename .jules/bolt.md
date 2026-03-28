## YYYY-MM-DD - Early filtering in Window Enumeration
**Learning:** In Windows UI programming within this repository (e.g., `src/game_detector.rs`), a key performance pattern when using `EnumWindows` is to filter invisible/nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries like PID lookups or process tree queries to prevent performance degradation.
**Action:** Move `GetWindowTextW` checks to happen before `GetWindowThreadProcessId` in `EnumWindows` callback, reducing unnecessary cross-process queries for invisible/nameless windows.
