use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ssh-agent-sel",
    about = "SSH agent forwarding selector — daemon and client in one binary"
)]
pub struct Cli {
    /// SSH agent socket to register (defaults to $SSH_AUTH_SOCK)
    #[arg(long)]
    pub socket: Option<PathBuf>,

    /// Log level: trace, debug, info, warn, error
    #[arg(long, default_value = "warn")]
    pub log_level: String,

    /// Start the daemon in the background if it is not already running, then exit.
    /// If a daemon is already running, does nothing.
    #[arg(long)]
    pub daemon: bool,

    #[command(subcommand)]
    pub cmd: Option<SubCmd>,
}

#[derive(Subcommand)]
pub enum SubCmd {
    /// Internal: run the daemon loop (used by --daemon to start a background process).
    #[command(name = "_run_daemon", hide = true)]
    RunDaemon,
}
