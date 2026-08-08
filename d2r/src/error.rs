//! The error type shared by the whole crate.

use thiserror::Error;

/// Anything that can go wrong while observing the game.
#[derive(Debug, Error)]
pub enum Error {
    /// The operating system refused something we asked of it, such as spawning a watcher thread.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The file watching backend failed to start, or lost the directory it was watching.
    #[error(transparent)]
    Notify(#[from] notify::Error),
}

/// A [`Result`](std::result::Result) carrying this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
