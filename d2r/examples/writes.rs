//! Shows every write to the save directory, with the size and fingerprint of the file behind it.
//!
//! ```text
//! cargo run -p d2r --example writes -- "C:\Users\<you>\Saved Games\Diablo II Resurrected"
//! ```
//!
//! For answering questions the classifier cannot, of the form "what does the game actually write
//! when I do X?". Writes are grouped into the same 300ms bursts the classifier uses, so what moved
//! together is visible, and each line says whether the contents really changed or the file was
//! merely rewritten.
//!
//! Nothing here interprets anything: no events, no rules, just what happened on disk.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Local;
use d2r::classifier::{BURST_WINDOW, FileFingerprints, Fingerprint, Fingerprints};
use d2r::sensing::{OsEvent, file};
use tokio::sync::mpsc;
use tokio::time::{Instant as Deadline, sleep_until};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(directory) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run -p d2r --example writes -- <save directory>");
        std::process::exit(2);
    };

    let mut fingerprints = FileFingerprints;
    let mut known: HashMap<OsString, Fingerprint> = HashMap::new();

    // Everything, not just saves: the point is to find out what matters.
    if let Ok(entries) = std::fs::read_dir(&directory) {
        for path in entries.flatten().map(|entry| entry.path()) {
            if let (Some(name), Some(fingerprint)) =
                (path.file_name(), fingerprints.fingerprint(&path))
            {
                known.insert(name.to_owned(), fingerprint);
            }
        }
    }

    println!("watching {}", directory.display());
    println!("{} files fingerprinted to start with", known.len());
    println!("press ctrl-c to stop\n");

    let (sender, mut events) = mpsc::unbounded_channel();
    let _watcher = file::watch(sender, &directory)?;

    let mut deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            biased;

            event = events.recv() => match event {
                Some(event) => {
                    if deadline.is_none() {
                        println!("--- burst at {} ---", Local::now().format("%H:%M:%S%.3f"));
                    }

                    report(&event, &mut known, &mut fingerprints);
                    deadline = Some(Instant::now() + BURST_WINDOW);
                }
                None => break,
            },

            () = wait_until(deadline), if deadline.is_some() => {
                println!();
                deadline = None;
            }

            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}

/// Says what happened to one file, and whether its contents actually moved.
fn report(
    event: &OsEvent,
    known: &mut HashMap<OsString, Fingerprint>,
    fingerprints: &mut impl Fingerprints,
) {
    let (verb, path) = match event {
        OsEvent::FileCreated(path) => ("created", path),
        OsEvent::FileModified(path) => ("written", path),
        OsEvent::FileRemoved(path) => {
            if let Some(name) = path.file_name() {
                known.remove(name);
            }

            println!("  {:<38} removed", name_of(path));
            return;
        }
        OsEvent::ProcessStarted(pid) => {
            println!("  the game started (pid {pid})");
            return;
        }
        OsEvent::ProcessStopped(pid) => {
            println!("  the game stopped (pid {pid})");
            return;
        }
    };

    let name = name_of(path);

    let Some(current) = fingerprints.fingerprint(path) else {
        println!("  {name:<38} {verb}, COULD NOT READ");
        return;
    };

    let previous = path
        .file_name()
        .and_then(|file| known.insert(file.to_owned(), current));

    match previous {
        Some(previous) if previous.hash == current.hash => {
            println!(
                "  {name:<38} {verb}, {} bytes, unchanged   {}",
                current.size,
                short(&current)
            );
        }
        Some(previous) => {
            let delta = current.size as i64 - previous.size as i64;

            println!(
                "  {name:<38} {verb}, {} -> {} bytes ({delta:+})   CHANGED   {}",
                previous.size,
                current.size,
                short(&current)
            );
        }
        None => {
            println!(
                "  {name:<38} {verb}, {} bytes, NEW   {}",
                current.size,
                short(&current)
            );
        }
    }
}

/// A file's name without its directory.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Enough of a hash to recognise it again by eye.
fn short(fingerprint: &Fingerprint) -> String {
    let [a, b, c, d, ..] = fingerprint.hash;

    format!("{a:02x}{b:02x}{c:02x}{d:02x}")
}

/// Sleeps until an instant that is known to be there.
async fn wait_until(deadline: Option<Instant>) {
    if let Some(deadline) = deadline {
        sleep_until(Deadline::from_std(deadline)).await;
    }
}
