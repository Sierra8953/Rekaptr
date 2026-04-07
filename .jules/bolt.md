## 2025-04-07 - Optimize Windows EnumWindows Callback

**Learning:** When using `EnumWindows` from `windows-rs`, many invisible or nameless background windows are enumerated. Performing expensive system calls like `GetWindowThreadProcessId` or `sysinfo` queries for these windows creates a significant performance bottleneck, especially when repeated rapidly (e.g., polling every 2 seconds).

**Action:** Always filter out unwanted windows early by calling `GetWindowTextW` and checking if its length is zero before doing any PID resolution or process hierarchy lookups. This prunes the list quickly and drastically reduces the CPU time spent in the enumeration callback.