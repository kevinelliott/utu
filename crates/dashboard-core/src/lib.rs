//! Provider-neutral domain contracts for Utu.
//!
//! This crate deliberately has no Tauri, browser, database, or process
//! dependencies. Connectors report evidence into these types; the UI renders
//! the same truth model on desktop and web.

mod models;
mod policy;

pub use models::*;
pub use policy::*;
