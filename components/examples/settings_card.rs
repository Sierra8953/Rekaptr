//! Settings Card — stub. Implement in this file.
//!
//! Titled card with optional description and a body slot.
//! See `src/ui/settings/mod.rs::settings_card` for the current ad-hoc version.
//!
//! Run with: cargo run --example settings_card

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Settings Card"));
}
