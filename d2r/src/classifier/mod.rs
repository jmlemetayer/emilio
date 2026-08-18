//! Working out what the game was doing when it wrote to disk.
//!
//! The sensors report that files moved and that the game came and went. This turns that into what
//! happened: the game appeared, a save happened during play, a game was left, the game went away.
//! It stops there. Whether leaving a game ends a *run* is a question about runs, and belongs to
//! whatever is counting them.
//!
//! **Only what is certain gets reported.** A signal that is usually right is worse than no signal,
//! because everything downstream learns to trust it. Entering a game is the tempting one and is
//! left out: nothing is written at the moment it happens, the map write that would stand in for it
//! means going deeper into an area, and the click that starts a game is missing from whole
//! sessions.
//!
//! Nothing here owns a clock or opens a file. Times arrive as arguments and contents arrive through
//! [`Fingerprints`], so every rule can be driven through a made-up sequence of events in a unit
//! test, with no game and no filesystem. [`stream`] is the part that supplies the real ones.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::sensing::OsEvent;

mod burst;
pub mod fingerprint;
pub mod stream;

#[cfg(test)]
mod tests;

use burst::{Burst, FileKind, SizeChange, character_of};
pub use fingerprint::{FileFingerprints, Fingerprint, Fingerprints};

/// How long the writes belonging to one action are allowed to be spread over.
///
/// Long enough to gather a save that touches several files, short enough that two separate actions
/// do not merge into one.
pub const BURST_WINDOW: Duration = Duration::from_millis(300);

/// What the game did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    /// The game appeared. Reported for one already running when watching starts, too.
    Started,

    /// A character left a game, whether back to the menu or by quitting.
    ///
    /// The one boundary that can be trusted: a character's own controls file is written when that
    /// character leaves and at no other time observed.
    Left {
        /// Who left.
        character: String,
    },

    /// A save happened during play.
    Saved {
        /// Who saved.
        character: String,

        /// How much the save grew or shrank, in bytes.
        ///
        /// Only the inventory changes the size of a save; experience, level and gold sit in
        /// fixed-width fields. A non-zero delta therefore means something was gained or lost.
        ///
        /// It is a hint rather than an answer. Zero does not mean nothing happened: a simultaneous
        /// pickup and drop cancels out, and an item merely moving within the inventory changes
        /// nothing. Nor does it account for the shared stash, so an item moved there reads as a
        /// loss.
        size_delta: i64,
    },

    /// The game went away, whether quit or crashed.
    ///
    /// The two are not told apart. Doing so needs save-recency timing that nothing yet consumes,
    /// and a wrong answer would be worse than no answer.
    Quit,
}

/// Turns raw observations into [`GameEvent`]s.
///
/// Feed it every [`OsEvent`] through [`observe`](Self::observe), and call [`flush`](Self::flush)
/// once [`deadline`](Self::deadline) has passed.
#[derive(Debug, Default)]
pub struct Classifier {
    /// The writes gathered so far, and when the most recent of them arrived.
    ///
    /// The window runs from the *last* write rather than the first, so a slow save that dribbles
    /// out over half a second is still gathered as one thing.
    pending: Option<(Instant, Burst)>,

    /// The last known contents of each save file, by filename.
    known: HashMap<OsString, Fingerprint>,
}

impl Classifier {
    /// A classifier that knows nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what the save files look like before anything has happened.
    ///
    /// Without this the first real save cannot be told from a rewrite, because there is nothing to
    /// compare it against. Paths that are not save files are ignored, so a caller can hand over a
    /// whole directory listing.
    pub fn prime<I>(&mut self, paths: I, fingerprints: &mut impl Fingerprints)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        for path in paths {
            if FileKind::of(&path) == FileKind::Character {
                self.remember(&path, fingerprints);
            }
        }
    }

    /// When the burst being gathered should be judged, if one is being gathered.
    ///
    /// The caller is expected to wake up at this point and call [`flush`](Self::flush). Nothing
    /// happens on its own; this type has no timer.
    pub fn deadline(&self) -> Option<Instant> {
        self.pending
            .as_ref()
            .map(|(last_write, _)| *last_write + BURST_WINDOW)
    }

    /// Takes in one observation, at the time it was observed.
    ///
    /// File writes are gathered rather than answered, so those report nothing; the answer comes
    /// from [`flush`](Self::flush). The game coming and going is answered at once, because nothing
    /// more is coming.
    pub fn observe(
        &mut self,
        event: &OsEvent,
        at: Instant,
        fingerprints: &mut impl Fingerprints,
    ) -> Option<GameEvent> {
        match event {
            OsEvent::FileCreated(path) | OsEvent::FileModified(path) => {
                self.gather(path, at, fingerprints);
                None
            }

            // A deleted save tells us nothing about play, but keeping a fingerprint for a file that
            // no longer exists would make a later file of the same name look unchanged.
            OsEvent::FileRemoved(path) => {
                self.forget(path);
                None
            }

            OsEvent::ProcessStarted(_) => Some(GameEvent::Started),

            OsEvent::ProcessStopped(_) => {
                // Whatever was being gathered belongs to a game that no longer exists.
                self.pending = None;
                Some(GameEvent::Quit)
            }
        }
    }

    /// Judges the gathered writes, if their window has closed.
    ///
    /// Safe to call at any time: it reports nothing unless there is a burst whose deadline has
    /// passed.
    pub fn flush(&mut self, now: Instant) -> Option<GameEvent> {
        if self.deadline().is_some_and(|deadline| now < deadline) {
            return None;
        }

        let (_, burst) = self.pending.take()?;
        let event = judge(&burst);

        tracing::debug!(
            character = ?burst.character,
            changed = burst.character_changed,
            size_delta = burst.size_delta,
            controls_for = ?burst.controls_for,
            ?event,
            "judged a burst"
        );

        event
    }

    /// Folds one written path into the burst being gathered.
    fn gather(&mut self, path: &Path, at: Instant, fingerprints: &mut impl Fingerprints) {
        let kind = FileKind::of(path);

        let (character, change) = match kind {
            FileKind::Character => (character_of(path), self.change_to(path, fingerprints)),

            // Controls carry no contents worth reading, only whose they are.
            FileKind::Controls => (character_of(path), None),

            FileKind::Other => (None, None),
        };

        let (last_write, burst) = self.pending.get_or_insert_with(|| (at, Burst::default()));

        *last_write = at;
        burst.add(kind, character.as_deref(), change);
    }

    /// Fingerprints a save and reports how its size moved, if its contents changed at all.
    fn change_to(
        &mut self,
        path: &Path,
        fingerprints: &mut impl Fingerprints,
    ) -> Option<SizeChange> {
        let name = path.file_name()?.to_owned();

        let Some(current) = fingerprints.fingerprint(path) else {
            // The save moved and we could not see how, which is indistinguishable from it not
            // having moved at all. Whatever really happened is lost.
            tracing::warn!(
                path = %path.display(),
                "a save changed but could not be read, so it will be ignored"
            );

            return None;
        };

        // A file we have never seen counts as changed. It is either a character that has just been
        // created, or one we failed to read while priming; calling that "unchanged" would hide a
        // real save, which is the worse of the two mistakes.
        let delta = match self.known.insert(name, current) {
            Some(previous) if previous.hash == current.hash => {
                tracing::trace!(path = %path.display(), "rewritten with identical contents");
                return None;
            }
            Some(previous) => current.size as i64 - previous.size as i64,
            None => {
                tracing::debug!(path = %path.display(), "never seen before, so counted as changed");
                0
            }
        };

        tracing::trace!(path = %path.display(), delta, "contents changed");

        Some(SizeChange { delta })
    }

    /// Fingerprints a save and remembers it, without judging anything.
    fn remember(&mut self, path: &Path, fingerprints: &mut impl Fingerprints) {
        if let (Some(name), Some(fingerprint)) = (path.file_name(), fingerprints.fingerprint(path))
        {
            self.known.insert(name.to_owned(), fingerprint);
        }
    }

    /// Drops what we knew about a file that no longer exists.
    fn forget(&mut self, path: &Path) {
        if let Some(name) = path.file_name() {
            self.known.remove(name);
        }
    }
}

/// The rules, applied to a finished burst.
///
/// A real save always moves the file contents, because the header carries a checksum and a
/// timestamp. So a byte-identical rewrite was never a save, whatever else was written alongside it.
/// This also holds when nothing about the character changed: a save where only the shared stash
/// moved still altered the character file, which is why watching it alone misses nothing.
fn judge(burst: &Burst) -> Option<GameEvent> {
    if !burst.character_changed {
        return None;
    }

    // Contents cannot change without a save having been written, so there is a name by here.
    let character = burst.character.clone()?;

    if burst.is_leaving() {
        return Some(GameEvent::Left { character });
    }

    Some(GameEvent::Saved {
        character,
        size_delta: burst.size_delta,
    })
}
