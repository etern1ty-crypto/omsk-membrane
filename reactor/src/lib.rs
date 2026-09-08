//! Host-side CLI components for OMSK Membrane.
//!
//! Prefer the `gulag` crate for embedding the pure byte scanner. The CLI's
//! process-global signal guard is not an embedding API.

pub mod config;
pub mod guest;
pub mod io_pump;
pub mod output;
pub mod report;
pub mod shutdown;
