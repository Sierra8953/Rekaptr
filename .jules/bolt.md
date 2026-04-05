## 2024-05-18 - Fast Window Enumeration in Windows UI
**Learning:** When using `EnumWindows`, calling OS queries like PID lookups or process tree queries on every window is a massive performance bottleneck because there are many invisible or nameless windows.
**Action:** Filter invisible and nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries. A `len == 0` check handles both API errors and nameless windows.
