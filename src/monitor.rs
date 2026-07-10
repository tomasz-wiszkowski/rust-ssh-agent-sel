use anyhow::Result;
use std::time::Duration;
use tokio::sync::watch;

use crate::stack::SharedStack;

const CHECK_INTERVAL: Duration = Duration::from_secs(10);

pub async fn run(stack: SharedStack, mut shutdown: watch::Receiver<bool>) -> Result<()> {
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    interval.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let mut locked = stack.lock().await;
                let dead: Vec<_> = locked
                    .snapshot()
                    .into_iter()
                    .filter(|e| !e.path.exists())
                    .collect();
                for entry in dead {
                    if let Some(id) = locked.remove(&entry.path) {
                        tracing::info!("socket [{id}] gone; removing");
                    }
                }
            }
            _ = shutdown.changed() => break,
        }
    }
    Ok(())
}
