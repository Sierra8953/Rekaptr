## 2024-04-08 - Use drain() instead of clone() + clear() for HashSets
**Learning:** Found an anti-pattern in `src/ui/clips.rs` where a `HashSet<String>` (`this.selected_clips`) was being cloned to iterate over it, followed by clearing the original set. This results in unnecessary O(N) heap allocations and string copying, especially for collections containing `String`.
**Action:** Replace `.clone()` followed by `.clear()` with `.drain()` to safely consume the collection elements directly without redundant allocations.
