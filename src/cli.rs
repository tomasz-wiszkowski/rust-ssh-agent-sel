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
    #[arg(long, default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub cmd: Option<SubCmd>,
}

#[derive(Subcommand)]
pub enum SubCmd {
    /// Internal: hold a ctrl connection for the given socket path.
    /// Not intended for direct use.
    #[command(name = "_hold_register", hide = true)]
    HoldRegister {
        path: PathBuf,
    },
}
