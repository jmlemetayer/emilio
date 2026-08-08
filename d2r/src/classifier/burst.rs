//! Gathering the writes that belong to one action.
//!
//! One thing the player does produces several writes: saving touches the character file and the
//! shared stash, leaving a game adds the settings and control files. Judging them one at a time
//! says nothing, so they are collected until the writing stops and judged together: which file
//! moved *with* which is the whole signal.

use std::ffi::OsStr;
use std::path::Path;

/// The kind of file a path points at, as far as the classifier cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileKind {
    /// A character save.
    Character,
    /// A generated map. Written when a game is created, never on the way out.
    Map,
    /// Key bindings or control settings. Written when leaving a game, never on entry.
    Controls,
    /// The game's settings file. Written when leaving a game.
    Settings,
    /// Something we have no use for.
    Other,
}

impl FileKind {
    /// Classifies a path by its name.
    pub(super) fn of(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();

        match extension.as_str() {
            "d2s" => Self::Character,
            "ma0" | "ma1" | "ma2" | "ma3" | "map" => Self::Map,
            "ctl" | "ctlo" | "key" | "keyo" => Self::Controls,
            "json" if is_named(path, "settings.json") => Self::Settings,
            _ => Self::Other,
        }
    }
}

/// Whether a path's final component is `name`, ignoring case.
fn is_named(path: &Path, name: &str) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|actual| actual.eq_ignore_ascii_case(name))
}

/// What was written together, and what changed while it was.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Burst {
    /// Whose save was written, whether or not its contents moved.
    ///
    /// Its presence is also what says a save was written at all. If two characters are somehow
    /// written in the same burst the last one wins, which is as good an answer as any: the game
    /// only has one character in play.
    pub(super) character: Option<String>,

    /// A character save's contents actually changed.
    pub(super) character_changed: bool,

    /// How much the character save grew or shrank, in bytes. An item arriving or leaving the
    /// inventory changes the size; a level or an experience total does not.
    pub(super) size_delta: i64,

    /// A map was written.
    pub(super) map: bool,

    /// Controls or settings were written. Either one means the player is on their way out.
    pub(super) leaving: bool,
}

impl Burst {
    /// Folds one written path into the burst.
    ///
    /// `character` is who the file belongs to, and is only meaningful for a character save.
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
            FileKind::Map => self.map = true,
            FileKind::Controls | FileKind::Settings => self.leaving = true,
            FileKind::Other => {}
        }
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
        assert_eq!(kind_of("Vikhyat.d2s"), FileKind::Character);
        assert_eq!(kind_of("Vikhyat.ma0"), FileKind::Map);
        assert_eq!(kind_of("Vikhyat.ctl"), FileKind::Controls);
        assert_eq!(kind_of("Settings.json"), FileKind::Settings);
    }

    /// Windows does not care about case in filenames and neither can we.
    #[test]
    fn ignores_case() {
        assert_eq!(kind_of("VIKHYAT.D2S"), FileKind::Character);
        assert_eq!(kind_of("settings.json"), FileKind::Settings);
        assert_eq!(kind_of("SETTINGS.JSON"), FileKind::Settings);
    }

    /// A `.json` is only interesting when it is *the* settings file.
    #[test]
    fn does_not_mistake_other_json_for_the_settings() {
        assert_eq!(kind_of("something-else.json"), FileKind::Other);
    }

    #[test]
    fn treats_unknown_files_as_noise() {
        assert_eq!(kind_of("Vikhyat.d2i"), FileKind::Other);
        assert_eq!(kind_of("readme.txt"), FileKind::Other);
        assert_eq!(kind_of("no-extension"), FileKind::Other);
    }

    /// A save written with no change of contents has to stay distinguishable from one with.
    #[test]
    fn separates_a_write_from_a_change() {
        let mut burst = Burst::default();
        burst.add(FileKind::Character, Some("Vikhyat"), None);

        assert_eq!(burst.character.as_deref(), Some("Vikhyat"));
        assert!(!burst.character_changed);
    }

    #[test]
    fn accumulates_a_size_change() {
        let mut burst = Burst::default();
        burst.add(
            FileKind::Character,
            Some("Vikhyat"),
            Some(SizeChange { delta: 15 }),
        );

        assert!(burst.character_changed);
        assert_eq!(burst.size_delta, 15);
    }

    /// Files that belong to nobody must not invent a character, or a burst of pure noise would
    /// look like a save.
    #[test]
    fn only_a_character_save_names_a_character() {
        let mut burst = Burst::default();
        burst.add(FileKind::Map, None, None);
        burst.add(FileKind::Settings, None, None);

        assert_eq!(burst.character, None);
    }

    /// Either file is enough on its own; the game does not always write both.
    #[test]
    fn treats_controls_and_settings_alike() {
        let mut from_controls = Burst::default();
        from_controls.add(FileKind::Controls, None, None);

        let mut from_settings = Burst::default();
        from_settings.add(FileKind::Settings, None, None);

        assert!(from_controls.leaving);
        assert!(from_settings.leaving);
    }
}
