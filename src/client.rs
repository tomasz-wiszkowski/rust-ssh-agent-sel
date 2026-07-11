use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::cli::Cli;
use crate::paths;

pub async fn run(cli: &Cli) -> Result<()> {
    let socket_path = resolve_socket(cli)?;
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
        .write_all(format!("REGISTER {}\n", socket_path.display()).as_bytes())
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

    let agent_sock = paths::agent_socket_path()?;
    println!("export SSH_AUTH_SOCK={}", agent_sock.display());
    Ok(())
}

fn resolve_socket(cli: &Cli) -> Result<PathBuf> {
    if let Some(ref p) = cli.socket {
        return Ok(p.clone());
    }
    match std::env::var("SSH_AUTH_SOCK") {
        Ok(v) if !v.is_empty() => Ok(PathBuf::from(v)),
        _ => bail!("no socket specified and SSH_AUTH_SOCK is not set"),
    }
}
