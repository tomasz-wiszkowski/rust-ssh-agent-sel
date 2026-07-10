use anyhow::{Context, Result};
use clap::Parser;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod agent;
mod cli;
mod client;
mod ctrl;
mod daemon;
mod hold;
mod monitor;
mod paths;
mod stack;

use cli::{Cli, SubCmd};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let _guard = init_logging(&cli.log_level)?;

    match &cli.cmd {
        Some(SubCmd::HoldRegister { path }) => return hold::run(path.clone()).await,
        Some(SubCmd::RunDaemon) => return daemon::run().await,
        None => {}
    }

    if cli.daemon {
        if daemon_ping().await {
            tracing::info!("daemon already running");
            return Ok(());
        }
        let exe = std::env::current_exe().context("resolving current executable")?;
        tokio::process::Command::new(&exe)
            .arg("--log-level")
            .arg(&cli.log_level)
            .arg("_run_daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawning daemon process")?;
        tracing::info!("daemon started in background");
        return Ok(());
    }

    // Auto-detect: client mode if a daemon is reachable, foreground daemon otherwise.
    if daemon_ping().await {
        client::run(&cli).await
    } else {
        daemon::run().await
    }
}

/// Send PING to the ctrl socket and return true if we get PONG back.
/// Cleans up stale socket files when the connect attempt fails.
async fn daemon_ping() -> bool {
    let ctrl_path = match paths::ctrl_socket_path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    if !ctrl_path.exists() {
        return false;
    }

    let stream = match tokio::net::UnixStream::connect(&ctrl_path).await {
        Ok(s) => s,
        Err(_) => {
            tracing::debug!("stale ctrl socket found; removing");
            let _ = std::fs::remove_file(&ctrl_path);
            return false;
        }
    };

    let result = tokio::time::timeout(Duration::from_secs(2), async {
        let (reader, mut writer) = stream.into_split();
        writer.write_all(b"PING\n").await?;
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        Ok::<bool, std::io::Error>(line.trim() == "PONG")
    })
    .await;

    matches!(result, Ok(Ok(true)))
}

fn init_logging(level: &str) -> Result<WorkerGuard> {
    let log_path = paths::log_file_path()?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).context("creating log directory")?;
    }

    let file_appender = tracing_appender::rolling::never(
        log_path.parent().unwrap(),
        log_path.file_name().unwrap(),
    );
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    Ok(guard)
}
