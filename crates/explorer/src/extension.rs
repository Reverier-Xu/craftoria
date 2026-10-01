//! The explorer extension: metadata, locale catalogs, and panel
//! contributions wired into the workbench through the extension host.

use std::sync::Arc;

use craftoria_exthost::{
  extension::{Extension, ExtensionMetadata},
  i18n::LocaleCatalog,
  panel::{PanelBuilder, PanelContribution, PanelPlacement, PanelRole},
};
use woocraft::{DockPlacement, gpui::AppContext as _};

use crate::{dir_list::DirListPanel, panel::ExplorerPanel};

/// The embedded catalogs, as `(locale, TOML text)` pairs.
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

/// The workspace-explorer extension: a persistent file-tree panel plus
/// transient directory-listing panels.
#[derive(Debug, Clone)]
pub struct ExplorerExtension {
  metadata: ExtensionMetadata,
}

impl ExplorerExtension {
  pub fn new() -> Self {
    Self {
      metadata: ExtensionMetadata {
        id: "explorer".into(),
        name: "Craftoria Explorer".into(),
        vendor: "craftoria".into(),
        version: env!("CARGO_PKG_VERSION").into(),
      },
    }
  }
}

impl Default for ExplorerExtension {
  fn default() -> Self {
    Self::new()
  }
}

impl Extension for ExplorerExtension {
  fn metadata(&self) -> &ExtensionMetadata {
    &self.metadata
  }

  fn translations(&self) -> Vec<LocaleCatalog> {
    catalogs()
  }

  fn panels(&self) -> Vec<PanelContribution> {
    // `cx.new` invokes its closure synchronously, so the `window`/`dock`
    // borrows of the builder signature flow straight into the panel
    // constructors.
    let tree: PanelBuilder = Arc::new(|dock, _state, _info, window, cx| {
      Box::new(cx.new(|cx| ExplorerPanel::new(dock, window, cx)))
    });
    let dir_list: PanelBuilder = Arc::new(|dock, _state, info, window, cx| {
      Box::new(cx.new(|cx| DirListPanel::from_state(info, dock, window, cx)))
    });

    vec![
      PanelContribution {
        name: crate::panel::PANEL_NAME.into(),
        role: PanelRole::Persistent(
          PanelPlacement::docked(DockPlacement::Left)
            .with_size(woocraft::gpui::px(260.))
            .expanded(),
        ),
        build: tree,
      },
      PanelContribution {
        name: crate::dir_list::PANEL_NAME.into(),
        role: PanelRole::Transient,
        build: dir_list,
      },
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_embedded_catalog_parses_in_the_extension_subtree() {
    let catalogs = catalogs();
    assert_eq!(catalogs.len(), CATALOGS.len());
    for catalog in &catalogs {
      for key in catalog.translations().keys() {
        assert!(
          key.starts_with("tech.woooo.craft.ext.explorer."),
          "key {key} escaped the explorer subtree"
        );
      }
    }
  }

  #[test]
  fn all_catalogs_expose_the_same_key_set() {
    let catalogs = catalogs();
    let reference = catalogs[0]
      .translations()
      .keys()
      .cloned()
      .collect::<std::collections::BTreeSet<_>>();
    assert!(!reference.is_empty());
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
}
