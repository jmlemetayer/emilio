//! The rules, checked against made-up sequences of events.
//!
//! No game and no filesystem: the classifier takes its times as arguments and its file contents
//! through a trait, so every rule can be exercised directly. Each test is named for the rule it
//! pins down, because a failure here means the classifier disagrees with what was observed of the
//! real game.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::*;
use crate::sensing::OsEvent;

/// Fingerprints straight out of a table, so a test can say "the save changed" without a disk.
#[derive(Default)]
struct FakeFiles(HashMap<PathBuf, Fingerprint>);

impl FakeFiles {
    /// Sets what a file currently looks like. `revision` stands in for its contents.
    fn set(&mut self, name: &str, size: u64, revision: u8) {
        self.0.insert(
            PathBuf::from(name),
            Fingerprint {
                size,
                hash: [revision; 32],
            },
        );
    }
}

impl Fingerprints for FakeFiles {
    fn fingerprint(&mut self, path: &Path) -> Option<Fingerprint> {
        self.0.get(path).copied()
    }
}

const SAVE: &str = "Vikhyat.d2s";

/// A classifier that already knows what the save looked like, as it would after priming.
fn primed() -> (Classifier, FakeFiles, Instant) {
    let mut files = FakeFiles::default();
    files.set(SAVE, 1000, 1);

    let mut classifier = Classifier::new();
    classifier.prime([PathBuf::from(SAVE)], &mut files);

    (classifier, files, Instant::now())
}

/// Writes some files together and judges them, the way the driver would.
fn wrote(
    classifier: &mut Classifier,
    files: &mut FakeFiles,
    paths: &[&str],
    at: Instant,
) -> Option<SaveEvent> {
    for path in paths {
        classifier.observe(&OsEvent::FileModified(PathBuf::from(path)), at, files);
    }

    classifier.flush(at + BURST_WINDOW)
}

#[test]
fn a_rewrite_with_no_change_is_only_a_touch() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Touched)
    );
}

#[test]
fn a_changed_save_of_the_same_size_reports_no_movement() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: 0 })
    );
}

#[test]
fn a_save_that_grew_reports_a_positive_delta() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1015, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: 15 })
    );
}

#[test]
fn a_save_that_shrank_reports_a_negative_delta() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 985, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: -15 })
    );
}

#[test]
fn settings_written_with_a_save_means_the_game_was_left() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, "Settings.json"], now),
        Some(SaveEvent::Left)
    );
}

#[test]
fn a_map_written_with_a_save_means_a_game_was_entered() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, "Vikhyat.ma0"], now),
        Some(SaveEvent::Entered)
    );
}

/// Both files appear together on the way out. Leaving is the half that can be trusted, so it has to
/// win, because reading this as "entered" would invent a game that never happened.
#[test]
fn leaving_wins_when_a_map_and_the_settings_arrive_together() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    assert_eq!(
        wrote(
            &mut classifier,
            &mut files,
            &[SAVE, "Vikhyat.ma0", "Settings.json"],
            now
        ),
        Some(SaveEvent::Left)
    );
}

#[test]
fn writes_that_do_not_touch_a_save_say_nothing() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        wrote(&mut classifier, &mut files, &["Vikhyat.ma0"], now),
        None
    );
}

/// The window runs from the last write, so files dribbling out slowly stay one action.
#[test]
fn a_slow_save_is_still_one_action() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    let nearly_expired = BURST_WINDOW - Duration::from_millis(50);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);
    assert_eq!(classifier.flush(now + nearly_expired), None);

    classifier.observe(
        &OsEvent::FileModified(PathBuf::from("Settings.json")),
        now + nearly_expired,
        &mut files,
    );

    // Would already have fired had the window run from the first write.
    assert_eq!(classifier.flush(now + BURST_WINDOW), None);
    assert_eq!(
        classifier.flush(now + nearly_expired + BURST_WINDOW),
        Some(SaveEvent::Left)
    );
}

#[test]
fn nothing_is_judged_before_the_window_closes() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);

    assert_eq!(classifier.flush(now), None);
    assert_eq!(
        classifier.flush(now + BURST_WINDOW),
        Some(SaveEvent::Saved { size_delta: 0 })
    );
}

#[test]
fn two_separate_actions_are_judged_separately() {
    let (mut classifier, mut files, now) = primed();

    files.set(SAVE, 1015, 2);
    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: 15 })
    );

    let later = now + Duration::from_secs(60);
    files.set(SAVE, 1015, 3);
    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, "Settings.json"], later),
        Some(SaveEvent::Left)
    );
}

#[test]
fn closing_just_after_a_save_is_a_deliberate_quit() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    wrote(&mut classifier, &mut files, &[SAVE, "Settings.json"], now);

    let shutdown = now + BURST_WINDOW + Duration::from_millis(200);
    assert_eq!(
        classifier.observe(&OsEvent::ProcessStopped(1234), shutdown, &mut files),
        Some(SaveEvent::QuitCleanly)
    );
}

#[test]
fn closing_long_after_the_last_save_is_a_crash() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    wrote(&mut classifier, &mut files, &[SAVE], now);

    let much_later = now + Duration::from_secs(300);
    assert_eq!(
        classifier.observe(&OsEvent::ProcessStopped(1234), much_later, &mut files),
        Some(SaveEvent::Crashed)
    );
}

#[test]
fn closing_without_ever_saving_is_a_crash() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        classifier.observe(&OsEvent::ProcessStopped(1234), now, &mut files),
        Some(SaveEvent::Crashed)
    );
}

/// Without a baseline the first real save is indistinguishable from a rewrite, so priming is what
/// stops a genuine save being reported as a touch.
#[test]
fn priming_is_what_makes_the_first_save_recognisable() {
    let mut files = FakeFiles::default();
    files.set(SAVE, 1000, 1);
    let now = Instant::now();

    // Never seen before, so it is treated as changed rather than dismissed as a touch.
    let mut unprimed = Classifier::new();
    assert_eq!(
        wrote(&mut unprimed, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: 0 })
    );

    let mut primed = Classifier::new();
    primed.prime([PathBuf::from(SAVE)], &mut files);
    assert_eq!(
        wrote(&mut primed, &mut files, &[SAVE], now),
        Some(SaveEvent::Touched)
    );
}

/// A deleted character must not leave a fingerprint behind: a new character of the same name would
/// compare against it, and its first save would vanish as a touch.
#[test]
fn a_deleted_save_is_forgotten() {
    let (mut classifier, mut files, now) = primed();

    classifier.observe(&OsEvent::FileRemoved(PathBuf::from(SAVE)), now, &mut files);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        Some(SaveEvent::Saved { size_delta: 0 })
    );
}

/// A crash abandons whatever was being gathered; it belongs to a game that is gone.
#[test]
fn a_burst_in_flight_is_dropped_when_the_game_disappears() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 1000, 2);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);
    classifier.observe(&OsEvent::ProcessStopped(1234), now, &mut files);

    assert_eq!(classifier.deadline(), None);
    assert_eq!(classifier.flush(now + BURST_WINDOW), None);
}

#[test]
fn starting_the_game_is_not_a_save_event() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        classifier.observe(&OsEvent::ProcessStarted(1234), now, &mut files),
        None
    );
}
