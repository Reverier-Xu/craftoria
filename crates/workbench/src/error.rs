use std::path::PathBuf;

use thiserror::Error;

/// Errors surfaced by the workbench framework.
#[derive(Debug, Error)]
pub enum WorkbenchError {
  #[error("failed to read the dock layout from `{}`: {source}", .path.display())]
  ReadLayout {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("failed to decode the dock layout from `{}`: {source}", .path.display())]
  DecodeLayout {
    path: PathBuf,
    #[source]
    source: serde_json::Error,
  },
  #[error("failed to encode the dock layout: {0}")]
  EncodeLayout(#[source] serde_json::Error),
  #[error("failed to write the dock layout to `{}`: {source}", .path.display())]
  WriteLayout {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
}
