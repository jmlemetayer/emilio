//! Driving the classifier from a live stream of events.
//!
//! This is the part that owns the awkward things the classifier deliberately does not: the clock,
//! the filesystem, and the waiting. All it does is arrange for [`Classifier`] to be asked the right
//! question at the right moment.

use std::path::Path;
use std::time::Instant;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{Instant as Deadline, sleep_until};

use super::{Classifier, FileFingerprints, SaveEvent};
use crate::sensing::OsEvent;

/// Reads raw events and reports what they meant, until either end of the pipe closes.
///
/// The contents of `directory` are read once at the start, so that the first real save can be told
/// from a file merely being rewritten.
pub async fn classify(
    mut events: UnboundedReceiver<OsEvent>,
    saves: UnboundedSender<SaveEvent>,
    directory: &Path,
) {
    let mut fingerprints = FileFingerprints;
    let mut classifier = Classifier::new();

    classifier.prime(existing_files(directory), &mut fingerprints);

    loop {
        let deadline = classifier.deadline();

        let event = tokio::select! {
            // Prefer draining events: a write arriving now belongs to the burst being gathered, and
            // judging that burst early would split one action into two.
            biased;

            event = events.recv() => match event {
                Some(event) => classifier.observe(&event, Instant::now(), &mut fingerprints),
                None => return,
            },

            () = wait_until(deadline), if deadline.is_some() => classifier.flush(Instant::now()),
        };

        if let Some(event) = event
            && saves.send(event).is_err()
        {
            return;
        }
    }
}

/// Sleeps until an instant that is known to be there.
async fn wait_until(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        sleep_until(Deadline::from_std(deadline)).await;
    }
}

/// Lists the files sitting in a directory, ignoring one that cannot be read.
fn existing_files(directory: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        tracing::warn!(
            directory = %directory.display(),
            "could not read the save directory, so the first save may be misread"
        );
        return Vec::new();
    };

    entries.flatten().map(|entry| entry.path()).collect()
}
