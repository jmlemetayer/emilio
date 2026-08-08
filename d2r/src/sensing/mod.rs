//! Raw observation of the operating system: whether the game is running, and what it writes.
//!
//! Each sensor lives in its own module, runs on its own, and only ever *emits*. They share no
//! state and they answer no questions about meaning: deciding that a particular burst of writes
//! means "the player left a game" belongs to the classifier, not here. Sensors are wired together
//! by giving them clones of a single channel sender, which merges their output into one ordered
//! stream.

use std::path::PathBuf;

pub mod file;
pub mod process;

/// A single raw observation, carrying no interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsEvent {
    /// A game process appeared, identified by its process id.
    ProcessStarted(u32),

    /// A game process that had been seen is gone.
    ProcessStopped(u32),

    /// A file appeared in the watched directory.
    FileCreated(PathBuf),

    /// A file in the watched directory was written to.
    FileModified(PathBuf),

    /// A file disappeared from the watched directory.
    FileRemoved(PathBuf),
}
