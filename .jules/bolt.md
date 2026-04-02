## 2024-04-02 - Optimize Game Window Enumeration
**Learning:** Filter invisible/nameless windows early using `GetWindowTextW` length checks before performing expensive OS queries like PID lookups or process tree queries to prevent performance degradation.
**Action:** Always check the easiest-to-verify, cheapest condition (like window title length) before performing costly OS operations, especially inside tight loops like `EnumWindows`.
