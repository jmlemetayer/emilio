//! Keeping track of where the player is, and how long each run took.
//!
//! The sensing layer reports what the game did. This decides what that means for the player: in
//! the menu, in a game, paused, or gone. It is the first thing in Emilio that interprets rather
//! than observes, which is why it lives here and not in `d2r`.
//!
//! The awkward part is that a run does not begin where you would expect. Entering a game writes
//! nothing to disk, so there is no moment to catch; what there is, reliably, is the moment the
//! previous game ended. So a run is timed from the end of the one before it, which also matches
//! how the time is actually spent: making the next game is part of the run, not a gap between two.
//! A menu click pushes that start forward, so sitting in the menu deciding what to do next does
//! not end up charged to the run.
//!
//! Like the classifier, this owns no clock. Times arrive as arguments, so a whole session can be
//! played out in a unit test in no time at all.

use std::time::{Duration, Instant};

use d2r::classifier::SaveEvent;

#[cfg(test)]
mod tests;

/// Where the player is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum State {
    /// The game is not running.
    #[default]
    NoProcess,

    /// The game is running, but the player is in the menu.
    Stopped,

    /// The player is in a game.
    Running {
        /// Who is playing, once anything has said so.
        ///
        /// This is not always known. Starting a game from the keyboard writes nothing that names a
        /// character, and the start hotkey exists for exactly that case, so a run can legitimately
        /// be under way with nobody named yet.
        character: Option<String>,
    },

    /// The player is in a game, with the clock held.
    Paused {
        /// Who is playing, if anything has said so.
        character: Option<String>,
    },
}

impl State {
    /// Who is playing, if the player is in a game at all.
    pub fn character(&self) -> Option<&str> {
        match self {
            Self::Running { character } | Self::Paused { character } => character.as_deref(),
            Self::NoProcess | Self::Stopped => None,
        }
    }

    /// Whether a run is under way, paused or not.
    pub fn in_game(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Paused { .. })
    }
}

/// Something that happened, from any of the sensors or from the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The game process appeared.
    GameAppeared,

    /// The classifier worked out what a write meant.
    Save(SaveEvent),

    /// The player asked to start, or restart, the current run.
    StartRequested,

    /// The player asked to pause, or to carry on.
    PauseRequested,
}

/// A finished run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Who was playing, if anything named them.
    pub character: Option<String>,

    /// How long it took, not counting time spent paused.
    pub duration: Duration,

    /// When it ended.
    pub ended: Instant,
}

/// Follows the sensors and counts the runs.
#[derive(Debug, Default)]
pub struct Tracker {
    /// Where the player is.
    state: State,

    /// When the run under way began, if one has.
    ///
    /// Set independently of the state: a run's clock starts when the previous game ended, which is
    /// a moment spent in the menu rather than in a game.
    started: Option<Instant>,

    /// When the current pause began, if the clock is held.
    held_since: Option<Instant>,

    /// How long the run under way has spent paused.
    held: Duration,

    /// The last character anything named, so a run started by hotkey still knows who is playing.
    last_character: Option<String>,

    /// Every run that has finished, oldest first.
    runs: Vec<Run>,
}

impl Tracker {
    /// A tracker that has seen nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the player is.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Every run that has finished, oldest first.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// How long the run under way has lasted, not counting time paused.
    pub fn elapsed(&self, now: Instant) -> Option<Duration> {
        let started = self.started?;
        let held = self.held + self.held_since.map_or(Duration::ZERO, |at| now - at);

        Some((now - started).saturating_sub(held))
    }

    /// Takes in something that happened, and reports the state if it changed.
    pub fn observe(&mut self, input: &Input, at: Instant) -> Option<State> {
        let before = self.state.clone();

        match input {
            Input::GameAppeared => self.game_appeared(at),
            Input::Save(save) => self.saw(save, at),
            Input::StartRequested => self.start(at),
            Input::PauseRequested => self.toggle_pause(at),
        }

        (self.state != before).then(|| self.state.clone())
    }

    /// The game process turned up.
    fn game_appeared(&mut self, at: Instant) {
        // Only ever a transition out of nothing. The process sensor reports a game that was
        // already running when Emilio started, and that must not restart the clock of a run that
        // is already under way.
        if self.state == State::NoProcess {
            self.state = State::Stopped;
            self.begin(at);
        }
    }

    /// The classifier said what a write meant.
    fn saw(&mut self, save: &SaveEvent, at: Instant) {
        match save {
            // The one boundary that can be trusted. The run ends here, and the next one starts in
            // the same breath, because the time spent making the next game belongs to it.
            SaveEvent::Left { character } => {
                self.remember(character);
                self.finish(at);
                self.state = State::Stopped;
                self.begin(at);
            }

            // A menu click, which is as close as the files get to "about to play". It cannot be
            // told apart from any other click in the menu, so rather than start a run it pushes
            // the current one's clock forward: waiting in the menu is not part of the run.
            SaveEvent::Touched { character } => {
                self.remember(character);

                if !self.state.in_game() {
                    self.begin(at);
                }
            }

            SaveEvent::Entered { character } => {
                self.remember(character);
                self.enter();
            }

            // A save can only happen in a game. If we thought otherwise we were wrong, which is
            // what happens when a game is started from the keyboard: nothing announces it, and
            // this is the first evidence that it happened at all.
            SaveEvent::Saved { character, .. } => {
                self.remember(character);
                self.enter();
            }

            // The game is gone. Whatever was under way is over.
            SaveEvent::QuitCleanly | SaveEvent::Crashed => {
                self.finish(at);
                self.state = State::NoProcess;
                self.started = None;
            }
        }
    }

    /// The player asked to start, or to start again.
    fn start(&mut self, at: Instant) {
        self.begin(at);
        self.held = Duration::ZERO;
        self.held_since = None;
        self.state = State::Running {
            character: self.last_character.clone(),
        };
    }

    /// The player asked to hold the clock, or to release it.
    fn toggle_pause(&mut self, at: Instant) {
        match &self.state {
            State::Running { character } => {
                self.state = State::Paused {
                    character: character.clone(),
                };
                self.held_since = Some(at);
            }

            State::Paused { character } => {
                self.state = State::Running {
                    character: character.clone(),
                };

                if let Some(since) = self.held_since.take() {
                    self.held += at - since;
                }
            }

            // Nothing to hold. Pausing a menu would only be confusing.
            State::NoProcess | State::Stopped => {}
        }
    }

    /// Moves into a game, keeping the clock that is already running.
    fn enter(&mut self) {
        if !self.state.in_game() {
            self.state = State::Running {
                character: self.last_character.clone(),
            };
        }
    }

    /// Starts the clock of the run to come.
    fn begin(&mut self, at: Instant) {
        self.started = Some(at);
        self.held = Duration::ZERO;
        self.held_since = None;
    }

    /// Closes the run under way, if there was one worth keeping.
    fn finish(&mut self, at: Instant) {
        let Some(duration) = self.elapsed(at) else {
            return;
        };

        // A run nobody ever entered is not a run. Leaving the game twice without playing in
        // between, or quitting straight from the menu, should not leave a phantom in the log.
        if !self.state.in_game() {
            return;
        }

        self.runs.push(Run {
            character: self.last_character.clone(),
            duration,
            ended: at,
        });
    }

    /// Notes who is playing.
    fn remember(&mut self, character: &str) {
        self.last_character = Some(character.to_owned());
    }
}
