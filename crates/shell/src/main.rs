#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use clap::Parser;
use tracing_subscriber::{EnvFilter, prelude::*};

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
  let cli = Cli::parse();
  let filter = install_tracing(cli.log_level.as_deref())?;
  tracing::debug!(?cli, filter, "parsed the command line");
  dispatch(cli, filter)
}

/// Install the global tracing subscriber.
///
/// Logs go to stderr so stdout stays reserved for command output (which may
/// be piped); every record is additionally mirrored into the workbench's
/// in-memory log buffer for the bottom-dock log panel. Returns the active
/// filter directives for display purposes.
fn install_tracing(log_level: Option<&str>) -> Result<String, ShellError> {
  let filter = match log_level {
    Some(directives) => EnvFilter::new(directives),
    None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
  };
  let directives = filter.to_string();
  let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
  let subscriber = tracing_subscriber::registry()
    .with(filter)
    .with(fmt_layer)
    .with(craftoria_workbench::logs::stream_layer());
  tracing::subscriber::set_global_default(subscriber)?;
  Ok(directives)
}

/// Single dispatch for every CLI command.
fn dispatch(cli: Cli, filter: String) -> Result<(), ShellError> {
  match cli.command.unwrap_or(Command::Gui) {
    Command::Info => commands::info::run(),
    Command::Gui => craftoria_workbench::run(craftoria_workbench::GuiOptions {
      reset_layout: cli.reset_layout,
      log_filter: filter,
    })
    .map_err(ShellError::from),
  }
}
