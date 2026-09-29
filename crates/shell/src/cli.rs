use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
  name = "craftoria",
  version,
  about = "craftoria command-line interface"
)]
pub(crate) struct Cli {
  /// Log filter directives (for example `debug` or
  /// `craftoria=trace,woocraft=info`). Overrides RUST_LOG; defaults to
  /// `info`.
  #[arg(long, global = true, value_name = "FILTER")]
  pub(crate) log_level: Option<String>,

  /// Start the workbench with the default dock layout, ignoring the saved one.
  #[arg(long, global = true)]
  pub(crate) reset_layout: bool,

  #[command(subcommand)]
  pub(crate) command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
  /// Print platform and build information.
  Info,
  /// Launch the desktop workbench (default when no subcommand is given).
  Gui,
}

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use super::Cli;

  #[test]
  fn cli_definition_is_valid() {
    Cli::command().debug_assert();
  }
}
