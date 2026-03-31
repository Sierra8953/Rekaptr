## 2024-05-20 - Optimize EnumWindows performance for game detection
**Learning:** In Windows UI programming, `EnumWindows` can degrade performance if OS queries (like `GetWindowThreadProcessId` or `sysinfo` process tree lookups) are executed on every single window handle. Many handles correspond to nameless or invisible system windows.
**Action:** Filter nameless windows early using `GetWindowTextW` length checks *before* making any process-level OS queries.
