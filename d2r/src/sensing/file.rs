//! Watches the save directory for writes.
//!
//! Only the fact of a write is reported here. What a write means (a save, leaving a game, or the
//! menu merely touching a file) depends on which files moved together and on what changed inside
//! them, and that is the classifier's problem.

use std::path::{Path, PathBuf};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::UnboundedSender;

use super::OsEvent;
use crate::Result;

/// A running watch over a directory.
///
/// The watch lasts exactly as long as this handle: dropping it stops the watch, so a caller has to
/// keep it somewhere for as long as it wants events.
pub struct FileWatcher {
    /// Held for its `Drop`, which unregisters the watch.
    _watcher: RecommendedWatcher,
}

/// Starts watching `directory` for writes, reporting into `sender`.
///
/// The directory is watched shallowly; the game keeps its saves in one flat directory.
pub fn watch(sender: UnboundedSender<OsEvent>, directory: &Path) -> Result<FileWatcher> {
    let mut watcher = notify::recommended_watcher(move |result| match result {
        Ok(event) => {
            for os_event in os_events(&event) {
                if sender.send(os_event).is_err() {
                    return;
                }
            }
        }
        Err(error) => tracing::warn!(%error, "the save directory watch reported an error"),
    })?;

    watcher.watch(directory, RecursiveMode::NonRecursive)?;

    Ok(FileWatcher { _watcher: watcher })
}

/// Translates one notification into the events it is worth reporting.
///
/// A notification can name more than one path, and can describe things we have no use for. Access
/// notifications in particular fire constantly and say nothing about the game having written
/// anything, so they are dropped here rather than burdening every consumer with ignoring them.
fn os_events(event: &notify::Event) -> Vec<OsEvent> {
    let as_event: fn(PathBuf) -> OsEvent = match event.kind {
        EventKind::Create(_) => OsEvent::FileCreated,
        EventKind::Modify(_) => OsEvent::FileModified,
        EventKind::Remove(_) => OsEvent::FileRemoved,
        _ => return Vec::new(),
    };

    event.paths.iter().cloned().map(as_event).collect()
}

#[cfg(test)]
mod tests {
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};

    use super::*;

    fn event(kind: EventKind, paths: &[&str]) -> notify::Event {
        paths.iter().fold(notify::Event::new(kind), |event, path| {
            event.add_path(PathBuf::from(path))
        })
    }

    #[test]
    fn translates_the_kinds_we_care_about() {
        let save = "Vikhyat.d2s";

        assert_eq!(
            os_events(&event(EventKind::Create(CreateKind::File), &[save])),
            vec![OsEvent::FileCreated(PathBuf::from(save))]
        );
        assert_eq!(
            os_events(&event(EventKind::Modify(ModifyKind::Any), &[save])),
            vec![OsEvent::FileModified(PathBuf::from(save))]
        );
        assert_eq!(
            os_events(&event(EventKind::Remove(RemoveKind::File), &[save])),
            vec![OsEvent::FileRemoved(PathBuf::from(save))]
        );
    }

    /// Reading a save is not the game writing one. These arrive in bulk and would drown the
    /// classifier in noise.
    #[test]
    fn ignores_access_notifications() {
        let ignored = event(EventKind::Access(AccessKind::Read), &["Vikhyat.d2s"]);

        assert!(os_events(&ignored).is_empty());
    }

    /// A rename arrives as one notification naming both paths, and the game writes several files
    /// at once when it saves. Reporting only the first would lose half of what happened.
    #[test]
    fn reports_every_path_a_notification_names() {
        let both = event(
            EventKind::Modify(ModifyKind::Any),
            &["Vikhyat.d2s", "Vikhyat.d2i"],
        );

        assert_eq!(
            os_events(&both),
            vec![
                OsEvent::FileModified(PathBuf::from("Vikhyat.d2s")),
                OsEvent::FileModified(PathBuf::from("Vikhyat.d2i")),
            ]
        );
    }

    #[test]
    fn reports_nothing_for_a_notification_naming_no_path() {
        let empty = event(EventKind::Modify(ModifyKind::Any), &[]);

        assert!(os_events(&empty).is_empty());
    }
}
