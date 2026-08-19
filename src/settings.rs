//! What the player has told Emilio to do, and where that is kept.
//!
//! Two things for v0.1: which directory the saves are in, and which keys do what. Both have a
//! default good enough to run on, so a first start needs no setup - the file is written with those
//! defaults in it, which is also how the player finds out where it lives and what it may contain.
//!
//! Read and written from the same struct, on purpose. A config that is parsed one way and written
//! another drifts, and the drift shows up as a setting that quietly fails to survive a restart.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::hotkeys::Bindings;

mod saves;

/// Everything the player can set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// The directory the game keeps its saves in.
    ///
    /// Detected on a first start, and overridable because the detection can only find the usual
    /// place: a second Windows account, a copied save set, or a game installed for another user
    /// all live somewhere this cannot guess.
    pub saves: PathBuf,

    /// Which keys do what.
    pub hotkeys: Bindings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            saves: saves::directory().unwrap_or_default(),
            hotkeys: Bindings::default(),
        }
    }
}

impl Settings {
    /// Reads the settings, writing the defaults first if there is nothing there yet.
    ///
    /// A missing file is a first start rather than a problem. A malformed one is a problem, and is
    /// reported rather than replaced: overwriting it would throw away the bindings the player was
    /// in the middle of getting wrong, and they cannot fix a typo they cannot see.
    pub fn read_or_create(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = Self::default();

                tracing::debug!(path = %path.display(), "no settings yet, writing the defaults");
                settings.write(path)?;

                Ok(settings)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Reads settings out of the text of a file, naming that file if it will not parse.
    ///
    /// Separate from reading the file so that the rules can be exercised without one.
    fn parse(text: &str, path: &Path) -> Result<Self> {
        toml::from_str(text).map_err(|source| Error::Unreadable {
            path: path.to_owned(),
            source,
        })
    }

    /// Writes the settings, creating the directory if it is not there.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, toml::to_string_pretty(self)?)?;
        tracing::debug!(path = %path.display(), "wrote the settings");

        Ok(())
    }
}

/// Where the settings file lives for this user.
pub fn path() -> Result<PathBuf> {
    let directories = ProjectDirs::from("", "", "emilio").ok_or(Error::Homeless)?;

    Ok(directories.config_dir().join("emilio.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of writing the defaults out: what comes back has to be what went in.
    #[test]
    fn settings_survive_a_round_trip_through_the_file_format() {
        let settings = Settings {
            saves: PathBuf::from(r"C:\Users\someone\Saved Games\Diablo II Resurrected"),
            hotkeys: Bindings::default(),
        };

        let written = toml::to_string_pretty(&settings).expect("settings should serialise");
        let read: Settings = toml::from_str(&written).expect("settings should parse");

        assert_eq!(read, settings);
    }

    /// A hotkey is stored as the string it prints as, and that string has to parse back to it.
    /// Nothing else checks that the two halves of the crate's own conversion agree.
    #[test]
    fn a_binding_is_written_as_something_that_reads_back() {
        let written = toml::to_string_pretty(&Bindings::default()).expect("bindings should write");

        assert!(written.contains(r#"start = "alt+KeyQ""#), "{written}");
        assert!(written.contains(r#"stop = "alt+KeyW""#), "{written}");
        assert!(written.contains(r#"pause = "control+Space""#), "{written}");

        let read: Bindings = toml::from_str(&written).expect("bindings should parse");

        assert_eq!(read, Bindings::default());
    }

    /// A file naming only one setting keeps the defaults for the rest, or every new setting added
    /// later would break every config file already out there.
    #[test]
    fn a_partial_file_leaves_the_rest_at_their_defaults() {
        let read: Settings = toml::from_str(r#"saves = "D:\\elsewhere""#).expect("should parse");

        assert_eq!(read.saves, PathBuf::from(r"D:\elsewhere"));
        assert_eq!(read.hotkeys, Bindings::default());
    }

    /// A misspelled key is the player's typo, and silently ignoring it means a setting that does
    /// nothing for reasons they cannot see.
    #[test]
    fn a_setting_that_is_not_one_is_refused() {
        let read = toml::from_str::<Settings>(r#"save = "D:\\typo""#);

        assert!(read.is_err());
    }

    /// A key nobody can press is refused rather than dropped, and the message names the file it
    /// came from, since that message is all the player has to go on.
    #[test]
    fn an_impossible_binding_is_reported_against_its_file() {
        let text = "[hotkeys]\nstart = \"not a key\"\n";

        let error = Settings::parse(text, Path::new("nonsense.toml")).expect_err("should refuse");

        assert!(error.to_string().contains("nonsense.toml"), "{error}");
    }
}
