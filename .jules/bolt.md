## 2024-05-24 - Early string length check for EnumWindows callbacks
**Learning:** Calling GetWindowThreadProcessId and process tree lookups on every visible window is an anti-pattern. There are hundreds of visible-but-nameless utility windows, leading to severe performance degradation.
**Action:** Always filter invisible/nameless windows early using GetWindowTextW length checks before performing expensive OS queries like PID lookups or process tree queries.
