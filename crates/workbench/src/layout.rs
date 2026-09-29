//! Dock layout persistence.
//!
//! The dock area layout is dumped to a [`DockAreaState`] and stored as JSON in
//! the platform config directory, so the workbench restores the user's panel
//! arrangement on the next start.

use std::path::{Path, PathBuf};

use woocraft::DockAreaState;

use crate::error::WorkbenchError;

/// Version of the persisted layout; bump when the panel set changes in
/// incompatible ways.
pub const LAYOUT_VERSION: usize = 1;

/// The platform configuration directory of craftoria.
fn config_dir() -> Option<PathBuf> {
  #[cfg(target_os = "windows")]
  {
    std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("craftoria"))
  }
  #[cfg(not(target_os = "windows"))]
  {
    std::env::var_os("XDG_CONFIG_HOME")
      .map(PathBuf::from)
      .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
      .map(|base| base.join("craftoria"))
  }
}

/// The file the dock layout is persisted to, if a config dir is available.
pub fn layout_file() -> Option<PathBuf> {
  config_dir().map(|dir| dir.join("layout.json"))
}

/// Load a persisted dock layout; `Ok(None)` when no layout was saved yet.
pub fn load(path: &Path) -> Result<Option<DockAreaState>, WorkbenchError> {
  match std::fs::read_to_string(path) {
    Ok(text) => {
      serde_json::from_str(&text)
        .map(Some)
        .map_err(|source| WorkbenchError::DecodeLayout {
          path: path.to_path_buf(),
          source,
        })
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(source) => Err(WorkbenchError::ReadLayout {
      path: path.to_path_buf(),
      source,
    }),
  }
}

/// Persist a dock layout, creating the config directory on first use.
pub fn save(path: &Path, state: &DockAreaState) -> Result<(), WorkbenchError> {
  let text = serde_json::to_string_pretty(state).map_err(WorkbenchError::EncodeLayout)?;
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|source| WorkbenchError::WriteLayout {
      path: parent.to_path_buf(),
      source,
    })?;
  }
  std::fs::write(path, text).map_err(|source| WorkbenchError::WriteLayout {
    path: path.to_path_buf(),
    source,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn load_returns_none_for_a_missing_file() {
    let path = std::env::temp_dir().join("craftoria-test-missing-layout.json");
    let state = load(&path).unwrap();
    assert!(state.is_none());
  }

  #[test]
  fn save_then_load_round_trips() {
    let path = std::env::temp_dir().join("craftoria-test-layout.json");
    let state = DockAreaState::default();
    save(&path, &state).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded, Some(state));
    std::fs::remove_file(&path).unwrap();
  }
}
