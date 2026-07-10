use anyhow::{Context, Result};
use clap::Parser;
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
        Some(SubCmd::HoldRegister { path }) => hold::run(path.clone()).await,
        None => {
            if ctrl_is_live().await {
                client::run(&cli).await
            } else {
                daemon::run().await
            }
        }
    }
}

/// Returns true if the ctrl socket exists and accepts a connection.
/// Removes a stale (unconnectable) socket file if found.
async fn ctrl_is_live() -> bool {
    let ctrl_path = match paths::ctrl_socket_path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    if !ctrl_path.exists() {
        return false;
    }

    match tokio::net::UnixStream::connect(&ctrl_path).await {
        Ok(_) => true,
        Err(_) => {
            tracing::debug!("stale ctrl socket found; removing");
            let _ = std::fs::remove_file(&ctrl_path);
            false
        }
    }
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
