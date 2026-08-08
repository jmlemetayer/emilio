//! Watches for the game process starting and stopping.
//!
//! This is a poller rather than a subscriber. WMI was the alternative, and would let Windows push
//! process notifications instead of us asking for them; it was discarded because a WMI connection
//! is `!Send` and so cannot be owned by an async task, which would cost a dedicated thread and a
//! runtime of its own. Polling was measured as sufficient during the save-event spike, keeps the
//! crate portable, and a quarter of a second of latency does not matter for an event that only
//! decides whether the game is up.
//!
//! Asking the operating system for the process list blocks, so the poll runs on a thread of its
//! own rather than on the async runtime.

use std::collections::HashSet;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::mpsc::UnboundedSender;

use super::OsEvent;
use crate::Result;

/// The name the game runs under.
const PROCESS_NAME: &str = "D2R.exe";

/// How often to look, unless the caller asks for something else.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// A running watch over the game process.
///
/// The watch stops on its own once the receiving end of the channel is dropped; it notices within
/// one poll interval. Holding on to this handle is only necessary to [`join`](Self::join) it.
pub struct ProcessWatcher {
    handle: JoinHandle<()>,
}

impl ProcessWatcher {
    /// Blocks until the watch has stopped.
    pub fn join(self) -> thread::Result<()> {
        self.handle.join()
    }
}

/// Starts watching for the game process, reporting into `sender`.
///
/// A game already running when the watch starts is reported as having started, so a caller does
/// not have to look for itself.
pub fn watch(sender: UnboundedSender<OsEvent>, interval: Duration) -> Result<ProcessWatcher> {
    let handle = thread::Builder::new()
        .name("d2r-process".to_owned())
        .spawn(move || run(&sender, interval))?;

    Ok(ProcessWatcher { handle })
}

/// The poll loop, until the receiving end goes away.
fn run(sender: &UnboundedSender<OsEvent>, interval: Duration) {
    let mut system = System::new();
    let mut previous = HashSet::new();

    while !sender.is_closed() {
        let current = running_pids(&mut system);

        for event in diff(&previous, &current) {
            if sender.send(event).is_err() {
                return;
            }
        }

        previous = current;
        thread::sleep(interval);
    }
}

/// Asks the operating system which game processes exist right now.
fn running_pids(system: &mut System) -> HashSet<u32> {
    system.refresh_processes(ProcessesToUpdate::All, true);

    system
        .processes_by_exact_name(PROCESS_NAME.as_ref())
        .map(|process| process.pid().as_u32())
        .collect()
}

/// Turns the difference between two observations into the events it implies.
fn diff(previous: &HashSet<u32>, current: &HashSet<u32>) -> Vec<OsEvent> {
    let started = current
        .difference(previous)
        .copied()
        .map(OsEvent::ProcessStarted);

    let stopped = previous
        .difference(current)
        .copied()
        .map(OsEvent::ProcessStopped);

    started.chain(stopped).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pids(pids: &[u32]) -> HashSet<u32> {
        pids.iter().copied().collect()
    }

    #[test]
    fn reports_nothing_when_nothing_changed() {
        assert!(diff(&pids(&[42]), &pids(&[42])).is_empty());
        assert!(diff(&pids(&[]), &pids(&[])).is_empty());
    }

    #[test]
    fn reports_a_process_that_appeared() {
        assert_eq!(
            diff(&pids(&[]), &pids(&[42])),
            vec![OsEvent::ProcessStarted(42)]
        );
    }

    #[test]
    fn reports_a_process_that_went_away() {
        assert_eq!(
            diff(&pids(&[42]), &pids(&[])),
            vec![OsEvent::ProcessStopped(42)]
        );
    }

    /// A game already running before the watch starts still has to be reported, which is what
    /// makes the first poll a difference against nothing rather than a silent baseline.
    #[test]
    fn reports_a_process_that_was_already_running() {
        assert_eq!(
            diff(&HashSet::new(), &pids(&[42])),
            vec![OsEvent::ProcessStarted(42)]
        );
    }

    /// Restarting the game between two polls looks like one process leaving and another arriving,
    /// and both halves have to survive.
    #[test]
    fn reports_both_halves_of_a_restart() {
        let events = diff(&pids(&[42]), &pids(&[43]));

        assert_eq!(events.len(), 2);
        assert!(events.contains(&OsEvent::ProcessStarted(43)));
        assert!(events.contains(&OsEvent::ProcessStopped(42)));
    }
}
