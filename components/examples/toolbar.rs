//! Toolbar / Top Bar — stub. Implement in this file.
//!
//! Title + search + actions row. Generalizes `render_clips_top_bar`.
//!
//! Run with: cargo run --example toolbar

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Toolbar"));
}
