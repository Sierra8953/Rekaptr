//! Empty State — stub. Implement in this file.
//!
//! Icon + heading + sub + CTA, for empty lists / first-run views.
//!
//! Run with: cargo run --example empty_state

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Empty State"));
}
