## 2024-03-23 - Fast Window Enumeration Early Filtering

**Learning:** In Windows UI programming within this repository (e.g., `src/game_detector.rs`), performing OS queries like PID lookups (`GetWindowThreadProcessId`) or process tree queries (`sysinfo`) for every enumerated window causes severe performance degradation due to the sheer volume of hidden/system windows.

**Action:** Always filter invisible/nameless windows early using `GetWindowTextW` length checks *before* performing any expensive OS queries during window enumeration.
