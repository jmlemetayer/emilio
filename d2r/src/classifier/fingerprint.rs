//! Telling whether a file's contents actually changed.
//!
//! The game rewrites a save file for reasons that have nothing to do with the player saving, so
//! "the file was written" and "the file changed" are different questions. Answering the second one
//! needs the contents, which is the only place in the classifier that touches the filesystem, and
//! the reason it is behind a trait, so the rest can be tested without one.

use std::fs;
use std::path::Path;
use std::time::Duration;

/// Enough of a file's contents to recognise a change in them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    /// The size in bytes. Kept alongside the hash because the *direction* of a size change is
    /// meaningful on its own: a save that grew is a save that gained an item.
    pub size: u64,

    /// A hash of the whole file.
    pub hash: [u8; 32],
}

/// Somewhere fingerprints come from.
///
/// The classifier decides which files are worth fingerprinting and asks through this; it never
/// opens a file itself. Tests supply their own implementation and never go near a disk.
pub trait Fingerprints {
    /// Fingerprints a file, or reports `None` if it could not be read.
    fn fingerprint(&mut self, path: &Path) -> Option<Fingerprint>;
}

/// How long to wait before a second attempt at reading a save.
const RETRY_DELAY: Duration = Duration::from_millis(25);

/// Reads fingerprints from the real filesystem.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileFingerprints;

impl Fingerprints for FileFingerprints {
    /// Reads and hashes the file, retrying once.
    ///
    /// We are reading a file the game is in the middle of writing, so an attempt can lose a race
    /// and fail. One retry was enough across the read-safety spike, which saw no partial reads at
    /// all; treating a second failure as "unknown" is safer than blocking the sensing loop for a
    /// file that may well be gone.
    fn fingerprint(&mut self, path: &Path) -> Option<Fingerprint> {
        for attempt in 0..2 {
            match fs::read(path) {
                Ok(contents) => {
                    let fingerprint = Fingerprint {
                        size: contents.len() as u64,
                        hash: *blake3::hash(&contents).as_bytes(),
                    };

                    tracing::trace!(
                        path = %path.display(),
                        size = fingerprint.size,
                        attempt,
                        "read the file"
                    );

                    return Some(fingerprint);
                }
                Err(error) => {
                    if attempt == 0 {
                        tracing::trace!(path = %path.display(), %error, "retrying the read");
                        std::thread::sleep(RETRY_DELAY);
                    } else {
                        // Worth a warning rather than a note: a save we cannot read is a save we
                        // cannot tell from an untouched one, so this silently costs an event.
                        tracing::warn!(
                            path = %path.display(),
                            %error,
                            "could not read the file, so any change to it will be missed"
                        );
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real reader has to survive a file that is not there, because by the time we look at a
    /// deletion the file is already gone.
    #[test]
    fn reports_nothing_for_a_file_that_does_not_exist() {
        let mut fingerprints = FileFingerprints;
        let missing = Path::new("no-such-directory-8f3a/no-such-file.d2s");

        assert_eq!(fingerprints.fingerprint(missing), None);
    }
}
