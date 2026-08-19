//! Emilio: a companion app for Diablo II: Resurrected.
//!
//! There is no window yet. Until there is, running this follows the game and prints what it sees,
//! which is enough to check the tracking against real play.
//!
//! ```text
//! cargo run -- "C:\Users\<you>\Saved Games\Diablo II Resurrected"
//! ```
//!
//! Whatever goes wrong is printed by main rather than returned from it, since returning an error
//! prints its Debug form and the player would get a struct instead of the sentence written for
//! them.

#![deny(unsafe_code)]

/// Talking to Windows, which is the only thing allowed to be unsafe.
#[allow(unsafe_code)]
pub mod compat;

pub mod errors;
pub mod hotkeys;
pub mod run;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use d2r::sensing::{file, process};
use errors::Result;
use hotkeys::Bindings;
use run::{State, Update, stream};
use tokio::sync::{broadcast, mpsc};

/// How many updates a subscriber may fall behind before it starts missing them.
const UPDATE_BACKLOG: usize = 64;

#[tokio::main]
async fn main() -> ExitCode {
    // Quiet unless asked. `RUST_LOG=debug` follows the decisions, `RUST_LOG=trace` adds every
    // write the game makes, and `RUST_LOG=emilio=debug,d2r=trace` mixes the two.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    match follow().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("emilio: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Follows the game until interrupted.
async fn follow() -> Result<()> {
    let Some(directory) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run -- <save directory>");
        std::process::exit(2);
    };

    let (raw, events) = mpsc::unbounded_channel();
    let (presses, intents) = mpsc::unbounded_channel();
    let (updates, mut watching) = broadcast::channel(UPDATE_BACKLOG);

    let bindings = Bindings::default();

    let _processes = process::watch(raw.clone(), process::DEFAULT_POLL_INTERVAL)?;
    let _files = file::watch(raw.clone(), &directory)?;
    let _hotkeys = hotkeys::watch(presses, bindings)?;
    drop(raw);

    let watched = directory.clone();
    tokio::spawn(async move { stream::track(events, intents, &watched, updates).await });

    println!("watching {}", directory.display());
    println!(
        "{} starts a run, {} stops counting, {} holds the clock",
        bindings.start, bindings.stop, bindings.pause
    );
    println!("press ctrl-c to stop\n");

    loop {
        tokio::select! {
            update = watching.recv() => match update {
                Ok(update) => println!("{}", describe(&update)),
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    println!("(missed {missed} updates)");
                }
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}

/// Says what an update means in the words a player would use.
fn describe(update: &Update) -> String {
    match update {
        Update::Moved(state) => match state {
            State::NoProcess => "x  the game is gone".to_owned(),
            State::Stopped => "-  not running".to_owned(),
            State::Running { character, index } => {
                format!(
                    ">  run {index} under way, as {}",
                    named(character.as_deref())
                )
            }
            State::Paused { character, index } => {
                format!("|| run {index} held, as {}", named(character.as_deref()))
            }
        },
        Update::Finished(run) => format!(
            "#  run {} finished: {} after {}",
            run.index,
            named(run.character.as_deref()),
            spoken(run.duration)
        ),
        Update::Started { index, character } => {
            format!("+  run {index} started for {}", named(character.as_deref()))
        }
        Update::Saved {
            character,
            size_delta: 0,
        } => format!("   {character}: saved"),
        Update::Saved {
            character,
            size_delta,
        } => format!("   {character}: saved ({size_delta:+} bytes)"),
    }
}

/// A character's name, or an admission that nothing has said who is playing.
fn named(character: Option<&str>) -> String {
    character.unwrap_or("someone unnamed").to_owned()
}

/// A duration, in the units a person would use for a run.
fn spoken(duration: Duration) -> String {
    let seconds = duration.as_secs();

    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h{:02}m", seconds / 3600, (seconds % 3600) / 60),
    }
}
