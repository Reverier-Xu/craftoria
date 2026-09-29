#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use clap::Parser;

pub(crate) mod cli;
pub(crate) mod commands;
pub(crate) mod error;

use cli::{Cli, Command};
use error::ShellError;

fn main() {
  if let Err(error) = run() {
    tracing::error!(%error, "command failed");
    std::process::exit(1);
  }
}

fn run() -> Result<(), ShellError> {
  // Logs go to stderr so stdout stays reserved for command output (which may
  // be piped). The default level is info so every user-visible message
  // appears; RUST_LOG overrides it.
  let filter = tracing_subscriber::EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
  let subscriber = tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter(filter)
    .finish();
  tracing::subscriber::set_global_default(subscriber)?;

  dispatch(Cli::parse().command)
}

/// Single dispatch for every CLI command.
fn dispatch(command: Command) -> Result<(), ShellError> {
  match command {
    Command::Info => commands::info::run(),
  }
}
