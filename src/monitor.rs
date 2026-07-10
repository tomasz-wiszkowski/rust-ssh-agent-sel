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
                let before = locked.snapshot().len();
                // drain_until_live prunes from the top; to prune all dead entries
                // across the whole stack we iterate the snapshot and remove missing ones.
                let dead: Vec<_> = locked
                    .snapshot()
                    .into_iter()
                    .filter(|p| !p.exists())
                    .collect();
                for p in dead {
                    locked.remove(&p);
                }
                let after = locked.snapshot().len();
                if after < before {
                    tracing::info!("monitor pruned {} dead socket(s)", before - after);
                }
            }
            _ = shutdown.changed() => break,
        }
    }
    Ok(())
}
