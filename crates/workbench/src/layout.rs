//! Dock layout persistence.
//!
//! The dock area layout is dumped to a [`DockAreaState`] and stored as JSON in
//! the platform config directory, so the workbench restores the user's panel
//! arrangement on the next start.

use std::path::{Path, PathBuf};

use woocraft::DockAreaState;

use crate::{WorkbenchError, paths};

/// Version of the persisted layout. Bump whenever the layout structure or
/// panel set changes; persisted states carrying any other version (or
/// none) are discarded during early development instead of migrated.
pub const LAYOUT_VERSION: usize = 2;

/// The file the dock layout is persisted to, if a config dir is available.
pub fn layout_file() -> Option<PathBuf> {
  paths::config_dir().map(|dir| dir.join("layout.json"))
}

/// Load a persisted dock layout; `Ok(None)` when no layout was saved yet
/// or the persisted version does not match [`LAYOUT_VERSION`] — stale
/// layouts re-default instead of being migrated.
pub fn load(path: &Path) -> Result<Option<DockAreaState>, WorkbenchError> {
  match std::fs::read_to_string(path) {
    Ok(text) => {
      let state = serde_json::from_str::<DockAreaState>(&text)
        .map(Some)
        .map_err(|source| WorkbenchError::DecodeLayout {
          path: path.to_path_buf(),
          source,
        })?;
      Ok(state.filter(|state| state.version == Some(LAYOUT_VERSION)))
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
    let state = DockAreaState {
      version: Some(LAYOUT_VERSION),
      ..DockAreaState::default()
    };
    save(&path, &state).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded, Some(state));
    std::fs::remove_file(&path).unwrap();
  }

  #[test]
  fn load_discards_stale_versions() {
    let path = std::env::temp_dir().join("craftoria-test-layout-stale.json");
    let state = DockAreaState {
      version: Some(LAYOUT_VERSION + 1),
      ..DockAreaState::default()
    };
    save(&path, &state).unwrap();
    assert_eq!(load(&path).unwrap(), None);
    std::fs::remove_file(&path).unwrap();
  }

  #[test]
  fn load_discards_unversioned_states() {
    let path = std::env::temp_dir().join("craftoria-test-layout-unversioned.json");
    save(&path, &DockAreaState::default()).unwrap();
    assert_eq!(load(&path).unwrap(), None);
    std::fs::remove_file(&path).unwrap();
  }
}
