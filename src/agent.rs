use anyhow::Result;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;

use crate::stack::SharedStack;

pub async fn accept_loop(
    listener: UnixListener,
    stack: SharedStack,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let stack = stack.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, stack).await {
                                tracing::debug!("agent connection ended: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("agent accept error: {e}");
                        break;
                    }
                }
            }
            _ = shutdown.changed() => break,
        }
    }
    Ok(())
}

async fn handle_connection(mut downstream: UnixStream, stack: SharedStack) -> Result<()> {
    // Walk the stack to find a live upstream socket.
    loop {
        let candidate = stack.lock().await.drain_until_live();
        let Some(path) = candidate else {
            tracing::warn!("no live upstream agent; closing incoming connection");
            return Ok(());
        };

        match UnixStream::connect(&path).await {
            Ok(mut upstream) => {
                tracing::info!(path = %path.display(), "forwarding to socket");
                tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await?;
                tracing::debug!(path = %path.display(), "forwarding session ended");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), "socket unreachable ({e}); dropping from stack");
                stack.lock().await.remove(&path);
            }
        }
    }
}
