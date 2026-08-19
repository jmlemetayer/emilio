//! Finding where the game keeps its saves.
//!
//! Windows calls it the Saved Games folder, and the player is allowed to move it, onto another
//! drive or into a synced folder. Asking Windows where it currently is answers that; building the
//! path out of the user's home directory only answers it while nobody has moved anything.

use std::path::PathBuf;

use windows_sys::Win32::UI::Shell::FOLDERID_SavedGames;

use crate::compat;

/// What the game calls its own folder inside Saved Games.
const GAME: &str = "Diablo II Resurrected";

/// Where the saves usually are, whether or not anything is there yet.
///
/// The directory is not checked for existence here. A path that is right but empty and a path that
/// is wrong look the same at this point, and the difference matters to whoever is about to watch
/// it, not to the guess itself.
pub fn directory() -> Option<PathBuf> {
    let Some(saved_games) = compat::known_folder(FOLDERID_SavedGames) else {
        tracing::warn!("Windows would not say where Saved Games is");
        return None;
    };

    tracing::debug!(path = %saved_games.display(), "Windows put Saved Games here");

    Some(saved_games.join(GAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Windows user has a Saved Games folder, so failing to find it means the call itself is
    /// wrong rather than the machine being unusual.
    #[test]
    fn windows_says_where_saved_games_is() {
        let found = compat::known_folder(FOLDERID_SavedGames).expect("Windows should know");

        assert!(found.is_absolute(), "{}", found.display());
        assert!(found.ends_with("Saved Games"), "{}", found.display());
    }

    /// The game's own folder sits inside it, whether or not it has been created yet.
    #[test]
    fn the_guess_names_the_game_inside_saved_games() {
        let guess = directory().expect("there should be a guess");

        assert!(guess.ends_with(GAME), "{}", guess.display());
    }
}
