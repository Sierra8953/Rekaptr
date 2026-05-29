//! Labeled Value / Detail Item — stub. Implement in this file.
//!
//! Label-above-value pair. Replaces `render_detail_item` (clips.rs:1361)
//! and the many ad-hoc `VStack(label + value)` blocks.
//!
//! Run with: cargo run --example labeled_value

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Labeled Value"));
}
