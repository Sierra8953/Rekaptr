//! Stat Strip — stub. Implement in this file.
//!
//! Horizontal row of stat cells with dividers. See `dashboard.rs::stat_strip`.
//!
//! Run with: cargo run --example stat_strip

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Stat Strip"));
}
