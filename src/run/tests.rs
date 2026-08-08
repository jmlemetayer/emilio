//! The transitions, played out against made-up sessions.
//!
//! Every rule the tracker follows is pinned down here, including the awkward ones: where a run's
//! clock actually starts, what happens to a game nothing announced, and which events must *not*
//! produce a run.

use super::*;

/// A session that has just had the game launched, with the clock running from that moment.
fn launched() -> (Tracker, Instant) {
    let mut tracker = Tracker::new();
    let start = Instant::now();

    tracker.observe(&Input::GameAppeared, start);

    (tracker, start)
}

/// A moment, so many seconds into the session.
fn secs(base: Instant, seconds: u64) -> Instant {
    base + Duration::from_secs(seconds)
}

fn entered(character: &str) -> Input {
    Input::Save(SaveEvent::Entered {
        character: character.to_owned(),
    })
}

fn left(character: &str) -> Input {
    Input::Save(SaveEvent::Left {
        character: character.to_owned(),
    })
}

fn touched(character: &str) -> Input {
    Input::Save(SaveEvent::Touched {
        character: character.to_owned(),
    })
}

fn saved(character: &str) -> Input {
    Input::Save(SaveEvent::Saved {
        character: character.to_owned(),
        size_delta: 0,
    })
}

#[test]
fn starts_out_knowing_nothing() {
    assert_eq!(*Tracker::new().state(), State::NoProcess);
    assert!(Tracker::new().runs().is_empty());
}

#[test]
fn the_game_appearing_puts_the_player_in_the_menu() {
    let (tracker, _) = launched();

    assert_eq!(*tracker.state(), State::Stopped);
}

/// The process sensor reports a game that was already running when Emilio started, and may report
/// again. Neither may restart the clock of a run already under way.
#[test]
fn the_game_appearing_twice_changes_nothing() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    let again = tracker.observe(&Input::GameAppeared, secs(start, 20));

    assert_eq!(again, None);
    assert_eq!(
        tracker.elapsed(secs(start, 30)),
        Some(Duration::from_secs(30))
    );
}

#[test]
fn entering_a_game_names_who_is_playing() {
    let (mut tracker, start) = launched();

    let state = tracker.observe(&entered("Vikhyat"), secs(start, 10));

    assert_eq!(
        state,
        Some(State::Running {
            character: Some("Vikhyat".to_owned())
        })
    );
    assert_eq!(tracker.state().character(), Some("Vikhyat"));
}

#[test]
fn leaving_a_game_records_the_run_and_returns_to_the_menu() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&left("Vikhyat"), secs(start, 70));

    assert_eq!(*tracker.state(), State::Stopped);
    assert_eq!(tracker.runs().len(), 1);
    assert_eq!(tracker.runs()[0].character.as_deref(), Some("Vikhyat"));
}

/// The clock starts when the previous game ended, not when the next one is entered: making the
/// next game is part of the run. For the first run of a session, that anchor is the launch.
#[test]
fn a_run_is_timed_from_the_end_of_the_one_before_it() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&left("Vikhyat"), secs(start, 70));

    // Sixty seconds in the game, but the run began at the launch.
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(70));

    tracker.observe(&entered("Vikhyat"), secs(start, 80));
    tracker.observe(&left("Vikhyat"), secs(start, 100));

    // Thirty seconds: ten spent making the game, twenty playing it.
    assert_eq!(tracker.runs()[1].duration, Duration::from_secs(30));
}

/// Leaving from the menu, having never been in a game, must not invent a run.
#[test]
fn leaving_without_ever_playing_records_nothing() {
    let (mut tracker, start) = launched();

    tracker.observe(&left("Vikhyat"), secs(start, 10));

    assert!(tracker.runs().is_empty());
    assert_eq!(*tracker.state(), State::Stopped);
}

/// A game started from the keyboard announces itself in no way at all. The save that follows is
/// the first evidence it ever happened, and has to be enough.
#[test]
fn a_save_reveals_a_game_nothing_announced() {
    let (mut tracker, start) = launched();

    let state = tracker.observe(&saved("Vikhyat"), secs(start, 30));

    assert_eq!(
        state,
        Some(State::Running {
            character: Some("Vikhyat".to_owned())
        })
    );

    tracker.observe(&left("Vikhyat"), secs(start, 90));
    assert_eq!(tracker.runs().len(), 1);
}

/// A menu click cannot be told from the click that starts a game, so it does not start a run. It
/// does push the clock forward, so time spent deciding in the menu is not charged to the run.
#[test]
fn a_menu_click_pushes_the_clock_forward_without_entering() {
    let (mut tracker, start) = launched();

    let state = tracker.observe(&touched("Vikhyat"), secs(start, 40));

    assert_eq!(state, None);
    assert_eq!(*tracker.state(), State::Stopped);
    assert_eq!(
        tracker.elapsed(secs(start, 60)),
        Some(Duration::from_secs(20))
    );
}

/// Once in a game the clock has to be left alone. Menu-shaped writes still happen, and restarting
/// the run mid-game would quietly discard the time already played.
#[test]
fn a_click_during_a_game_leaves_the_clock_alone() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&touched("Vikhyat"), secs(start, 40));

    assert_eq!(
        tracker.elapsed(secs(start, 60)),
        Some(Duration::from_secs(60))
    );
}

#[test]
fn pausing_holds_the_clock_and_carrying_on_releases_it() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&Input::PauseRequested, secs(start, 20));

    assert_eq!(
        *tracker.state(),
        State::Paused {
            character: Some("Vikhyat".to_owned())
        }
    );

    // Held, so the clock does not move.
    assert_eq!(
        tracker.elapsed(secs(start, 50)),
        Some(Duration::from_secs(20))
    );

    tracker.observe(&Input::PauseRequested, secs(start, 50));
    assert_eq!(
        *tracker.state(),
        State::Running {
            character: Some("Vikhyat".to_owned())
        }
    );

    // Ten more seconds of play, on top of the twenty before the pause.
    assert_eq!(
        tracker.elapsed(secs(start, 60)),
        Some(Duration::from_secs(30))
    );
}

#[test]
fn time_spent_paused_is_left_out_of_the_recorded_run() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&Input::PauseRequested, secs(start, 20));
    tracker.observe(&Input::PauseRequested, secs(start, 80));
    tracker.observe(&left("Vikhyat"), secs(start, 100));

    // A hundred seconds passed, sixty of them paused.
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(40));
}

/// There is no clock to hold in the menu, and pretending otherwise would only be confusing.
#[test]
fn pausing_outside_a_game_does_nothing() {
    let (mut tracker, start) = launched();

    assert_eq!(
        tracker.observe(&Input::PauseRequested, secs(start, 10)),
        None
    );
    assert_eq!(*tracker.state(), State::Stopped);
}

/// The hotkey is what covers a game the files never announced, so it has to start a run outright
/// rather than wait for anything to confirm it.
#[test]
fn the_start_hotkey_starts_a_run_from_scratch() {
    let (mut tracker, start) = launched();

    tracker.observe(&touched("Vikhyat"), secs(start, 10));
    let state = tracker.observe(&Input::StartRequested, secs(start, 30));

    assert_eq!(
        state,
        Some(State::Running {
            character: Some("Vikhyat".to_owned())
        })
    );
    assert_eq!(
        tracker.elapsed(secs(start, 45)),
        Some(Duration::from_secs(15))
    );
}

/// Pressed again, it restarts the run rather than doing nothing: it is the only way back from a
/// run the tracker has got wrong.
#[test]
fn the_start_hotkey_pressed_again_restarts_the_run() {
    let (mut tracker, start) = launched();

    tracker.observe(&Input::StartRequested, secs(start, 10));
    tracker.observe(&Input::StartRequested, secs(start, 40));

    assert_eq!(
        tracker.elapsed(secs(start, 50)),
        Some(Duration::from_secs(10))
    );
}

/// Nothing has named a character, and the type has to admit it rather than invent one.
#[test]
fn a_run_can_be_under_way_with_nobody_named() {
    let (mut tracker, start) = launched();

    tracker.observe(&Input::StartRequested, secs(start, 10));

    assert_eq!(*tracker.state(), State::Running { character: None });
    assert_eq!(tracker.state().character(), None);
}

#[test]
fn quitting_closes_the_run_and_the_session() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&Input::Save(SaveEvent::QuitCleanly), secs(start, 70));

    assert_eq!(*tracker.state(), State::NoProcess);
    assert_eq!(tracker.runs().len(), 1);
    assert_eq!(tracker.runs()[0].duration, Duration::from_secs(70));
}

/// A crash loses no more than the moment it happened: the run up to that point still counts.
#[test]
fn a_crash_still_records_the_run() {
    let (mut tracker, start) = launched();

    tracker.observe(&entered("Vikhyat"), secs(start, 10));
    tracker.observe(&Input::Save(SaveEvent::Crashed), secs(start, 70));

    assert_eq!(*tracker.state(), State::NoProcess);
    assert_eq!(tracker.runs().len(), 1);
}

/// Quitting from the menu ends the session without inventing a run out of the time spent there.
#[test]
fn quitting_from_the_menu_records_nothing() {
    let (mut tracker, start) = launched();

    tracker.observe(&Input::Save(SaveEvent::QuitCleanly), secs(start, 30));

    assert_eq!(*tracker.state(), State::NoProcess);
    assert!(tracker.runs().is_empty());
}

#[test]
fn there_is_no_clock_before_the_game_appears() {
    let tracker = Tracker::new();

    assert_eq!(tracker.elapsed(Instant::now()), None);
}

/// A session of several runs, checked end to end.
#[test]
fn a_handful_of_runs_are_all_counted() {
    let (mut tracker, start) = launched();

    for run in 0..3 {
        let base = run * 100;
        tracker.observe(&entered("Vikhyat"), secs(start, base + 10));
        tracker.observe(&left("Vikhyat"), secs(start, base + 100));
    }

    assert_eq!(tracker.runs().len(), 3);
    assert_eq!(*tracker.state(), State::Stopped);

    let total: Duration = tracker.runs().iter().map(|run| run.duration).sum();
    assert_eq!(total, Duration::from_secs(300));
}
