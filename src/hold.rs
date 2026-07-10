use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::paths;

pub async fn run(path: PathBuf) -> Result<()> {
    let ctrl_path = paths::ctrl_socket_path()?;

    // Retry a few times — the daemon may not have bound the socket yet.
    let mut stream = None;
    for attempt in 1..=5 {
        match UnixStream::connect(&ctrl_path).await {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => {
                tracing::debug!("ctrl connect attempt {attempt}/5 failed: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    let stream = stream.context("could not connect to daemon control socket")?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    writer
        .write_all(format!("REGISTER {}\n", path.display()).as_bytes())
        .await
        .context("sending REGISTER")?;

    let mut response = String::new();
    reader
        .read_line(&mut response)
        .await
        .context("reading response")?;

    let response = response.trim();
    if response != "OK" {
        bail!("daemon rejected registration: {response}");
    }

    // Install SIGTERM handler so we exit cleanly when the session ends.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("SIGTERM handler")?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
        // Also exit if the daemon goes away (reader returns EOF).
        result = async {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 1];
            reader.read(&mut buf).await
        } => {
            if let Ok(0) | Err(_) = result {
                tracing::info!("daemon closed ctrl connection; exiting");
            }
        }
    }

    Ok(())
}
