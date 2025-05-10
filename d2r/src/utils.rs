use std::marker::Send;
use tokio::task::LocalSet;

use crate::errors::Result;

/// Spawn an async closure into a dedicated thread, allowing the use of `!Send` futures.
pub struct LocalSpawner {}

impl LocalSpawner {
    pub fn new<F, Fut>(name: &str, function: F) -> Result<()>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let thread = std::thread::Builder::new();

        let name = name.to_owned();

        thread.spawn(move || {
            let local = LocalSet::new();

            local.spawn_local(async move {
                while let Err(err) = function().await {
                    tracing::error!("{name}: {err}");
                }
            });

            rt.block_on(local);
        })?;

        Ok(())
    }
}
