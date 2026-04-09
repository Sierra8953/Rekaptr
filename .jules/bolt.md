
## 2024-04-09 - Avoid Unnecessary Allocations with HashSet drain
**Learning:** In UI event listeners, iterating through a `HashSet<String>` by using `.clone()` followed by `.clear()` incurs a heavy cost of allocating memory and copying all strings.
**Action:** Use `.drain()` instead to avoid the allocation and string copying overhead when iterating through collections that are discarded after use.
