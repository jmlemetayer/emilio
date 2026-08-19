//! The error type shared by the whole application, and the [`Result`] that carries it.
//!
//! Every one of these reaches a player rather than a developer, so each says what to do about it
//! where there is anything to be done: which file, which setting, which key.

use thiserror::Error;

/// Anything that can stop Emilio following the game.
#[derive(Debug, Error)]
pub enum Error {
    /// A file could not be read or written, or a thread could not be started.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Anything d2r could not do, such as starting a sensor.
    #[error(transparent)]
    D2r(#[from] d2r::Error),

    /// The operating system refused the hidden window the hotkeys hang from.
    #[error(transparent)]
    Hotkeys(#[from] global_hotkey::Error),

    /// The thread owning the hotkeys stopped before saying whether it was ready.
    #[error("the hotkey thread stopped before it was ready")]
    HotkeysStopped,
}

/// A [`Result`](std::result::Result) carrying this application's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
