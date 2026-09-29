use crate::error::ShellError;

/// Print platform and build information for the craftoria shell.
pub(crate) fn run() -> Result<(), ShellError> {
  println!("craftoria {}", env!("CARGO_PKG_VERSION"));
  println!("homepage: {}", env!("CARGO_PKG_HOMEPAGE"));
  println!("license: {}", env!("CARGO_PKG_LICENSE"));
  Ok(())
}
