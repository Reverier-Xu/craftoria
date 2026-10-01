//! The extension registry: the seed of craftoria's plugin system.
//!
//! An extension is a unit that contributes capabilities to the workbench.
//! Today the only contribution kind is translations
//! ([`Extension::translations`]); commands, panels, and settings sections
//! will join the same contract as the host grows, so that a built-in module
//! and a future dynamically loaded plugin are indistinguishable from the
//! host's point of view.
//!
//! Registration is static for now — built-in modules call
//! [`register_extension`] at startup — but the API is shaped for a dynamic
//! loader: contributions always flow through the same registry and the same
//! [`init`] flush, never through ad-hoc shortcuts.

use std::sync::{Arc, LazyLock, RwLock};

use woocraft::gpui::SharedString;

use crate::i18n::{self, LocaleCatalog};

/// Who an extension is: stable identity plus display metadata.
///
/// The `id` doubles as the extension's i18n subtree owner — its translations
/// live under `tech.woooo.craft.ext.<id>` (see
/// [`i18n::craftoria_ext_key`]) — so it must be a stable, filesystem- and
/// registry-safe identifier (`terminal`, `modeling.rig`, …), not a display
/// name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionMetadata {
  /// The stable extension identifier.
  pub id: SharedString,
  /// The human-readable extension name.
  pub name: SharedString,
  /// Who ships the extension.
  pub vendor: SharedString,
  /// The extension version, as a plain string.
  pub version: SharedString,
}

/// A unit of workbench capability contribution.
///
/// Keep implementations cheap and synchronous: the host calls them once at
/// startup (and, later, at plugin activation). Anything heavier belongs
/// behind the contribution data it returns.
pub trait Extension: Send + Sync {
  /// The extension identity.
  fn metadata(&self) -> &ExtensionMetadata;

  /// Locale catalogs this extension contributes. Keys must be
  /// fully-qualified and inside the craftoria domain tree — built-in
  /// modules use core keys (`tech.woooo.craft.…`), extensions their own
  /// subtree built with [`i18n::craftoria_ext_key`].
  fn translations(&self) -> Vec<LocaleCatalog> {
    Vec::new()
  }
}

/// The registered extensions, in registration order.
static EXTENSIONS: LazyLock<RwLock<Vec<Arc<dyn Extension>>>> =
  LazyLock::new(|| RwLock::new(Vec::new()));

fn extensions_locked() -> std::sync::RwLockReadGuard<'static, Vec<Arc<dyn Extension>>> {
  EXTENSIONS
    .read()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Registers an extension with the host. Registering an `id` twice is a
/// bug, not a reload: the duplicate is rejected with a warning.
pub fn register_extension(extension: Arc<dyn Extension>) {
  let mut extensions = EXTENSIONS
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let id = extension.metadata().id.clone();
  if extensions
    .iter()
    .any(|registered| registered.metadata().id == id)
  {
    tracing::warn!(%id, "ignoring a duplicate extension registration");
    return;
  }
  tracing::debug!(%id, "registered an extension");
  extensions.push(extension);
}

/// Every registered extension, in registration order.
pub fn extensions() -> Vec<Arc<dyn Extension>> {
  extensions_locked().clone()
}

/// Flushes every registered extension's contributions into the host.
///
/// Called once at startup, after the built-in modules registered their
/// extensions and before the first window opens. Catalogs that fail domain
/// validation are skipped with a warning — a broken catalog must not take
/// the workbench down.
pub fn init() {
  let extensions = extensions_locked().clone();
  for extension in &extensions {
    let metadata = extension.metadata();
    for catalog in extension.translations() {
      let locale = catalog.locale().to_string();
      let count = catalog.translations().len();
      let out_of_domain = catalog
        .translations()
        .keys()
        .any(|key| !key.starts_with(i18n::CRAFTORIA_I18N_DOMAIN));

      if out_of_domain {
        // `from_toml` already rejects out-of-domain keys, so this only
        // guards catalogs built through other constructors.
        tracing::warn!(
          extension = %metadata.id,
          locale = %locale,
          "skipping a catalog outside the craftoria i18n domain"
        );
        continue;
      }

      i18n::register_catalog(catalog);
      tracing::info!(
        extension = %metadata.id,
        locale = %locale,
        keys = count,
        "loaded extension translations"
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;

  struct StubExtension {
    metadata: ExtensionMetadata,
  }

  impl Extension for StubExtension {
    fn metadata(&self) -> &ExtensionMetadata {
      &self.metadata
    }
  }

  fn metadata(id: &str) -> ExtensionMetadata {
    ExtensionMetadata {
      id: id.into(),
      name: id.into(),
      vendor: "craftoria".into(),
      version: "0.0.0".into(),
    }
  }

  #[test]
  fn extensions_register_in_order_and_reject_duplicates() {
    let first = Arc::new(StubExtension {
      metadata: metadata("test.first"),
    });
    let second = Arc::new(StubExtension {
      metadata: metadata("test.second"),
    });

    register_extension(first.clone());
    register_extension(second.clone());
    // A duplicate id is ignored, not appended.
    register_extension(Arc::new(StubExtension {
      metadata: metadata("test.first"),
    }));

    let ids: Vec<String> = extensions()
      .iter()
      .map(|extension| extension.metadata().id.to_string())
      .collect();
    assert!(ids.contains(&"test.first".to_string()));
    assert!(ids.contains(&"test.second".to_string()));
    assert_eq!(
      ids.iter().filter(|id| *id == "test.first").count(),
      1,
      "the duplicate must not have been registered"
    );
  }
}
