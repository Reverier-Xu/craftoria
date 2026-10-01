//! Platform paths shared by the workbench persistence modules.

use std::path::PathBuf;

/// The platform configuration directory of craftoria.
pub(crate) fn config_dir() -> Option<PathBuf> {
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
