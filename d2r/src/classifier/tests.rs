//! The rules, checked against made-up sequences of events.
//!
//! No game and no filesystem: the classifier takes its times as arguments and its file contents
//! through a trait, so every rule can be exercised directly. Several tests are named after live
//! captures, because the rules are empirical facts about a closed-source game and a test written
//! alongside the code inherits whatever the code assumed.

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

const CHARACTER: &str = "Kate";
const SAVE: &str = "Kate.d2s";
const CONTROLS: &str = "Kate.ctl";

/// A classifier that already knows what the save looked like, as it would after priming.
fn primed() -> (Classifier, FakeFiles, Instant) {
    let mut files = FakeFiles::default();
    files.set(SAVE, 3429, 1);

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
) -> Option<GameEvent> {
    for path in paths {
        classifier.observe(&OsEvent::FileModified(PathBuf::from(path)), at, files);
    }

    classifier.flush(at + BURST_WINDOW)
}

fn left(character: &str) -> Option<GameEvent> {
    Some(GameEvent::Left {
        character: character.to_owned(),
    })
}

fn saved(character: &str, size_delta: i64) -> Option<GameEvent> {
    Some(GameEvent::Saved {
        character: character.to_owned(),
        size_delta,
    })
}

#[test]
fn a_rewrite_with_no_change_says_nothing() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(wrote(&mut classifier, &mut files, &[SAVE], now), None);
}

/// A save with its own controls file is the one boundary that can be trusted.
#[test]
fn a_save_with_its_own_controls_means_the_game_was_left() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, CONTROLS], now),
        left(CHARACTER)
    );
}

#[test]
fn a_changed_save_on_its_own_is_a_save_during_play() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        saved(CHARACTER, 0)
    );
}

#[test]
fn a_save_that_grew_reports_a_positive_delta() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3524, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        saved(CHARACTER, 95)
    );
}

#[test]
fn a_save_that_shrank_reports_a_negative_delta() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3334, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        saved(CHARACTER, -95)
    );
}

/// Captured live: changing an audio setting rewrites the settings file, changing a keybind writes
/// the keybind profile, and both happen during play. Counting either as a boundary would end a run
/// in the middle of one whenever the change landed near an autosave.
#[test]
fn settings_and_keybinds_written_with_a_save_are_not_leaving() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    assert_eq!(
        wrote(
            &mut classifier,
            &mut files,
            &[SAVE, "Settings.json", "Custom.key", "lootfilter.json"],
            now
        ),
        saved(CHARACTER, 0)
    );
}

/// The controls file has to belong to the character whose save moved.
#[test]
fn another_characters_controls_are_not_leaving() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, "Daphne.ctl"], now),
        saved(CHARACTER, 0)
    );
}

/// Captured live: the controls file is rewritten byte-identically on the way out. Requiring its
/// contents to change would mean leaving a game is never recognised at all.
#[test]
fn leaving_does_not_need_the_controls_file_to_change() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);
    files.set(CONTROLS, 900, 1);

    // Primed with the same contents it will be written with.
    classifier.prime([PathBuf::from(CONTROLS)], &mut files);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, CONTROLS], now),
        left(CHARACTER)
    );
}

/// Maps record going deeper into an area rather than entering a game, and a whole session can pass
/// without one. They say nothing either way.
#[test]
fn maps_say_nothing() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(wrote(&mut classifier, &mut files, &["Kate.ma0"], now), None);

    files.set(SAVE, 3429, 2);
    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, "Kate.ma0"], now),
        saved(CHARACTER, 0)
    );
}

#[test]
fn writes_that_carry_no_signal_say_nothing() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        wrote(&mut classifier, &mut files, &["something-else.txt"], now),
        None
    );
}

/// The window runs from the last write, so files dribbling out slowly stay one action.
#[test]
fn a_slow_save_is_still_one_action() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    let nearly_expired = BURST_WINDOW - Duration::from_millis(50);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);
    assert_eq!(classifier.flush(now + nearly_expired), None);

    classifier.observe(
        &OsEvent::FileModified(PathBuf::from(CONTROLS)),
        now + nearly_expired,
        &mut files,
    );

    // Would already have fired had the window run from the first write.
    assert_eq!(classifier.flush(now + BURST_WINDOW), None);
    assert_eq!(
        classifier.flush(now + nearly_expired + BURST_WINDOW),
        left(CHARACTER)
    );
}

#[test]
fn nothing_is_judged_before_the_window_closes() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);

    assert_eq!(classifier.flush(now), None);
    assert_eq!(classifier.flush(now + BURST_WINDOW), saved(CHARACTER, 0));
}

#[test]
fn two_separate_actions_are_judged_separately() {
    let (mut classifier, mut files, now) = primed();

    files.set(SAVE, 3524, 2);
    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        saved(CHARACTER, 95)
    );

    let later = now + Duration::from_secs(60);
    files.set(SAVE, 3524, 3);
    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE, CONTROLS], later),
        left(CHARACTER)
    );
}

#[test]
fn the_game_appearing_and_going_away_are_reported_at_once() {
    let (mut classifier, mut files, now) = primed();

    assert_eq!(
        classifier.observe(&OsEvent::ProcessStarted(1234), now, &mut files),
        Some(GameEvent::Started)
    );
    assert_eq!(
        classifier.observe(&OsEvent::ProcessStopped(1234), now, &mut files),
        Some(GameEvent::Quit)
    );
}

/// A burst in flight belongs to a game that is gone.
#[test]
fn the_game_going_away_abandons_a_burst_in_flight() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3429, 2);

    classifier.observe(&OsEvent::FileModified(PathBuf::from(SAVE)), now, &mut files);
    classifier.observe(&OsEvent::ProcessStopped(1234), now, &mut files);

    assert_eq!(classifier.deadline(), None);
    assert_eq!(classifier.flush(now + BURST_WINDOW), None);
}

/// Without a baseline the first real save is indistinguishable from a rewrite, so priming is what
/// stops a genuine save being ignored.
#[test]
fn priming_is_what_makes_the_first_save_recognisable() {
    let mut files = FakeFiles::default();
    files.set(SAVE, 3429, 1);
    let now = Instant::now();

    // Never seen before, so it is treated as changed rather than dismissed.
    let mut unprimed = Classifier::new();
    assert_eq!(
        wrote(&mut unprimed, &mut files, &[SAVE], now),
        saved(CHARACTER, 0)
    );

    let mut primed = Classifier::new();
    primed.prime([PathBuf::from(SAVE)], &mut files);
    assert_eq!(wrote(&mut primed, &mut files, &[SAVE], now), None);
}

/// A deleted character must not leave a fingerprint behind: a new character of the same name would
/// compare against it, and its first save would vanish.
#[test]
fn a_deleted_save_is_forgotten() {
    let (mut classifier, mut files, now) = primed();

    classifier.observe(&OsEvent::FileRemoved(PathBuf::from(SAVE)), now, &mut files);

    assert_eq!(
        wrote(&mut classifier, &mut files, &[SAVE], now),
        saved(CHARACTER, 0)
    );
}

/// Captured live: sorting only the shared stash still moved the character save's hash, its size
/// unchanged, because the header carries a checksum and a timestamp. So no save is missed by
/// watching the character file alone, even when nothing about the character changed.
#[test]
fn a_save_where_only_the_stash_changed_is_still_a_save() {
    let (mut classifier, mut files, now) = primed();

    // Same size, different contents, exactly as captured.
    files.set(SAVE, 3429, 2);

    assert_eq!(
        wrote(
            &mut classifier,
            &mut files,
            &[SAVE, "ModernSharedStashSoftCoreV2.d2i"],
            now
        ),
        saved(CHARACTER, 0)
    );
}

/// The exit burst from a live capture, replayed: the save and stash both changed with no net size
/// change, the controls file was rewritten four times unchanged, and the settings twice.
#[test]
fn the_captured_exit_burst_is_read_as_leaving() {
    let (mut classifier, mut files, now) = primed();
    files.set(SAVE, 3524, 2);

    assert_eq!(
        wrote(
            &mut classifier,
            &mut files,
            &[
                SAVE,
                SAVE,
                "ModernSharedStashSoftCoreV2.d2i",
                "ModernSharedStashSoftCoreV2.d2i",
                CONTROLS,
                CONTROLS,
                CONTROLS,
                CONTROLS,
                "Settings.json",
                "Settings.json",
            ],
            now
        ),
        left(CHARACTER)
    );
}
