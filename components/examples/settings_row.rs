//! Settings Row — stub. Implement in this file.
//!
//! Label + optional description on the left, control on the right, bottom border.
//! Pairs with `settings_card`.
//!
//! Run with: cargo run --example settings_row

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Settings Row"));
}
