
## 2024-05-18 - Early exit on nameless windows and avoiding redundant OS queries
**Learning:** In Windows UI programming within this repository (e.g., `src/game_detector.rs`), a key performance pattern when using `EnumWindows` is to filter invisible/nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries like PID lookups.
**Action:** When working with EnumWindows or similar iteration structures that query the OS, perform lightweight checks (like string length) first to short-circuit the loop, and consolidate any necessary system queries (like `sysinfo::System::process` lookups) to avoid redundant allocations and lookups within the same callback.
