//! Cloud integration for the Teams feature (rekaptr.dev).
//!
//! The desktop app is local-first; this module is the one networked surface.
//! `auth` handles signing in to the cloud account (Clerk OAuth + PKCE) and
//! caching the per-user token; a future `api` module will wrap the team/clip
//! endpoints. See `dev/docs/teams-auth-flow.md`.

pub mod api;
pub mod auth;
pub mod upload;

#[allow(unused_imports)] // consumed by the Teams UI in a later step (Phase 7)
pub use auth::{CloudAuth, CloudAuthError};
