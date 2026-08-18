//! Gathering the writes that belong to one action.
//!
//! One thing the player does produces several writes, and judging them one at a time says nothing:
//! which file moved *with* which is the whole signal. They are collected until the writing stops
//! and judged together.

use std::ffi::OsStr;
use std::path::Path;

/// The kind of file a path points at, as far as the classifier cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileKind {
    /// A character save.
    Character,

    /// A character's controls, written when that character leaves a game.
    Controls,

    /// Something we have no use for.
    Other,
}

impl FileKind {
    /// Classifies a path by its extension.
    pub(super) fn of(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();

        match extension.as_str() {
            "d2s" => Self::Character,
            "ctl" | "ctlo" => Self::Controls,
            _ => Self::Other,
        }
    }
}

/// The character a file belongs to, taken from its name.
///
/// Saves and controls files are both named after their character, which is what lets the two be
/// matched up.
pub(super) fn character_of(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_string_lossy().into_owned())
}

/// What was written together, and what changed while it was.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Burst {
    /// Whose save was written, if one was.
    pub(super) character: Option<String>,

    /// That save's contents actually changed, rather than it merely being rewritten.
    pub(super) character_changed: bool,

    /// How much the save grew or shrank, in bytes.
    pub(super) size_delta: i64,

    /// Whose controls file was written, if one was.
    pub(super) controls_for: Option<String>,
}

impl Burst {
    /// Folds one written path into the burst.
    ///
    /// `character` is who the file belongs to, and is meaningless for anything else.
    pub(super) fn add(
        &mut self,
        kind: FileKind,
        character: Option<&str>,
        change: Option<SizeChange>,
    ) {
        match kind {
            FileKind::Character => {
                self.character = character.map(str::to_owned);

                if let Some(change) = change {
                    self.character_changed = true;
                    self.size_delta += change.delta;
                }
            }
            FileKind::Controls => self.controls_for = character.map(str::to_owned),
            FileKind::Other => {}
        }
    }

    /// Whether this burst says a character left a game.
    ///
    /// The controls file has to be the one belonging to the character whose save moved. Files that
    /// are not character-specific are written during ordinary play: changing an audio setting
    /// rewrites the settings, changing a keybind writes the keybind profile. Counting those would
    /// report a game being left in the middle of one, whenever such a change happened to land in
    /// the same burst as an autosave.
    ///
    /// Compared at the end rather than as the files arrive, so their order does not matter.
    pub(super) fn is_leaving(&self) -> bool {
        self.character.is_some() && self.character == self.controls_for
    }
}

/// A confirmed change of contents, and by how many bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SizeChange {
    pub(super) delta: i64,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn kind_of(name: &str) -> FileKind {
        FileKind::of(&PathBuf::from(name))
    }

    #[test]
    fn recognises_the_files_that_carry_signal() {
        assert_eq!(kind_of("Kate.d2s"), FileKind::Character);
        assert_eq!(kind_of("Kate.ctl"), FileKind::Controls);
        assert_eq!(kind_of("Kate.ctlo"), FileKind::Controls);
    }

    /// Windows does not care about case in filenames and neither can we.
    #[test]
    fn ignores_case() {
        assert_eq!(kind_of("KATE.D2S"), FileKind::Character);
        assert_eq!(kind_of("KATE.CTL"), FileKind::Controls);
    }

    /// All written during play. Treating any of them as a boundary would end runs that had not
    /// ended, whenever the write landed near an autosave.
    #[test]
    fn ignores_the_files_that_change_during_play() {
        assert_eq!(kind_of("Settings.json"), FileKind::Other);
        assert_eq!(kind_of("Custom.key"), FileKind::Other);
        assert_eq!(kind_of("Kate.key"), FileKind::Other);
        assert_eq!(kind_of("lootfilter.json"), FileKind::Other);
        assert_eq!(kind_of("WiseMammoth.fltr"), FileKind::Other);
    }

    /// Maps record going deeper into an area, not entering a game, so they say nothing about runs.
    #[test]
    fn ignores_maps() {
        assert_eq!(kind_of("Kate.ma0"), FileKind::Other);
        assert_eq!(kind_of("Kate.map"), FileKind::Other);
    }

    #[test]
    fn takes_the_character_from_the_filename() {
        assert_eq!(
            character_of(&PathBuf::from("Kate.d2s")).as_deref(),
            Some("Kate")
        );
        assert_eq!(
            character_of(&PathBuf::from("Kate.ctl")).as_deref(),
            Some("Kate")
        );
    }

    /// A save written with no change of contents has to stay distinguishable from one with.
    #[test]
    fn separates_a_write_from_a_change() {
        let mut burst = Burst::default();
        burst.add(FileKind::Character, Some("Kate"), None);

        assert_eq!(burst.character.as_deref(), Some("Kate"));
        assert!(!burst.character_changed);
    }

    #[test]
    fn accumulates_a_size_change() {
        let mut burst = Burst::default();
        burst.add(
            FileKind::Character,
            Some("Kate"),
            Some(SizeChange { delta: 95 }),
        );

        assert!(burst.character_changed);
        assert_eq!(burst.size_delta, 95);
    }

    #[test]
    fn a_save_and_its_own_controls_mean_leaving() {
        let mut burst = Burst::default();
        burst.add(FileKind::Character, Some("Kate"), None);
        burst.add(FileKind::Controls, Some("Kate"), None);

        assert!(burst.is_leaving());
    }

    /// The files can arrive in either order.
    #[test]
    fn the_order_the_files_arrive_in_does_not_matter() {
        let mut burst = Burst::default();
        burst.add(FileKind::Controls, Some("Kate"), None);
        burst.add(FileKind::Character, Some("Kate"), None);

        assert!(burst.is_leaving());
    }

    /// Someone else's controls file says nothing about this character's game.
    #[test]
    fn controls_for_another_character_do_not_mean_leaving() {
        let mut burst = Burst::default();
        burst.add(FileKind::Character, Some("Kate"), None);
        burst.add(FileKind::Controls, Some("Daphne"), None);

        assert!(!burst.is_leaving());
    }

    #[test]
    fn controls_with_no_save_do_not_mean_leaving() {
        let mut burst = Burst::default();
        burst.add(FileKind::Controls, Some("Kate"), None);

        assert!(!burst.is_leaving());
    }
}
