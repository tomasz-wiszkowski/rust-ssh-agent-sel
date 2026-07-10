use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;
use tokio::sync::watch;

use crate::{agent, ctrl, monitor, paths, stack::AgentStack};

struct SocketGuard(PathBuf);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn bind_socket(path: &Path) -> Result<UnixListener> {
    // Remove stale file if present.
    match std::fs::remove_file(path) {
        Ok(()) => tracing::debug!(path = %path.display(), "removed stale socket"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).context("removing stale socket"),
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("setting permissions on {}", path.display()))?;
    Ok(listener)
}

pub async fn run() -> Result<()> {
    let ctrl_path = paths::ctrl_socket_path()?;
    let agent_path = paths::agent_socket_path()?;

    // Ensure ~/.ssh exists.
    if let Some(parent) = ctrl_path.parent() {
        std::fs::create_dir_all(parent).context("creating ~/.ssh")?;
    }

    let ctrl_listener = bind_socket(&ctrl_path)?;
    let agent_listener = bind_socket(&agent_path)?;

    // Guards remove socket files when they drop at the end of this function.
    let _ctrl_guard = SocketGuard(ctrl_path);
    let _agent_guard = SocketGuard(agent_path);

    tracing::info!("daemon started");

    let stack = AgentStack::shared();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let ctrl_task = tokio::spawn(ctrl::accept_loop(
        ctrl_listener,
        stack.clone(),
        shutdown_rx.clone(),
    ));
    let agent_task = tokio::spawn(agent::accept_loop(
        agent_listener,
        stack.clone(),
        shutdown_rx.clone(),
    ));
    let monitor_task = tokio::spawn(monitor::run(stack, shutdown_rx));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received SIGINT; shutting down");
        }
        _ = sigterm() => {
            tracing::info!("received SIGTERM; shutting down");
        }
    }

    // Signal all tasks to stop.
    let _ = shutdown_tx.send(true);

    let _ = tokio::join!(ctrl_task, agent_task, monitor_task);
    tracing::info!("daemon stopped");
    Ok(())
}

async fn sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    if let Ok(mut s) = signal(SignalKind::terminate()) {
        s.recv().await;
    } else {
        // If we can't install SIGTERM handler, just block forever — ctrl_c will still work.
        std::future::pending::<()>().await;
    }
}
