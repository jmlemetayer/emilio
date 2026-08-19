//! The three things only the player can say.
//!
//! Emilio can see that a game was left. It cannot see that the player counts this as a run, that
//! they have stopped for a while to mule, or that they want the clock held. Those arrive as
//! keystrokes, registered system-wide so that they work while D2R is focused and fullscreen, which
//! is the only time they are any use.
//!
//! Windows delivers a press to a hidden window belonging to whichever thread registered it, and
//! only while that thread is pumping its messages. Registration and pump therefore live together
//! on a thread of their own, and presses leave it on a channel like any other sensor's output.

use std::sync::mpsc::{Sender, channel};
use std::thread;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::compat;
use crate::errors::{Error, Result};

/// What the player asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Count this as a run, from now. Pressed again, it times the same run again from now.
    Start,

    /// Stop counting until told otherwise. The run in progress was not one.
    Stop,

    /// Hold the clock, or let it go again.
    Pause,
}

/// Which keys mean what.
///
/// Written to the config file as the strings the player typed, since a hotkey reads back from its
/// own `Display` form: `start = "alt+KeyQ"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    /// Starts a run, or restarts the one under way.
    #[serde(deserialize_with = "binding")]
    pub start: HotKey,

    /// Stops counting runs.
    #[serde(deserialize_with = "binding")]
    pub stop: HotKey,

    /// Holds the clock, or releases it.
    #[serde(deserialize_with = "binding")]
    pub pause: HotKey,
}

/// Reads one binding out of the settings file, saying what can be typed if it will not parse.
///
/// The parsing is global-hotkey's, but not the complaint: its own is addressed to somebody who has
/// found a missing key rather than to somebody who has mistyped one, and it asks them to report it
/// to a project this application does not use.
fn binding<'de, D>(deserializer: D) -> std::result::Result<HotKey, D::Error>
where
    D: Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;

    text.parse().map_err(|_| {
        serde::de::Error::custom(format!(
            "{text} is not a key combination: write any of alt, ctrl, shift and super first, \
             then one key, as in alt+Q or ctrl+shift+F1"
        ))
    })
}

impl Default for Bindings {
    /// The bindings MF timer uses, which is what a player's hands already know.
    fn default() -> Self {
        Self {
            start: HotKey::new(Some(Modifiers::ALT), Code::KeyQ),
            stop: HotKey::new(Some(Modifiers::ALT), Code::KeyW),
            pause: HotKey::new(Some(Modifiers::CONTROL), Code::Space),
        }
    }
}

impl Bindings {
    /// Every binding, with what it means.
    fn each(&self) -> [(HotKey, Intent); 3] {
        [
            (self.start, Intent::Start),
            (self.stop, Intent::Stop),
            (self.pause, Intent::Pause),
        ]
    }

    /// What a press of the hotkey with this id means, if it is one of ours.
    fn meaning(&self, id: u32) -> Option<Intent> {
        self.each()
            .into_iter()
            .find(|(hotkey, _)| hotkey.id() == id)
            .map(|(_, intent)| intent)
    }
}

/// Keeps the hotkeys registered. Dropping it gives them back to the rest of the desktop.
pub struct HotkeyWatcher {
    thread: u32,
}

impl Drop for HotkeyWatcher {
    fn drop(&mut self) {
        // Nothing else wakes a thread waiting for a message, and until it wakes it holds the
        // registrations, so the next binding of the same keys would be refused.
        compat::post_quit(self.thread);
    }
}

/// Registers the bindings and reports every press until the watcher is dropped.
pub fn watch(sender: UnboundedSender<Intent>, bindings: Bindings) -> Result<HotkeyWatcher> {
    let (ready, started) = channel();

    thread::Builder::new()
        .name("emilio-hotkeys".to_owned())
        .spawn(move || run(&sender, bindings, &ready))?;

    let thread = started.recv().map_err(|_| Error::HotkeysStopped)??;

    Ok(HotkeyWatcher { thread })
}

/// Owns the registrations for as long as it runs.
fn run(sender: &UnboundedSender<Intent>, bindings: Bindings, ready: &Sender<Result<u32>>) {
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            let _ = ready.send(Err(error.into()));
            return;
        }
    };

    for (hotkey, intent) in bindings.each() {
        match manager.register(hotkey) {
            Ok(()) => tracing::debug!(%hotkey, ?intent, "listening for a hotkey"),
            Err(error) => tracing::warn!(
                %hotkey,
                ?intent,
                %error,
                "another application holds this hotkey, so it will do nothing"
            ),
        }
    }

    if ready.send(Ok(compat::current_thread())).is_err() {
        return;
    }

    pump(sender, bindings);
}

/// Turns the crank Windows needs turning, and forwards what falls out.
fn pump(sender: &UnboundedSender<Intent>, bindings: Bindings) {
    // Not set_event_handler: it is a OnceCell, so a restart with new bindings would go on feeding
    // the sender the first one captured.
    let presses = GlobalHotKeyEvent::receiver();

    // The channel is one per process and outlives any single registration, so anything already in
    // it was pressed before these bindings existed.
    while presses.try_recv().is_ok() {}

    while compat::dispatch_next_message() {
        while let Ok(press) = presses.try_recv() {
            if press.state != HotKeyState::Pressed {
                continue;
            }

            let Some(intent) = bindings.meaning(press.id) else {
                tracing::warn!(id = press.id, "a hotkey fired that belongs to no binding");
                continue;
            };

            tracing::debug!(?intent, "the player pressed a hotkey");

            if sender.send(intent).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three bindings that cannot be told apart would silently make two of them do one thing.
    #[test]
    fn the_default_bindings_are_distinct() {
        let bindings = Bindings::default();
        let ids = bindings.each().map(|(hotkey, _)| hotkey.id());

        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
    }

    /// A press arrives as an id and nothing else, so this lookup is the whole of what it means.
    #[test]
    fn a_press_is_read_back_as_what_it_was_bound_to() {
        let bindings = Bindings::default();

        assert_eq!(bindings.meaning(bindings.start.id()), Some(Intent::Start));
        assert_eq!(bindings.meaning(bindings.stop.id()), Some(Intent::Stop));
        assert_eq!(bindings.meaning(bindings.pause.id()), Some(Intent::Pause));
    }

    /// Presses from another application's binding reach the same channel.
    #[test]
    fn an_unbound_press_means_nothing() {
        let bindings = Bindings::default();
        let stranger = HotKey::new(Some(Modifiers::SHIFT), Code::F9);

        assert_eq!(bindings.meaning(stranger.id()), None);
    }

    /// The bindings a player is told to press are the ones that get registered.
    #[test]
    fn the_defaults_are_the_ones_mf_timer_uses() {
        let bindings = Bindings::default();

        assert_eq!(
            bindings.start,
            HotKey::new(Some(Modifiers::ALT), Code::KeyQ)
        );
        assert_eq!(bindings.stop, HotKey::new(Some(Modifiers::ALT), Code::KeyW));
        assert_eq!(
            bindings.pause,
            HotKey::new(Some(Modifiers::CONTROL), Code::Space)
        );
    }
}
