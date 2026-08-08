//! Reusable sensing and parsing core for Diablo II: Resurrected companion tools.
//!
//! The crate observes; it does not act. Nothing here writes to a save file, to the game's memory,
//! or to the game installation.

pub mod classifier;
pub mod error;
pub mod sensing;

pub use error::{Error, Result};
