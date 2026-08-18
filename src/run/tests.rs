//! The rules, played out against made-up sessions.
//!
//! Every rule the tracker follows is pinned down here, including the awkward ones: which games are
//! not runs, what stopping does to the one in progress, and why leaving does not always start
//! another.

use super::*;

/// A session with the game freshly launched. Runs are allowed, none has started.
fn launched() -> (Tracker, Instant) {
    let mut tracker = Tracker::new();
    let start = Instant::now();

    tracker.observe(&Input::Game(GameEvent::Started), start);

    (tracker, start)
}

/// A session already counting runs, which is the normal case: one game has been left, so the clock
/// of the next run is going. Times are measured from that moment.
fn counting() -> (Tracker, Instant) {
    let (mut tracker, launch) = launched();
    let start = launch + Duration::from_secs(30);

    tracker.observe(&left("Kate"), start);

    (tracker, start)
}

/// A moment, so many seconds in.
fn secs(base: Instant, seconds: u64) -> Instant {
    base + Duration::from_secs(seconds)
}

fn left(character: &str) -> Input {
    Input::Game(GameEvent::Left {
        character: character.to_owned(),
    })
}

fn saved(character: &str) -> Input {
    Input::Game(GameEvent::Saved {
        character: character.to_owned(),
        size_delta: 0,
    })
}

#[test]
fn starts_out_knowing_nothing() {
    let tracker = Tracker::new();

    assert_eq!(*tracker.state(), State::NoProcess);
    assert!(tracker.runs().is_empty());
    assert_eq!(tracker.elapsed(Instant::now()), None);
}

/// The game appearing allows runs without starting one. Nothing has timed anything yet.
#[test]
fn the_game_appearing_starts_no_clock() {
    let (tracker, start) = launched();

    assert_eq!(*tracker.state(), State::Stopped);
    assert_eq!(tracker.elapsed(secs(start, 30)), None);
}

/// The sensor reports a game that was already running when watching began, and may report again.
/// Neither may disturb a run under way.
#[test]
fn the_game_appearing_twice_changes_nothing() {
    let (mut tracker, start) = counting();

    let again = tracker.observe(&Input::Game(GameEvent::Started), secs(start, 20));

    assert_eq!(again, None);
    assert_eq!(
        tracker.elapsed(secs(start, 30)),
        Some(Duration::from_secs(30))
    );
}

/// Nothing timed the first game, so its length would include however long was spent logging in.
/// It still ends something: the clock of the next run starts here.
#[test]
fn the_first_game_to_end_is_not_a_run_but_starts_the_next() {
    let (mut tracker, launch) = launched();

    tracker.observe(&left("Kate"), secs(launch, 30));

    assert!(tracker.runs().is_empty());

    // Unnamed: Kate left the game that just ended, and says nothing about who plays the next one.
    assert_eq!(
        *tracker.state(),
        State::Running {
            character: None,
            index: 1
        }
    );
}

#[test]
fn leaving_records_the_run_and_starts_the_next() {
    let (mut tracker, start) = counting();

    tracker.observe(&left("Kate"), secs(start, 60));

    assert_eq!(tracker.runs().len(), 1);
    assert_eq!(tracker.runs()[0].index, 1);
    assert_eq!(tracker.runs()[0].character.as_deref(), Some("Kate"));
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(60));

    assert_eq!(
        *tracker.state(),
        State::Running {
            character: None,
            index: 2
        }
    );
}

/// A run belongs to whoever plays it, which is only known once something in that run says so.
/// Carrying the previous run's name forward would be a guess, and wrong exactly when the player
/// switches character.
#[test]
fn a_new_run_starts_unnamed_until_something_names_it() {
    let (mut tracker, start) = counting();

    assert_eq!(tracker.character(), None);

    let moved = tracker.observe(&saved("Daphne"), secs(start, 20));

    assert_eq!(
        moved,
        Some(State::Running {
            character: Some("Daphne".to_owned()),
            index: 1
        })
    );

    // And the run is recorded under whoever was actually playing it.
    tracker.observe(&left("Daphne"), secs(start, 60));
    assert_eq!(tracker.runs()[0].character.as_deref(), Some("Daphne"));
}

/// Switching character between runs must not inherit the previous one's name.
#[test]
fn switching_character_is_not_carried_over() {
    let (mut tracker, start) = counting();

    tracker.observe(&saved("Kate"), secs(start, 20));
    tracker.observe(&left("Kate"), secs(start, 60));

    assert_eq!(tracker.runs()[0].character.as_deref(), Some("Kate"));
    assert_eq!(tracker.character(), None);

    tracker.observe(&saved("Daphne"), secs(start, 90));
    tracker.observe(&left("Daphne"), secs(start, 120));

    assert_eq!(tracker.runs()[1].character.as_deref(), Some("Daphne"));
}

/// A keypress says nothing about who is playing, so it can neither supply a name nor take one
/// away: restarting keeps the run, its number, and whatever named it.
#[test]
fn the_start_hotkey_neither_gives_nor_takes_a_name() {
    let (mut tracker, launch) = launched();

    tracker.observe(&Input::StartRequested, secs(launch, 10));
    assert_eq!(tracker.character(), None);

    tracker.observe(&saved("Kate"), secs(launch, 40));
    tracker.observe(&Input::StartRequested, secs(launch, 60));

    assert_eq!(
        *tracker.state(),
        State::Running {
            character: Some("Kate".to_owned()),
            index: 1
        }
    );
}

/// A run is timed from the end of the previous one, so the time spent making the next game counts
/// towards it.
#[test]
fn runs_are_timed_end_to_end() {
    let (mut tracker, start) = counting();

    tracker.observe(&left("Kate"), secs(start, 60));
    tracker.observe(&left("Kate"), secs(start, 150));

    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(60));
    assert_eq!(tracker.runs()[1].duration, Duration::from_secs(90));
}

#[test]
fn runs_are_numbered_from_one() {
    let (mut tracker, start) = counting();

    for run in 1..=3 {
        tracker.observe(&left("Kate"), secs(start, run * 60));
    }

    let indices: Vec<_> = tracker.runs().iter().map(|run| run.index).collect();
    assert_eq!(indices, vec![1, 2, 3]);
}

/// A save moves no boundary and no clock. It only says who is playing.
#[test]
fn a_save_moves_neither_boundary_nor_clock() {
    let (mut tracker, start) = counting();

    tracker.observe(&saved("Kate"), secs(start, 20));

    assert!(matches!(tracker.state(), State::Running { index: 1, .. }));
    assert!(tracker.runs().is_empty());
    assert_eq!(
        tracker.elapsed(secs(start, 30)),
        Some(Duration::from_secs(30))
    );
}

/// A save outside a run has nothing to attribute itself to.
#[test]
fn a_save_outside_a_run_names_nothing() {
    let (mut tracker, launch) = launched();

    let moved = tracker.observe(&saved("Kate"), secs(launch, 20));

    assert_eq!(moved, None);
    assert_eq!(tracker.character(), None);
}

#[test]
fn holding_the_clock_leaves_the_time_out_of_the_run() {
    let (mut tracker, start) = counting();

    // Named by a save, so the hold can be checked to keep it.
    tracker.observe(&saved("Kate"), secs(start, 10));
    tracker.observe(&Input::PauseRequested, secs(start, 20));
    assert_eq!(
        *tracker.state(),
        State::Paused {
            character: Some("Kate".to_owned()),
            index: 1
        }
    );

    // Held, so the clock does not move.
    assert_eq!(
        tracker.elapsed(secs(start, 50)),
        Some(Duration::from_secs(20))
    );

    tracker.observe(&Input::PauseRequested, secs(start, 80));
    tracker.observe(&left("Kate"), secs(start, 100));

    // A hundred seconds passed, sixty of them held.
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(40));
}

/// Leaving while held still ends the run.
#[test]
fn leaving_while_held_still_ends_the_run() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::PauseRequested, secs(start, 20));
    tracker.observe(&left("Kate"), secs(start, 60));

    assert_eq!(tracker.runs().len(), 1);
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(20));
    assert!(matches!(tracker.state(), State::Running { index: 2, .. }));
}

/// There is no clock to hold when nothing is running.
#[test]
fn holding_outside_a_run_does_nothing() {
    let (mut tracker, launch) = launched();

    assert_eq!(
        tracker.observe(&Input::PauseRequested, secs(launch, 10)),
        None
    );
    assert_eq!(*tracker.state(), State::Stopped);
}

/// The hotkey is what claims the first run of a session, which nothing else can.
#[test]
fn the_start_hotkey_starts_a_run() {
    let (mut tracker, launch) = launched();

    tracker.observe(&Input::StartRequested, secs(launch, 30));

    assert!(matches!(tracker.state(), State::Running { index: 1, .. }));
    assert_eq!(
        tracker.elapsed(secs(launch, 45)),
        Some(Duration::from_secs(15))
    );
}

/// Pressed again it restarts the same run rather than beginning another: it is the only way to
/// correct one the tracker has got wrong.
#[test]
fn the_start_hotkey_restarts_the_same_run() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::StartRequested, secs(start, 40));

    assert!(matches!(tracker.state(), State::Running { index: 1, .. }));
    assert_eq!(
        tracker.elapsed(secs(start, 50)),
        Some(Duration::from_secs(10))
    );

    // Still run one when it finishes.
    tracker.observe(&left("Kate"), secs(start, 70));
    assert_eq!(tracker.runs()[0].index, 1);
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(30));
}

/// Nothing has named a character, and the state has to admit it rather than invent one.
#[test]
fn a_run_can_be_under_way_with_nobody_named() {
    let (mut tracker, launch) = launched();

    tracker.observe(&Input::StartRequested, secs(launch, 10));

    assert_eq!(
        *tracker.state(),
        State::Running {
            character: None,
            index: 1
        }
    );
}

/// Stopping means the run was not one, so it is thrown away rather than recorded.
#[test]
fn stopping_discards_the_run_in_progress() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::StopRequested, secs(start, 40));

    assert!(tracker.runs().is_empty());
    assert_eq!(*tracker.state(), State::Stopped);
    assert_eq!(tracker.elapsed(secs(start, 60)), None);
}

/// The discarded run's number goes back, so the next run to start takes it.
#[test]
fn a_discarded_run_frees_its_number() {
    let (mut tracker, start) = counting();

    tracker.observe(&left("Kate"), secs(start, 60));
    tracker.observe(&Input::StopRequested, secs(start, 90));
    tracker.observe(&Input::StartRequested, secs(start, 120));

    assert!(matches!(tracker.state(), State::Running { index: 2, .. }));
}

/// The case stopping exists for. The player says they have stopped while genuinely in a game, then
/// leaves it; that exit must not quietly begin tracking again, or muling and organising the stash
/// would produce runs out of ordinary play.
#[test]
fn leaving_after_stopping_starts_nothing() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::StopRequested, secs(start, 40));
    tracker.observe(&left("Kate"), secs(start, 60));

    assert_eq!(*tracker.state(), State::Stopped);
    assert!(tracker.runs().is_empty());

    // And it stays stopped however many games are left.
    tracker.observe(&left("Kate"), secs(start, 200));
    assert_eq!(*tracker.state(), State::Stopped);
    assert!(tracker.runs().is_empty());
}

/// Starting is the player asking directly, so it overrides having stopped. Obeying the flag there
/// would leave no way back to tracking.
#[test]
fn starting_undoes_stopping() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::StopRequested, secs(start, 40));
    tracker.observe(&Input::StartRequested, secs(start, 60));

    assert!(matches!(tracker.state(), State::Running { .. }));

    // And leaving counts again from here.
    tracker.observe(&left("Kate"), secs(start, 100));
    assert_eq!(tracker.runs().len(), 1);
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(40));
}

/// A run interrupted by the game going away never finished, so it is not one.
#[test]
fn the_game_going_away_discards_the_run_in_progress() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::Game(GameEvent::Quit), secs(start, 60));

    assert_eq!(*tracker.state(), State::NoProcess);
    assert!(tracker.runs().is_empty());
}

/// And leaving afterwards must not start one either, since there is no game to leave.
#[test]
fn the_game_going_away_disallows_runs_until_it_returns() {
    let (mut tracker, start) = counting();

    tracker.observe(&Input::Game(GameEvent::Quit), secs(start, 60));
    tracker.observe(&left("Kate"), secs(start, 70));

    assert_eq!(*tracker.state(), State::NoProcess);

    tracker.observe(&Input::Game(GameEvent::Started), secs(start, 100));
    assert_eq!(*tracker.state(), State::Stopped);

    tracker.observe(&left("Kate"), secs(start, 130));
    assert!(matches!(tracker.state(), State::Running { .. }));
}

/// A whole session, checked end to end: the first game is not counted, three are, and the numbers
/// run consecutively.
#[test]
fn a_session_of_several_runs() {
    let (mut tracker, launch) = launched();

    // The game already under way when Emilio started. Not a run.
    tracker.observe(&left("Kate"), secs(launch, 40));

    for run in 1..=3 {
        tracker.observe(&saved("Kate"), secs(launch, 40 + run * 60 - 30));
        tracker.observe(&left("Kate"), secs(launch, 40 + run * 60));
    }

    assert_eq!(tracker.runs().len(), 3);
    assert!(
        tracker
            .runs()
            .iter()
            .all(|run| run.duration == Duration::from_secs(60))
    );
    assert_eq!(
        tracker
            .runs()
            .iter()
            .map(|run| run.index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}
