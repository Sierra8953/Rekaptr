## 2024-03-22 - Window Enumeration Performance Pattern
**Learning:** In Windows UI programming using `EnumWindows`, a significant amount of overhead comes from querying PIDs and process names for shell/nameless windows that will eventually be discarded anyway.
**Action:** Always filter invisible or nameless windows early using `GetWindowTextW` length checks *before* performing expensive OS queries like `GetWindowThreadProcessId` or `sysinfo::System::process` lookups.
