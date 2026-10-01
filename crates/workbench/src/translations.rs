//! The workbench's built-in translations.
//!
//! The four locale catalogs are embedded at compile time and contributed to
//! the extension host through the same [`Extension`] contract a future
//! plugin will use: there is deliberately no separate registration path for
//! built-in UI strings. Keys live in the core `tech.woooo.craft` domain;
//! call sites resolve them through
//! [`craftoria_exthost::i18n::tr_static`].

use craftoria_exthost::{
  extension::{Extension, ExtensionMetadata},
  i18n::LocaleCatalog,
};

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
pub(crate) struct WorkbenchExtension {
  metadata: ExtensionMetadata,
}

impl WorkbenchExtension {
  pub(crate) fn new() -> Self {
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
