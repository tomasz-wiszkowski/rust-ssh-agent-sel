use anyhow::{Context, Result};
use std::path::PathBuf;

fn ssh_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".ssh"))
}

pub fn ctrl_socket_path() -> Result<PathBuf> {
    Ok(ssh_dir()?.join("ssh-agent.ctrl"))
}

pub fn agent_socket_path() -> Result<PathBuf> {
    Ok(ssh_dir()?.join("ssh-agent.sock"))
}

pub fn log_file_path() -> Result<PathBuf> {
    Ok(ssh_dir()?.join("ssh-agent.log"))
}
