//! The workbench's built-in extension: the panels and locale catalogs the
//! shell itself contributes.
//!
//! Going through the same [`Extension`] contract as every other
//! contributor keeps exactly one registration path: the log console and
//! the welcome placeholder dock exactly like an external extension's
//! panels would, and the composition root (the `craftoria` shell)
//! registers this extension alongside the real ones before the first
//! window opens.

use std::sync::Arc;

use craftoria_exthost::{
  extension::{Extension, ExtensionMetadata},
  i18n::LocaleCatalog,
  panel::{PanelBuilder, PanelContribution, PanelPlacement, PanelRole},
};
use woocraft::{DockPlacement, gpui::AppContext as _};

use crate::panels::{LogPanel, PlaceholderPanel};

/// The embedded catalogs, as `(locale, TOML text)` pairs. Locale spellings
/// are the canonical ones woocraft normalizes `zh-cn`/`zh-tw` into.
const CATALOGS: [(&str, &str); 4] = [
  ("en-us", include_str!("../locales/en-us.toml")),
  ("zh-hans", include_str!("../locales/zh-hans.toml")),
  ("zh-hant", include_str!("../locales/zh-hant.toml")),
  ("ja-jp", include_str!("../locales/ja-jp.toml")),
];

/// Parses every embedded catalog, logging and skipping broken ones (a bad
/// catalog must not take the workbench down).
fn catalogs() -> Vec<LocaleCatalog> {
  CATALOGS
    .iter()
    .filter_map(
      |(locale, text)| match LocaleCatalog::from_toml(*locale, text) {
        Ok(catalog) => Some(catalog),
        Err(error) => {
          tracing::error!(locale = *locale, %error, "failed to parse an embedded catalog");
          None
        }
      },
    )
    .collect()
}

/// The built-in workbench extension.
#[derive(Debug, Clone)]
pub struct WorkbenchExtension {
  metadata: ExtensionMetadata,
}

impl WorkbenchExtension {
  pub fn new() -> Self {
    Self {
      metadata: ExtensionMetadata {
        id: "workbench".into(),
        name: "Craftoria Workbench".into(),
        vendor: "craftoria".into(),
        version: env!("CARGO_PKG_VERSION").into(),
      },
    }
  }
}

impl Default for WorkbenchExtension {
  fn default() -> Self {
    Self::new()
  }
}

impl Extension for WorkbenchExtension {
  fn metadata(&self) -> &ExtensionMetadata {
    &self.metadata
  }

  fn translations(&self) -> Vec<LocaleCatalog> {
    catalogs()
  }

  fn panels(&self) -> Vec<PanelContribution> {
    let logs: PanelBuilder =
      Arc::new(|_dock, _state, _info, window, cx| Box::new(cx.new(|cx| LogPanel::new(window, cx))));
    let welcome: PanelBuilder =
      Arc::new(|_dock, _state, _info, _window, cx| Box::new(cx.new(PlaceholderPanel::welcome)));

    vec![
      PanelContribution {
        name: LogPanel::PANEL_NAME.into(),
        role: PanelRole::Persistent(
          PanelPlacement::docked(DockPlacement::Bottom)
            .with_size(woocraft::gpui::px(240.))
            .expanded(),
        ),
        build: logs,
      },
      PanelContribution {
        name: PlaceholderPanel::WELCOME_NAME.into(),
        role: PanelRole::Persistent(PanelPlacement::docked(DockPlacement::Center).expanded()),
        build: welcome,
      },
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_embedded_catalog_parses() {
    let catalogs = catalogs();
    assert_eq!(
      catalogs.len(),
      CATALOGS.len(),
      "an embedded catalog failed to parse"
    );
    for (catalog, (locale, _)) in catalogs.iter().zip(CATALOGS) {
      assert_eq!(catalog.locale(), locale);
    }
  }

  /// A locale missing a key renders the raw key in the UI, so the four
  /// catalogs must stay in lockstep.
  #[test]
  fn all_catalogs_expose_the_same_key_set() {
    let catalogs = catalogs();
    let reference = catalogs[0]
      .translations()
      .keys()
      .cloned()
      .collect::<std::collections::BTreeSet<_>>();

    assert!(
      !reference.is_empty(),
      "the reference catalog must contain keys"
    );
    for catalog in &catalogs[1..] {
      let keys = catalog
        .translations()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
      assert_eq!(
        keys,
        reference,
        "the {} catalog drifted from the {} catalog",
        catalog.locale(),
        catalogs[0].locale()
      );
    }
  }

  #[test]
  fn placeholder_strings_declare_their_arguments() {
    // The interpolation convention (`%{name}`) is easy to break while
    // editing a translation; pin the two parameterized strings.
    let catalogs = catalogs();
    for catalog in &catalogs {
      let translations = catalog.translations();
      let log_lines = &translations["tech.woooo.craft.status_bar.log_lines"];
      assert!(
        log_lines.contains("%{count}"),
        "{log_lines} lost %{{count}}"
      );
      let filter = &translations["tech.woooo.craft.status_bar.filter"];
      assert!(filter.contains("%{filter}"), "{filter} lost %{{filter}}");
    }
  }
}
