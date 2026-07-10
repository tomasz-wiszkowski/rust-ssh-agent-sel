use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;

use crate::cli::Cli;
use crate::paths;

pub async fn run(cli: &Cli) -> Result<()> {
    let socket_path = resolve_socket(cli)?;

    let exe = std::env::current_exe().context("resolving current executable")?;

    // --log-level is a top-level Cli arg and must come before the subcommand name.
    tokio::process::Command::new(&exe)
        .arg("--log-level")
        .arg(&cli.log_level)
        .arg("_hold_register")
        .arg(&socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning holder process")?;

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
