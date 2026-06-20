//! Core app logic, separated from the UI that drives it.
//!
//! Modules here own "what the app does" (building/running the ffmpeg export,
//! the recording pipeline lifecycle) as plain functions over plain inputs that
//! report progress/results through [`crate::state::AppState`]. The UI in
//! `src/ui/` stays a thin layer that gathers state, calls in here, and renders
//! the result — mirroring how `cloud/` is layered.

pub mod export;
