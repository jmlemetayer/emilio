//! Wiring the classifier and the player to the tracker.
//!
//! Everything that decides anything is elsewhere and takes its times as arguments. This owns the
//! clock, drives the classifier's burst window, and passes what comes out to the tracker, along
//! with whatever the player asked for by hand.

use std::path::{Path, PathBuf};
use std::time::Instant;

use d2r::classifier::{Classifier, FileFingerprints, GameEvent};
use d2r::sensing::OsEvent;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant as Deadline, sleep_until};

use super::{Input, Run, State, Tracker};
use crate::hotkeys::Intent;

/// Something worth telling the rest of the application about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// A run finished.
    Finished(Run),

    /// The player moved somewhere else.
    Moved(State),

    /// A run's clock started, or started again.
    ///
    /// Not the same as entering a game. A run is timed from the end of the previous one, so this
    /// arrives while the player is still in the menu deciding what to do next.
    Started {
        /// Which run of the session it is.
        index: u32,

        /// Who it is for, when anything has said.
        character: Option<String>,
    },

    /// The game saved during a run.
    ///
    /// Forwarded for its own sake, for item tracking later. Only while a run is in progress: a save
    /// with nothing to attribute it to is not worth passing on.
    Saved {
        /// Who saved.
        character: String,

        /// How much the save grew or shrank, in bytes.
        size_delta: i64,
    },
}

/// Follows the game until the sensors stop, reporting anything that changes.
///
/// Sending is best-effort: a subscriber that falls behind misses updates rather than holding the
/// tracker up, and one that goes away entirely does not stop the tracking.
pub async fn track(
    mut events: UnboundedReceiver<OsEvent>,
    mut intents: UnboundedReceiver<Intent>,
    directory: &Path,
    updates: broadcast::Sender<Update>,
) {
    let mut fingerprints = FileFingerprints;
    let mut classifier = Classifier::new();
    let mut tracker = Tracker::new();
    let mut listening = true;

    classifier.prime(existing_files(directory), &mut fingerprints);

    loop {
        let deadline = classifier.deadline();

        let input = tokio::select! {
            // Prefer draining events: a write arriving now belongs to the burst being gathered,
            // and judging that burst early would split one action into two.
            biased;

            event = events.recv() => match event {
                Some(event) => classifier
                    .observe(&event, Instant::now(), &mut fingerprints)
                    .map(Input::Game),
                None => return,
            },

            intent = intents.recv(), if listening => match intent {
                Some(intent) => Some(asked(intent)),
                None => {
                    tracing::warn!("the hotkeys have stopped, so nothing can be said by hand now");
                    listening = false;
                    None
                }
            },

            () = wait_until(deadline), if deadline.is_some() => {
                classifier.flush(Instant::now()).map(Input::Game)
            }
        };

        if let Some(input) = input {
            apply(&mut tracker, input, &updates);
        }
    }
}

/// What the tracker calls the thing the player pressed.
fn asked(intent: Intent) -> Input {
    match intent {
        Intent::Start => Input::StartRequested,
        Intent::Stop => Input::StopRequested,
        Intent::Pause => Input::PauseRequested,
    }
}

/// Hands one input to the tracker and reports whatever it changed.
fn apply(tracker: &mut Tracker, input: Input, updates: &broadcast::Sender<Update>) {
    // Taken before the tracker sees it: a save arriving as a run ends belongs to the run that is
    // ending, not the one about to start.
    let in_run = tracker.state().in_run();
    let saved = match &input {
        Input::Game(GameEvent::Saved {
            character,
            size_delta,
        }) if in_run => Some(Update::Saved {
            character: character.clone(),
            size_delta: *size_delta,
        }),
        _ => None,
    };

    let runs_before = tracker.runs().len();
    let clock_before = tracker.started();

    let moved = tracker.observe(&input, Instant::now());

    if let Some(saved) = saved {
        let _ = updates.send(saved);
    }

    // Reported in the order they happen: one run ends, the player is somewhere else, and the clock
    // of the next one is already going.
    if tracker.runs().len() > runs_before
        && let Some(run) = tracker.runs().last()
    {
        let _ = updates.send(Update::Finished(run.clone()));
    }

    if let Some(state) = moved {
        let _ = updates.send(Update::Moved(state));
    }

    if tracker.started() != clock_before
        && let Some(index) = tracker.state().index()
    {
        let _ = updates.send(Update::Started {
            index,
            character: tracker.character().map(str::to_owned),
        });
    }
}

/// Sleeps until an instant that is known to be there.
async fn wait_until(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        sleep_until(Deadline::from_std(deadline)).await;
    }
}

/// Lists the files sitting in a directory, so the classifier knows what they looked like first.
fn existing_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        tracing::warn!(
            directory = %directory.display(),
            "could not read the save directory, so the first save may be misread"
        );
        return Vec::new();
    };

    entries.flatten().map(|entry| entry.path()).collect()
}
