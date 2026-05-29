//! Chip — stub. Implement in this file.
//!
//! Optional dot + label, tinted pill. Variants: recording / idle / info / warn.
//! Unifies `status_chip` (dashboard) and `track_source_pill` (add_source).
//!
//! Run with: cargo run --example chip

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Chip"));
}
