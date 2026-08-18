//! Counting runs.
//!
//! The classifier reports what the game did. This decides what that means for a run, which is
//! interpretation rather than observation and so belongs here rather than in `d2r`.
//!
//! A run is timed from the end of the previous one. Entering a game cannot be detected, since it
//! writes nothing at the moment it happens, whereas leaving one can be, reliably, so that is what
//! the clock is measured between. It also matches how the time is spent: making the next game is
//! part of the run, not a gap between two.
//!
//! Two consequences follow, and both are deliberate. **The first game of a session is never
//! counted**, because nothing timed its beginning. And **being in a run is the normal state while
//! playing**, menu included, because nothing can say when the menu is on screen and pretending
//! otherwise reported the wrong thing for half of every run.
//!
//! Where the game cannot say something, the player can: the hotkeys claim a first run, correct one
//! that went wrong, and stop tracking without closing the game.
//!
//! No clock is owned here. Times arrive as arguments, so a whole session plays out in a test.

use std::time::{Duration, Instant};

use d2r::classifier::GameEvent;

pub mod stream;

#[cfg(test)]
mod tests;

pub use stream::Update;

/// Where the player is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum State {
    /// The game is not running.
    #[default]
    NoProcess,

    /// The game is running and no run is in progress.
    ///
    /// Either nothing has started one yet this session, or the player stopped tracking. The two
    /// look the same and behave differently; see [`Tracker::allowed`].
    Stopped,

    /// A run is in progress.
    Running {
        /// Who is running, once anything has said.
        ///
        /// Not always known: a run started by the hotkey before anything named a character has
        /// nobody to name, and inventing one would be worse than admitting it.
        character: Option<String>,

        /// Which run of the session this is, counting from one.
        index: u32,
    },

    /// A run is in progress with its clock held.
    Paused {
        /// Who is running, if anything has said.
        character: Option<String>,

        /// Which run of the session this is.
        index: u32,
    },
}

impl State {
    /// Who is running, if a run is in progress at all.
    pub fn character(&self) -> Option<&str> {
        match self {
            Self::Running { character, .. } | Self::Paused { character, .. } => {
                character.as_deref()
            }
            Self::NoProcess | Self::Stopped => None,
        }
    }

    /// Which run is in progress, if one is.
    pub fn index(&self) -> Option<u32> {
        match self {
            Self::Running { index, .. } | Self::Paused { index, .. } => Some(*index),
            Self::NoProcess | Self::Stopped => None,
        }
    }

    /// Whether a run is in progress, held or not.
    pub fn in_run(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Paused { .. })
    }
}

/// Something that happened, from the game or from the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The classifier worked out what the game did.
    Game(GameEvent),

    /// The player asked to start, or restart, a run.
    StartRequested,

    /// The player asked to hold the clock, or to release it.
    PauseRequested,

    /// The player asked to stop tracking. They are still playing; they are not running.
    StopRequested,
}

/// A finished run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Which run of the session this was, counting from one.
    pub index: u32,

    /// Who was running, if anything named them.
    pub character: Option<String>,

    /// How long it took, not counting time spent paused.
    pub duration: Duration,

    /// When it ended.
    pub ended: Instant,
}

/// Follows the game and counts the runs.
#[derive(Debug)]
pub struct Tracker {
    /// Where the player is.
    state: State,

    /// Whether leaving a game may start the next run on its own.
    ///
    /// True once the game appears, false once the player stops tracking. It is what tells the two
    /// kinds of [`State::Stopped`] apart: waiting for the first run of a session, and stopped on
    /// purpose. Without it, leaving a game after stopping would quietly begin tracking again, and
    /// muling or organising the stash would produce runs out of ordinary play.
    allowed: bool,

    /// The index the next run to start will take.
    next_index: u32,

    /// The index of the run in progress, if one is.
    index: Option<u32>,

    /// When the run in progress began.
    started: Option<Instant>,

    /// When the current hold began, if the clock is held.
    held_since: Option<Instant>,

    /// How long the run in progress has spent held.
    held: Duration,

    /// Who the run in progress belongs to, once something in that run has said.
    ///
    /// Cleared whenever a run starts. Carrying the previous run's name forward would be a guess,
    /// and wrong precisely when the player switches character, so a run stays unnamed until
    /// something inside it names one: a save while it is under way, or the exit that ends it.
    character: Option<String>,

    /// Every run that finished, oldest first.
    runs: Vec<Run>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            state: State::default(),
            allowed: false,
            next_index: 1,
            index: None,
            started: None,
            held_since: None,
            held: Duration::ZERO,
            character: None,
            runs: Vec::new(),
        }
    }
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

    /// Every run that finished, oldest first.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// When the run in progress began, if one is.
    pub fn started(&self) -> Option<Instant> {
        self.started
    }

    /// Who the run in progress belongs to, if anything in it has said.
    pub fn character(&self) -> Option<&str> {
        self.character.as_deref()
    }

    /// How long the run in progress has lasted, not counting time held.
    pub fn elapsed(&self, now: Instant) -> Option<Duration> {
        let started = self.started?;
        let held = self.held + self.held_since.map_or(Duration::ZERO, |at| now - at);

        Some((now - started).saturating_sub(held))
    }

    /// Takes in something that happened, and reports the state if it changed.
    pub fn observe(&mut self, input: &Input, at: Instant) -> Option<State> {
        let before = self.state.clone();
        let runs_before = self.runs.len();

        match input {
            Input::Game(GameEvent::Started) => self.game_appeared(),
            Input::Game(GameEvent::Quit) => self.game_went_away(),
            Input::Game(GameEvent::Left { character }) => self.left(character, at),

            // A save says nothing about where a run begins or ends, but it does say who is
            // playing, and for a run that started at the previous exit it is usually the first
            // thing to do so.
            Input::Game(GameEvent::Saved { character, .. }) => self.saved(character),

            Input::StartRequested => self.start(at),
            Input::PauseRequested => self.toggle_hold(at),
            Input::StopRequested => self.stop(),
        }

        // Logged whether or not anything moved. An input that changes nothing is exactly what a
        // misbehaving tracker looks like from outside, so silence has to be visible too.
        tracing::debug!(
            ?input,
            ?before,
            after = ?self.state,
            allowed = self.allowed,
            elapsed = ?self.elapsed(at),
            recorded_a_run = self.runs.len() > runs_before,
            "the tracker took an input"
        );

        (self.state != before).then(|| self.state.clone())
    }

    /// The game turned up.
    fn game_appeared(&mut self) {
        // Only a transition out of nothing: the sensor reports a game that was already running when
        // watching began, and may report again, and neither may disturb a run under way.
        if self.state == State::NoProcess {
            self.state = State::Stopped;
            self.allowed = true;
        }
    }

    /// The game went away, quit or crashed.
    fn game_went_away(&mut self) {
        // Whatever was in progress never finished, so it is not a run. Its index goes back.
        self.discard();
        self.allowed = false;
        self.state = State::NoProcess;
    }

    /// A character left a game.
    fn left(&mut self, character: &str, at: Instant) {
        // Stopped on purpose. Leaving the game they were in does not contradict that, and treating
        // it as a fresh start is exactly what stopping exists to prevent.
        if !self.allowed {
            return;
        }

        // Names the run that is ending, not the one about to start. Whoever just left is who was
        // playing; who plays next is not known until they do something.
        self.name(character);
        self.record(at);
        self.begin(at);
    }

    /// The game saved during a run.
    ///
    /// Says nothing about where a run begins or ends, but it does say who is playing, and for a run
    /// that started at the previous exit this is usually the first thing that does.
    fn saved(&mut self, character: &str) {
        if self.state.in_run() {
            self.name(character);
        }
    }

    /// Records who the run in progress belongs to.
    fn name(&mut self, character: &str) {
        self.character = Some(character.to_owned());

        self.state = match &self.state {
            State::Running { index, .. } => State::Running {
                character: self.character.clone(),
                index: *index,
            },
            State::Paused { index, .. } => State::Paused {
                character: self.character.clone(),
                index: *index,
            },
            other => other.clone(),
        };
    }

    /// The player asked to start, or to start again.
    fn start(&mut self, at: Instant) {
        // Always allows. It is the player asking directly, so it overrides a previous stop;
        // obeying the flag here would leave no way back to tracking.
        self.allowed = true;

        // Restarting keeps the index: it is the same run, timed again from now. It keeps the name
        // too, for the same reason. A keypress says nothing about who is playing, so it can neither
        // supply a name nor take one away.
        let index = self.index.unwrap_or(self.next_index);

        self.index = Some(index);
        self.started = Some(at);
        self.held = Duration::ZERO;
        self.held_since = None;
        self.state = State::Running {
            character: self.character.clone(),
            index,
        };
    }

    /// The player asked to hold the clock, or to release it.
    fn toggle_hold(&mut self, at: Instant) {
        match &self.state {
            State::Running { character, index } => {
                self.state = State::Paused {
                    character: character.clone(),
                    index: *index,
                };
                self.held_since = Some(at);
            }

            State::Paused { character, index } => {
                self.state = State::Running {
                    character: character.clone(),
                    index: *index,
                };

                if let Some(since) = self.held_since.take() {
                    self.held += at - since;
                }
            }

            // Nothing to hold. Holding a session that is not running would only be confusing.
            State::NoProcess | State::Stopped => {}
        }
    }

    /// The player asked to stop tracking.
    fn stop(&mut self) {
        // Stopping deliberately means the run was not one, so it is discarded rather than recorded.
        self.discard();
        self.allowed = false;
        self.state = State::Stopped;
    }

    /// Starts the clock of the run to come.
    ///
    /// It begins unnamed. The player may be about to run a different character, and the previous
    /// one's name would be a guess dressed up as knowledge.
    fn begin(&mut self, at: Instant) {
        let index = self.next_index;

        self.index = Some(index);
        self.started = Some(at);
        self.held = Duration::ZERO;
        self.held_since = None;
        self.character = None;
        self.state = State::Running {
            character: None,
            index,
        };
    }

    /// Records the run in progress, if there is one.
    fn record(&mut self, at: Instant) {
        let (Some(duration), Some(index)) = (self.elapsed(at), self.index) else {
            return;
        };

        self.runs.push(Run {
            index,
            character: self.character.clone(),
            duration,
            ended: at,
        });

        self.next_index += 1;
        self.discard();
    }

    /// Throws away the run in progress, leaving its index for the next one.
    fn discard(&mut self) {
        self.index = None;
        self.started = None;
        self.held = Duration::ZERO;
        self.held_since = None;
        self.character = None;
    }
}
