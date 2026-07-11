use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::watch;

use crate::{paths, stack::SharedStack};

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
                                tracing::warn!("ctrl connection error: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("ctrl accept error: {e}");
                        break;
                    }
                }
            }
            _ = shutdown.changed() => break,
        }
    }
    Ok(())
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    stack: SharedStack,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }

    let line = line.trim_end_matches('\n').trim_end_matches('\r');

    if line == "PING" {
        writer.write_all(b"PONG\n").await?;
        return Ok(());
    }

    let path = if let Some(rest) = line.strip_prefix("REGISTER ") {
        let path = std::path::PathBuf::from(rest);
        if !path.is_absolute() {
            writer.write_all(b"ERR path must be absolute\n").await?;
            return Ok(());
        }
        if paths::agent_socket_path().map(|p| p == path).unwrap_or(false) {
            tracing::warn!("rejected self-registration attempt (would create a forwarding loop)");
            writer.write_all(b"ERR cannot register the daemon's own socket\n").await?;
            return Ok(());
        }
        path
    } else {
        writer.write_all(b"ERR unknown command\n").await?;
        return Ok(());
    };

    let id = stack.lock().await.push(path.clone());
    writer.write_all(b"OK\n").await?;
    tracing::info!("socket [{id}] registered as \"{}\"", path.display());
    Ok(())
}
