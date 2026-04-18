## 2024-04-18 - File I/O Optimization in Main Thread
**Learning:** GPUI `on_click` listeners run directly on the main UI thread. Any synchronous disk operations like `std::fs::remove_file` will block the application from rendering, causing stutter.
**Action:** When performing file deletions or other disk I/O from a UI event listener, always offload the work using `cx.background_executor().spawn()`.
