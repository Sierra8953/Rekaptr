//! Form Field — stub. Implement in this file.
//!
//! Wraps any control with label, optional description, optional error text.
//! Replaces `field_row` in `add_source.rs:953`.
//!
//! Run with: cargo run --example form_field

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Form Field"));
}
