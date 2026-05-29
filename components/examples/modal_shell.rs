//! Modal Shell — stub. Implement in this file.
//!
//! Header (title + close), scrollable body, sticky footer slot.
//! `add_source.rs` reinvents this; export and preview modals will want it too.
//!
//! Run with: cargo run --example modal_shell

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Modal Shell"));
}
