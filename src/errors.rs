//! The error type shared by the whole application, and the [`Result`] that carries it.
//!
//! Every one of these reaches a player rather than a developer, so each says what to do about it
//! where there is anything to be done: which file, which setting, which key.

use std::path::PathBuf;

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

    /// The settings file is not the settings it claims to be.
    #[error("{path}: {source}")]
    Unreadable {
        /// Which file failed to parse.
        path: PathBuf,

        /// What the parser made of it.
        source: toml::de::Error,
    },

    /// The settings could not be turned back into a file.
    #[error(transparent)]
    Unwritable(#[from] toml::ser::Error),

    /// This user has no directory to keep a config file in.
    #[error("no configuration directory for this user")]
    Homeless,

    /// The save directory named in the settings is not there.
    #[error("no save directory at {saves}\n       set `saves` in {settings}")]
    NoSaves {
        /// Where the saves were expected.
        saves: PathBuf,

        /// The file that says so, and where to correct it.
        settings: PathBuf,
    },

    /// The operating system refused the hidden window the hotkeys hang from.
    #[error(transparent)]
    Hotkeys(#[from] global_hotkey::Error),

    /// The thread owning the hotkeys stopped before saying whether it was ready.
    #[error("the hotkey thread stopped before it was ready")]
    HotkeysStopped,
}

/// A [`Result`](std::result::Result) carrying this application's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
