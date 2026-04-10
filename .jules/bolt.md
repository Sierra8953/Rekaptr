## 2024-04-10 - Avoid `.clone()...clear()` on `HashSet` in UI callbacks
**Learning:** We found a pattern where `.clone()` was being called on a `HashSet<String>` to iterate over it in an event listener, followed by `.clear()`. This causes an unnecessary heap allocation and clones of all strings.
**Action:** Use `.drain()` instead to iterate and remove elements in a single pass without any extra heap allocation. This is a common pattern for event listeners in UI systems that consume a temporary list of collected selections.
