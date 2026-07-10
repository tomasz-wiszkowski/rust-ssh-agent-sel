use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
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

    // Read the REGISTER command
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }

    let line = line.trim_end_matches('\n').trim_end_matches('\r');

    let path = if let Some(rest) = line.strip_prefix("REGISTER ") {
        let path = std::path::PathBuf::from(rest);
        if !path.is_absolute() {
            writer.write_all(b"ERR path must be absolute\n").await?;
            return Ok(());
        }
        path
    } else {
        writer.write_all(b"ERR unknown command\n").await?;
        return Ok(());
    };

    stack.lock().await.push(path.clone());
    writer.write_all(b"OK\n").await?;

    // Hold the connection; an EOF signals the session has ended.
    let mut buf = [0u8; 1];
    loop {
        use tokio::io::AsyncReadExt;
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }

    stack.lock().await.remove(&path);
    Ok(())
}
