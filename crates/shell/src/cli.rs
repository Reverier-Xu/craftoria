use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
  name = "craftoria",
  version,
  about = "craftoria command-line interface"
)]
pub(crate) struct Cli {
  #[command(subcommand)]
  pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
  /// Print platform and build information.
  Info,
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
