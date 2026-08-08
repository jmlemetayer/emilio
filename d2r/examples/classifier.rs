//! Prints what the game was doing, as it does it.
//!
//! ```text
//! cargo run -p d2r --example classifier -- "C:\Users\<you>\Saved Games\Diablo II Resurrected"
//! ```
//!
//! Where [`sensing`](../examples/sensing.rs) shows the raw writes, this shows what they were taken
//! to mean. Enter and leave a game, pick something up, quit: every line should match what you did.

use std::path::PathBuf;

use d2r::classifier::{self, SaveEvent};
use d2r::sensing::{file, process};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Some(directory) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run -p d2r --example classifier -- <save directory>");
        std::process::exit(2);
    };

    let (raw, events) = mpsc::unbounded_channel();
    let (classified, mut saves) = mpsc::unbounded_channel();

    let _processes = process::watch(raw.clone(), process::DEFAULT_POLL_INTERVAL)?;
    let _files = file::watch(raw.clone(), &directory)?;
    drop(raw);

    let watched = directory.clone();
    tokio::spawn(async move { classifier::stream::classify(events, classified, &watched).await });

    println!("watching {}", directory.display());
    println!("press ctrl-c to stop\n");

    loop {
        tokio::select! {
            save = saves.recv() => match save {
                Some(save) => println!("{}", describe(save)),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}

/// Says what an event means in the words a player would use.
fn describe(save: SaveEvent) -> String {
    match save {
        SaveEvent::Touched { character } => {
            format!(".  {character}: menu click (a game may be starting)")
        }
        SaveEvent::Entered { character } => format!(">  {character}: entered a game"),
        SaveEvent::Left { character } => format!("<  {character}: left the game"),
        SaveEvent::Saved {
            character,
            size_delta: 0,
        } => format!("   {character}: saved"),
        SaveEvent::Saved {
            character,
            size_delta,
        } if size_delta > 0 => {
            format!("+  {character}: saved, and gained something ({size_delta:+} bytes)")
        }
        SaveEvent::Saved {
            character,
            size_delta,
        } => format!("-  {character}: saved, and lost something ({size_delta:+} bytes)"),
        SaveEvent::QuitCleanly => "x  quit".to_owned(),
        SaveEvent::Crashed => "!  gone without saving (crash, or killed)".to_owned(),
    }
}
