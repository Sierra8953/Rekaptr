# mockups/

Static UI mockups for Rekaptr — standalone, runnable visual prototypes. No real
app state, video, or persistence; placeholder data and a local palette only.

These are written with the `rsx!` macro (`crates/rekaptr-rsx`) to exercise it on
real layouts before converting any production UI.

## Running

| Mockup | Command |
|---|---|
| Main dashboard | `cargo run --example dashboard_mockup` |
| Source settings popup redesign | `cargo run --example game_settings_mockup` |
| In-game overlay HUD | `cargo run --example overlay_mockup` |

Each mockup is wired up as a `[[example]]` of the `rekaptr` package in the root
`Cargo.toml`.

## Adding a mockup

1. Create `mockups/<name>.rs` with its own `fn main()` (see `dashboard.rs`).
2. Register it in the root `Cargo.toml`:
   ```toml
   [[example]]
   name = "<name>_mockup"
   path = "mockups/<name>.rs"
   ```
