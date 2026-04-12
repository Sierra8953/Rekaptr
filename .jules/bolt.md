## 2024-04-12 - Use .drain() instead of .clone() + .clear() for HashSets in event listeners
**Learning:** Using `.clone()` followed by `.clear()` on a `HashSet` during iteration causes an unnecessary heap allocation and copy of strings, which impacts performance in high-frequency UI paths.
**Action:** Use `.drain()` directly on the collection to iterate and clear it in place without extra allocations.
