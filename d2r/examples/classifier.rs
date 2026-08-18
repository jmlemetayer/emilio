//! Prints what the game was doing, as it does it.
//!
//! ```text
//! cargo run -p d2r --example classifier -- "C:\Users\<you>\Saved Games\Diablo II Resurrected"
//! ```
//!
//! Where [`sensing`](../examples/sensing.rs) shows the raw writes and
//! [`writes`](../examples/writes.rs) shows what each one did to the file behind it, this shows what
//! they were taken to mean. Enter and leave a game, pick something up, quit: every line should
//! match what you did.

use std::path::PathBuf;

use d2r::classifier::{self, GameEvent};
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
    let (classified, mut happenings) = mpsc::unbounded_channel();

    let _processes = process::watch(raw.clone(), process::DEFAULT_POLL_INTERVAL)?;
    let _files = file::watch(raw.clone(), &directory)?;
    drop(raw);

    let watched = directory.clone();
    tokio::spawn(async move { classifier::stream::classify(events, classified, &watched).await });

    println!("watching {}", directory.display());
    println!("press ctrl-c to stop\n");

    loop {
        tokio::select! {
            happening = happenings.recv() => match happening {
                Some(happening) => println!("{}", describe(&happening)),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}

/// Says what an event means in the words a player would use.
fn describe(event: &GameEvent) -> String {
    match event {
        GameEvent::Started => ">  the game started".to_owned(),
        GameEvent::Left { character } => format!("<  {character}: left the game"),
        GameEvent::Saved {
            character,
            size_delta: 0,
        } => format!("   {character}: saved"),
        GameEvent::Saved {
            character,
            size_delta,
        } if *size_delta > 0 => {
            format!("+  {character}: saved, and the inventory grew ({size_delta:+} bytes)")
        }
        GameEvent::Saved {
            character,
            size_delta,
        } => format!("-  {character}: saved, and the inventory shrank ({size_delta:+} bytes)"),
        GameEvent::Quit => "x  the game went away".to_owned(),
    }
}
