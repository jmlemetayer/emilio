use anyhow::Result;
use std::time::Duration;
use tokio::{signal::windows::ctrl_c, sync::mpsc};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use d2r::tracker::process::ProcessTracker;

#[tokio::main]
async fn main() -> Result<()> {
    FmtSubscriber::builder().with_max_level(Level::DEBUG).init();

    let (send, mut recv) = mpsc::channel(10);

    ProcessTracker::new(send, Duration::from_millis(500)).await?;

    let mut ctrl_c_recv = ctrl_c()?;

    tokio::select! {
        _ = async {
            while let Some(event) = recv.recv().await {
                tracing::info!("Event: {event:?}");
            }
        } => (),
        _ = ctrl_c_recv.recv() => tracing::debug!("Received CTRL+C"),
    };

    Ok(())
}
