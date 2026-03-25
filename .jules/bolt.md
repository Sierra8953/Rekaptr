# Bolt's Journal

## 2024-05-24 - Window Enumeration Performance
**Learning:** In Windows UI programming using `EnumWindows`, it's critical to filter out invisible or nameless windows early using `IsWindowVisible` and `GetWindowTextW` length checks *before* performing expensive OS queries like PID lookups (`GetWindowThreadProcessId`) or process tree queries (`sysinfo::System::process`). Bypassing sysinfo queries for the vast majority of background windows prevents significant performance degradation, especially during rapid foreground change events where `EnumWindows` is polled.
**Action:** When filtering operating system entities like windows or processes, always place fast attribute checks (like title length or visibility) before expensive system-level queries.