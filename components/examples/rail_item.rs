//! Rail Item / Nav Item — stub. Implement in this file.
//!
//! Icon + label + optional count + active state. Used by clips filter rail
//! (`clips.rs::clips_rail_item`) and the settings nav.
//!
//! Run with: cargo run --example rail_item

use components::preview;

fn main() {
    preview::run(|| preview::Placeholder::new("Rail Item"));
}
