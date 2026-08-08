//! Working out what the game was doing when it wrote to disk.
//!
//! The sensors report that files moved. This turns that into what the movement meant: the player
//! saved, picked something up, or left a game. It stops there. Whether leaving a game ends a *run*
//! is a question about runs, and belongs to whatever is counting them, not here.
//!
//! The rules below were not reasoned out; they were found by watching a real game write to a real
//! directory, and several of the obvious-looking ones turned out to be wrong. Where a rule looks
//! arbitrary, the comment says what was observed.
//!
//! Nothing in here owns a clock or opens a file. Times arrive as arguments and contents arrive
//! through [`Fingerprints`], so the whole classifier can be driven through a made-up sequence of
//! events in a unit test, with no game and no filesystem. [`stream`] is the part that supplies the
//! real ones.

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

use burst::{Burst, FileKind, SizeChange};
pub use fingerprint::{FileFingerprints, Fingerprint, Fingerprints};

/// How long the writes belonging to one action are allowed to be spread over.
///
/// Long enough to gather a save that touches several files, short enough that two separate actions
/// do not merge into one.
pub const BURST_WINDOW: Duration = Duration::from_millis(300);

/// How recently the game must have saved for its exit to count as deliberate.
const CLEAN_EXIT_WINDOW: Duration = Duration::from_secs(3);

/// What the game was doing when it wrote.
///
/// Everything that comes from a save file names the character it belongs to, taken from the
/// filename. The two that come from the process going away do not: nothing was written, so there
/// is nobody to name, and a consumer that wants to know whose game just ended already knows from
/// the last event that did name one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveEvent {
    /// A save file was rewritten without its contents changing.
    ///
    /// The menu does this on a mouse click, including the click that starts a game, which makes it
    /// a hint that a game may be starting, and nothing stronger. Starting a game from the keyboard
    /// produces no write at all, so an absent `Touched` means nothing either.
    Touched {
        /// Whose save was rewritten.
        character: String,
    },

    /// A game was entered.
    ///
    /// Inferred from a map being written, which is a hint rather than a guarantee: a full
    /// system-wide trace found that entering a game writes nothing by itself.
    Entered {
        /// Who entered.
        character: String,
    },

    /// A game was left, whether back to the menu or by quitting.
    ///
    /// The settings and control files are written when leaving and at no other time, which makes
    /// this the one boundary that can be trusted.
    Left {
        /// Who left.
        character: String,
    },

    /// The game saved during play.
    Saved {
        /// Who saved.
        character: String,

        /// How much the save grew or shrank, in bytes.
        ///
        /// Only the inventory changes the size of a save: experience, level and gold sit in
        /// fixed-width fields. A non-zero delta therefore means something was gained or lost, and
        /// is worth re-reading the save for.
        ///
        /// Zero does not mean the opposite. Picking one item up while dropping another of the same
        /// size cancels out, and an item merely moving within the inventory never changes the size
        /// at all. Treat this as a cheap hint, never as an answer.
        size_delta: i64,
    },

    /// The game closed, having saved on the way out.
    QuitCleanly,

    /// The game vanished without saving first: a crash, or being killed.
    Crashed,
}

/// Turns raw observations into [`SaveEvent`]s.
///
/// Feed it every [`OsEvent`] through [`observe`](Self::observe), and call
/// [`flush`](Self::flush) once [`deadline`](Self::deadline) has passed.
#[derive(Debug, Default)]
pub struct Classifier {
    /// The writes gathered so far, and when the most recent of them arrived.
    ///
    /// The window runs from the *last* write rather than the first, so a slow save that dribbles
    /// out over half a second is still gathered as one thing.
    pending: Option<(Instant, Burst)>,

    /// The last known contents of each save file, by filename.
    known: HashMap<OsString, Fingerprint>,

    /// When the game last actually saved. Separates quitting from crashing.
    last_save: Option<Instant>,
}

impl Classifier {
    /// A classifier that knows nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what the save files look like before anything has happened.
    ///
    /// Without this the first real save cannot be told from a touch, because there is nothing to
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
    /// File writes are gathered rather than answered, so this usually reports nothing; the answer
    /// comes from [`flush`](Self::flush). A process disappearing is answered immediately, because
    /// nothing more is coming.
    pub fn observe(
        &mut self,
        event: &OsEvent,
        at: Instant,
        fingerprints: &mut impl Fingerprints,
    ) -> Option<SaveEvent> {
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

            OsEvent::ProcessStopped(_) => Some(self.exited(at)),

            OsEvent::ProcessStarted(_) => None,
        }
    }

    /// Judges the gathered writes, if their window has closed.
    ///
    /// Safe to call at any time: it reports nothing unless there is a burst whose deadline has
    /// passed.
    pub fn flush(&mut self, now: Instant) -> Option<SaveEvent> {
        if self.deadline().is_some_and(|deadline| now < deadline) {
            return None;
        }

        let (_, burst) = self.pending.take()?;

        self.judge(&burst, now)
    }

    /// Folds one written path into the burst being gathered.
    fn gather(&mut self, path: &Path, at: Instant, fingerprints: &mut impl Fingerprints) {
        let kind = FileKind::of(path);

        let (character, change) = match kind {
            FileKind::Character => (character_of(path), self.change_to(path, fingerprints)),
            _ => (None, None),
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
        let current = fingerprints.fingerprint(path)?;

        // A file we have never seen counts as changed. It is either a character that has just been
        // created, or one we failed to read while priming; calling that "unchanged" would hide a
        // real save, which is the worse of the two mistakes.
        let delta = match self.known.insert(name, current) {
            Some(previous) if previous.hash == current.hash => return None,
            Some(previous) => current.size as i64 - previous.size as i64,
            None => 0,
        };

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

    /// Decides whether the game closing was deliberate.
    fn exited(&mut self, at: Instant) -> SaveEvent {
        // Quitting writes a save on the way out; a crash does not get the chance. Anything else
        // still gathered is abandoned: it belongs to a game that no longer exists.
        self.pending = None;

        let saved_just_now = self
            .last_save
            .is_some_and(|last| at.duration_since(last) < CLEAN_EXIT_WINDOW);

        if saved_just_now {
            SaveEvent::QuitCleanly
        } else {
            SaveEvent::Crashed
        }
    }

    /// The rules, applied to a finished burst.
    fn judge(&mut self, burst: &Burst, now: Instant) -> Option<SaveEvent> {
        // Nothing to say about a burst that never touched a save: whatever else was written, it
        // belongs to no character and marks no boundary.
        let character = burst.character.clone()?;

        if !burst.character_changed {
            // Rewritten but identical. Real saves always move the contents, because the header
            // carries a checksum and a timestamp, so an unchanged file was never a save.
            return Some(SaveEvent::Touched { character });
        }

        self.last_save = Some(now);

        if burst.leaving {
            return Some(SaveEvent::Left { character });
        }

        // A map written alongside a save, with nothing that says "leaving", means a game was
        // created rather than ended. Checked after `leaving` because both files appear together on
        // the way out, and leaving is the reliable half.
        if burst.map {
            return Some(SaveEvent::Entered { character });
        }

        // Neither entering nor leaving, so it happened during play. Whether the size moved is
        // reported rather than judged: what a given delta means is a question about inventories,
        // and answering it needs the save parsed, not measured.
        Some(SaveEvent::Saved {
            character,
            size_delta: burst.size_delta,
        })
    }
}

/// The character a save file belongs to.
///
/// The game names a save after its character, so the filename without its extension is the name,
/// and it is the only place the name is available without parsing the save itself.
fn character_of(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_string_lossy().into_owned())
}
