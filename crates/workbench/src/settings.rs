//! Workbench settings persistence.
//!
//! Settings live as JSON in the platform config directory next to the dock
//! layout (see [`crate::layout`]). Today the file only holds the UI locale
//! override; more sections will join as the workbench grows.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{WorkbenchError, paths};

/// User-tunable workbench settings. Everything optional so that a fresh
/// install (or an absent file) means "follow the defaults".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
  /// The UI locale override; `None` follows the system locale.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub locale: Option<String>,
}

/// The file workbench settings are persisted to, if a config dir is
/// available.
pub fn settings_file() -> Option<PathBuf> {
  paths::config_dir().map(|dir| dir.join("settings.json"))
}

/// Load persisted workbench settings; `Ok(Settings::default())` when no
/// file was saved yet.
pub fn load(path: &Path) -> Result<Settings, WorkbenchError> {
  match std::fs::read_to_string(path) {
    Ok(text) => serde_json::from_str(&text).map_err(|source| WorkbenchError::DecodeSettings {
      path: path.to_path_buf(),
      source,
    }),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
    Err(source) => Err(WorkbenchError::ReadSettings {
      path: path.to_path_buf(),
      source,
    }),
  }
}

/// Persist workbench settings, creating the config directory on first use.
pub fn save(path: &Path, settings: &Settings) -> Result<(), WorkbenchError> {
  let text = serde_json::to_string_pretty(settings).map_err(WorkbenchError::EncodeSettings)?;
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|source| WorkbenchError::WriteSettings {
      path: parent.to_path_buf(),
      source,
    })?;
  }
  std::fs::write(path, text).map_err(|source| WorkbenchError::WriteSettings {
    path: path.to_path_buf(),
    source,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn load_returns_defaults_for_a_missing_file() {
    let path = std::env::temp_dir().join("craftoria-test-missing-settings.json");
    assert_eq!(load(&path).unwrap(), Settings::default());
  }

  #[test]
  fn save_then_load_round_trips() {
    let path = std::env::temp_dir().join("craftoria-test-settings.json");
    let settings = Settings {
      locale: Some("zh-hans".to_string()),
    };
    save(&path, &settings).unwrap();
    assert_eq!(load(&path).unwrap(), settings);
    std::fs::remove_file(&path).unwrap();
  }

  #[test]
  fn save_omits_unset_fields() {
    let path = std::env::temp_dir().join("craftoria-test-settings-empty.json");
    save(&path, &Settings::default()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "{}");
    assert_eq!(load(&path).unwrap(), Settings::default());
    std::fs::remove_file(&path).unwrap();
  }
}
