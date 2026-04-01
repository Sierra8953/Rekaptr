## 2024-04-01 - Early filtering in Win32 EnumWindows
**Learning:** In Windows UI programming within this repository (e.g., `src/game_detector.rs`), a key performance pattern when using `EnumWindows` is to filter invisible/nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries like PID lookups or process tree queries to prevent performance degradation.
**Action:** When working with `EnumWindows` and system process enumeration, always filter based on window titles and visibility first before querying the underlying process tree.
