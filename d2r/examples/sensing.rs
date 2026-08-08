//! Prints raw sensing events as they happen, to watch the sensors against a real game.
//!
//! ```text
//! cargo run -p d2r --example sensing -- "C:\Users\<you>\Saved Games\Diablo II Resurrected"
//! ```
//!
//! The `-p d2r` is needed because the repository root is a package in its own right, and cargo
//! looks there first.

use std::path::PathBuf;

use d2r::sensing::{file, process};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let Some(directory) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run -p d2r --example sensing -- <save directory>");
        std::process::exit(2);
    };

    let (sender, mut receiver) = mpsc::unbounded_channel();

    let _processes = process::watch(sender.clone(), process::DEFAULT_POLL_INTERVAL)?;
    let _files = file::watch(sender.clone(), &directory)?;
    drop(sender);

    println!("watching D2R.exe and {}", directory.display());
    println!("press ctrl-c to stop\n");

    loop {
        tokio::select! {
            event = receiver.recv() => match event {
                Some(event) => println!("{event:?}"),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}
